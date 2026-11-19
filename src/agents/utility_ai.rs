use ::rand::rngs::StdRng;
use ::rand::Rng;
use macroquad::prelude::*;

use crate::world::{Material, World, GRID_H, GRID_W, TILE_PX, WET_SOIL_THRESHOLD};

use super::pathfinding::{
    find_build_spot, find_next_trench_dig_tile, find_safe_floor_near, find_tillable_spot,
    has_nearby_water, has_walkable_neighbor, is_near_hazard, is_tile_passable,
    is_tile_reachable_any, is_tile_reachable_by, nearest_tile_to, villager_component,
};
use super::{
    is_walkable, tile_center, tile_dist_sq, ColonyState, ItemDrop, ItemKind, JobKind, VillageScan,
    Villager, BUILD_WOOD_COST, EXTINGUISH_RADIUS, FLEE_HAZARD_RADIUS, HYSTERESIS_BONUS,
    VILLAGE_SCAN_RADIUS,
};

impl VillageScan {
    #[cfg(test)]
    pub(super) fn scan(world: &World) -> Self {
        let mut comp = vec![0_u16; GRID_W * GRID_H];
        super::pathfinding::compute_reachability(world, &[], &mut comp);
        Self::scan_with_reachability(world, &comp)
    }

    pub(super) fn scan_with_reachability(world: &World, reach_comp: &[u16]) -> Self {
        let center = world.village_center();
        let min_x = (center.0 - VILLAGE_SCAN_RADIUS).max(0);
        let max_x = (center.0 + VILLAGE_SCAN_RADIUS).min(GRID_W as i32 - 1);
        let min_y = (center.1 - VILLAGE_SCAN_RADIUS).max(0);
        let max_y = (center.1 + VILLAGE_SCAN_RADIUS).min(GRID_H as i32 - 1);

        let mut scan = Self {
            center,
            hazards: Vec::new(),
            village_fires: Vec::new(),
            water_edges: Vec::new(),
            lit_campfires: Vec::new(),
            unlit_campfires: Vec::new(),
            chests: Vec::new(),
            cabins: Vec::new(),
            ripe_crops: Vec::new(),
            all_crops: Vec::new(),
            ripe_bushes: Vec::new(),
            dry_tilled: Vec::new(),
            plantable_soil: Vec::new(),
            farm_tile_count: 0,
            trees: Vec::new(),
            reach_comp: reach_comp.to_vec(),
        };

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let Some(cell) = world.get(x, y) else {
                    continue;
                };
                match cell.material {
                    Material::Fire => {
                        scan.hazards.push((x, y));
                        let dx = (x - center.0).abs();
                        let dy = (y - center.1).abs();
                        if dx.max(dy) <= EXTINGUISH_RADIUS
                            && is_tile_reachable_any(reach_comp, x, y)
                        {
                            scan.village_fires.push((x, y));
                        }
                    }
                    Material::Lava | Material::Acid => {
                        scan.hazards.push((x, y));
                    }
                    Material::ShallowWater | Material::DeepWater => {
                        if cell.water_volume >= 20
                            && has_walkable_neighbor(world, x, y)
                            && is_tile_reachable_any(reach_comp, x, y)
                        {
                            scan.water_edges.push((x, y));
                        }
                    }
                    Material::Campfire => {
                        if cell.is_lit_campfire() {
                            scan.lit_campfires.push((x, y));
                        } else {
                            scan.unlit_campfires.push((x, y));
                        }
                    }
                    Material::Chest => scan.chests.push((x, y)),
                    Material::Cabin => scan.cabins.push((x, y)),
                    Material::Crop(stage) => {
                        if !is_tile_reachable_any(reach_comp, x, y) {
                            continue;
                        }
                        scan.all_crops.push((x, y));
                        scan.farm_tile_count += 1;
                        if stage >= 4 {
                            scan.ripe_crops.push((x, y));
                        } else if !cell.is_wet_crop()
                            && cell.moisture < WET_SOIL_THRESHOLD
                            && !has_nearby_water(world, x, y, 2)
                        {
                            scan.dry_tilled.push((x, y));
                        }
                    }
                    Material::TilledSoil => {
                        scan.farm_tile_count += 1;
                        if is_tile_reachable_any(reach_comp, x, y) {
                            scan.plantable_soil.push((x, y, false));
                            if cell.moisture < WET_SOIL_THRESHOLD
                                && !has_nearby_water(world, x, y, 2)
                            {
                                scan.dry_tilled.push((x, y));
                            }
                        }
                    }
                    Material::WetSoil if cell.is_tilled() => {
                        scan.farm_tile_count += 1;
                        if is_tile_reachable_any(reach_comp, x, y) {
                            scan.plantable_soil.push((x, y, true));
                        }
                    }
                    Material::BerryBush if cell.has_berries() => {
                        if is_tile_reachable_any(reach_comp, x, y) {
                            scan.ripe_bushes.push((x, y));
                        }
                    }
                    Material::Tree if is_tile_reachable_any(reach_comp, x, y) => {
                        scan.trees.push((x, y));
                    }
                    _ => {}
                }
            }
        }

        scan
    }
}

#[inline]
fn tile_allowed_for(
    scan: &VillageScan,
    v: &Villager,
    cid: u16,
    job: JobKind,
    tile: (i32, i32),
    sim_time: f32,
) -> bool {
    is_tile_reachable_by(&scan.reach_comp, cid, tile.0, tile.1)
        && !v.is_blacklisted(job, tile, sim_time)
}

fn nearest_allowed_tile(
    scan: &VillageScan,
    v: &Villager,
    cid: u16,
    job: JobKind,
    tiles: &[(i32, i32)],
    sim_time: f32,
) -> Option<(i32, i32)> {
    tiles
        .iter()
        .copied()
        .filter(|&t| tile_allowed_for(scan, v, cid, job, t, sim_time))
        .min_by_key(|&(x, y)| tile_dist_sq(v.pos, x, y))
}

fn nearest_allowed_dist_tiles(
    scan: &VillageScan,
    v: &Villager,
    cid: u16,
    job: JobKind,
    tiles: &[(i32, i32)],
    sim_time: f32,
) -> Option<f32> {
    let (tx, ty) = nearest_allowed_tile(scan, v, cid, job, tiles, sim_time)?;
    Some(v.pos.distance(tile_center(tx, ty)) / TILE_PX)
}

pub(super) fn pick_best_job(
    world: &World,
    scan: &VillageScan,
    v: &Villager,
    active_wolves: &[Vec2],
    colony: ColonyState,
    drops: &[ItemDrop],
) -> JobKind {
    // 1. FleeHazard overrides all other urges within 3 tiles (firefighters ignore fire distance while putting it out)
    let ignore_fire = v.job == JobKind::Extinguish;
    if is_near_hazard(world, v.pos, active_wolves, FLEE_HAZARD_RADIUS, ignore_fire) {
        return JobKind::FleeHazard;
    }

    let scores = [
        (
            JobKind::Extinguish,
            score_extinguish(scan, v, colony.sim_time),
        ),
        (JobKind::Eat, score_eat(scan, v, colony)),
        (JobKind::Warm, score_warm(world, scan, v)),
        (
            JobKind::HarvestHaul,
            score_harvest_haul(world, scan, v, colony.sim_time, drops),
        ),
        (JobKind::FarmIrrigate, score_farm_irrigate(scan, v, colony)),
        (
            JobKind::LumberBuild,
            score_lumber_build(world, scan, v, colony, drops),
        ),
        (JobKind::Idle, 0.15),
    ];

    let mut best_job = JobKind::Idle;
    let mut best_effective = -1.0_f32;
    for (job, raw) in scores {
        let clamped = raw.clamp(0.0, 1.0);
        let effective = if job == v.job && job != JobKind::Idle {
            clamped * HYSTERESIS_BONUS
        } else {
            clamped
        };
        if effective > best_effective {
            best_effective = effective;
            best_job = job;
        }
    }
    best_job
}

pub(super) fn score_extinguish(scan: &VillageScan, v: &Villager, sim_time: f32) -> f32 {
    let cid = villager_component(&scan.reach_comp, v.pos);
    let Some(nearest) = nearest_allowed_dist_tiles(
        scan,
        v,
        cid,
        JobKind::Extinguish,
        &scan.village_fires,
        sim_time,
    ) else {
        return 0.0;
    };
    let has_water = v.bucket_water > 0
        || scan
            .water_edges
            .iter()
            .any(|&wt| tile_allowed_for(scan, v, cid, JobKind::Extinguish, wt, sim_time));
    if !has_water {
        return 0.0;
    }
    let bonus = if v.bucket_water > 0 { 0.06 } else { 0.0 };
    (0.98 - (nearest / 36.0) * 0.04 + bonus).clamp(0.94, 1.0)
}

pub(super) fn score_eat(scan: &VillageScan, v: &Villager, colony: ColonyState) -> f32 {
    if v.hunger >= 0.72 {
        return 0.0;
    }
    let cid = villager_component(&scan.reach_comp, v.pos);
    let has_chest_food = colony.food_stored > 0
        && scan
            .chests
            .iter()
            .any(|&ch| tile_allowed_for(scan, v, cid, JobKind::Eat, ch, colony.sim_time));
    let has_bush = scan
        .ripe_bushes
        .iter()
        .any(|&b| tile_allowed_for(scan, v, cid, JobKind::Eat, b, colony.sim_time));
    let has_crop = scan
        .ripe_crops
        .iter()
        .any(|&c| tile_allowed_for(scan, v, cid, JobKind::Eat, c, colony.sim_time));
    if !has_chest_food && !has_bush && !has_crop {
        return 0.0;
    }
    let raw = ((0.75 - v.hunger) * 1.35).clamp(0.0, 0.94);
    // Once committed to walking for food, stay on Eat above routine chores (but below Extinguish/Warm/Flee)
    if v.job == JobKind::Eat {
        raw.max(0.65)
    } else {
        raw
    }
}

pub(super) fn score_warm(world: &World, scan: &VillageScan, v: &Villager) -> f32 {
    if scan.lit_campfires.is_empty() {
        return 0.0;
    }
    if world.is_night() {
        let base = if scan.village_fires.is_empty() {
            0.87
        } else {
            0.72
        };
        return (base + (1.0 - v.warmth) * 0.05).clamp(base, base + 0.05);
    }
    if v.warmth >= 0.50 {
        return 0.0;
    }
    ((0.78 - v.warmth) * 1.1).clamp(0.0, 0.85)
}

fn score_harvest_haul(
    world: &World,
    scan: &VillageScan,
    v: &Villager,
    sim_time: f32,
    drops: &[ItemDrop],
) -> f32 {
    if v.carrying == Some(ItemKind::Wood) {
        return 0.0;
    }
    let cid = villager_component(&scan.reach_comp, v.pos);
    let has_chest = scan
        .chests
        .iter()
        .any(|&ch| tile_allowed_for(scan, v, cid, JobKind::HarvestHaul, ch, sim_time));
    if matches!(v.carrying, Some(ItemKind::Crop | ItemKind::Berry)) && has_chest {
        return 0.84;
    }
    let has_food_drop = drops.iter().any(|d| {
        if !matches!(d.kind, ItemKind::Crop | ItemKind::Berry) || !is_walkable(world, d.pos) {
            return false;
        }
        let dt = (
            (d.pos.x / TILE_PX).floor() as i32,
            (d.pos.y / TILE_PX).floor() as i32,
        );
        tile_allowed_for(scan, v, cid, JobKind::HarvestHaul, dt, sim_time)
    });
    let has_ripe_crop = scan
        .ripe_crops
        .iter()
        .any(|&cr| tile_allowed_for(scan, v, cid, JobKind::HarvestHaul, cr, sim_time));
    if has_ripe_crop || (has_food_drop && has_chest) {
        return 0.72;
    }
    0.0
}

fn score_farm_irrigate(scan: &VillageScan, v: &Villager, colony: ColonyState) -> f32 {
    let cid = villager_component(&scan.reach_comp, v.pos);
    let needs_irrigation = !scan.water_edges.is_empty()
        && scan
            .dry_tilled
            .iter()
            .any(|&d| tile_allowed_for(scan, v, cid, JobKind::FarmIrrigate, d, colony.sim_time));
    let needs_plant = scan.plantable_soil.iter().any(|&(x, y, _)| {
        tile_allowed_for(scan, v, cid, JobKind::FarmIrrigate, (x, y), colony.sim_time)
    });
    let needs_till = scan.farm_tile_count < 14;
    if !needs_irrigation && !needs_plant && !needs_till {
        return 0.0;
    }
    if needs_irrigation {
        0.68
    } else if colony.food_stored < 8 {
        0.62
    } else {
        0.54
    }
}

fn score_lumber_build(
    world: &World,
    scan: &VillageScan,
    v: &Villager,
    colony: ColonyState,
    drops: &[ItemDrop],
) -> f32 {
    if matches!(v.carrying, Some(ItemKind::Crop | ItemKind::Berry)) {
        return 0.0;
    }
    let cid = villager_component(&scan.reach_comp, v.pos);
    let desired_cabins = (colony.villager_count / 4).max(1);
    let needs_structure = scan.chests.is_empty()
        || scan.lit_campfires.is_empty()
        || scan.cabins.len() < desired_cabins;
    let has_chest = scan
        .chests
        .iter()
        .any(|&ch| tile_allowed_for(scan, v, cid, JobKind::LumberBuild, ch, colony.sim_time));
    if v.carrying == Some(ItemKind::Wood) {
        return if needs_structure || has_chest {
            0.78
        } else {
            0.0
        };
    }
    let has_wood_drop = drops.iter().any(|d| {
        if d.kind != ItemKind::Wood || !is_walkable(world, d.pos) {
            return false;
        }
        let dt = (
            (d.pos.x / TILE_PX).floor() as i32,
            (d.pos.y / TILE_PX).floor() as i32,
        );
        tile_allowed_for(scan, v, cid, JobKind::LumberBuild, dt, colony.sim_time)
    });
    let has_tree = scan
        .trees
        .iter()
        .any(|&t| tile_allowed_for(scan, v, cid, JobKind::LumberBuild, t, colony.sim_time));
    if needs_structure && colony.wood_stored >= BUILD_WOOD_COST {
        return 0.64;
    }
    if has_wood_drop || (colony.wood_stored < 16 && has_tree) || (needs_structure && has_tree) {
        return 0.50;
    }
    0.0
}

pub(super) fn is_job_target_still_valid(
    world: &World,
    scan: &VillageScan,
    v: &Villager,
    drops: &[ItemDrop],
) -> bool {
    let Some((tx, ty)) = v.target_tile else {
        return false;
    };
    let Some(cell) = world.get(tx, ty) else {
        return false;
    };
    match v.job {
        JobKind::Idle | JobKind::FleeHazard => true,
        JobKind::Extinguish => {
            if v.bucket_water == 0 {
                cell.material.is_water() && cell.water_volume >= 20
            } else {
                cell.material == Material::Fire
            }
        }
        JobKind::Eat => {
            cell.material == Material::Chest
                || (cell.material == Material::BerryBush && cell.has_berries())
                || matches!(cell.material, Material::Crop(s) if s >= 4)
        }
        JobKind::Warm => {
            is_tile_passable(cell, false)
                && scan
                    .lit_campfires
                    .iter()
                    .any(|&(fx, fy)| (fx - tx).abs() <= 2 && (fy - ty).abs() <= 2)
        }
        JobKind::HarvestHaul => {
            if matches!(v.carrying, Some(ItemKind::Crop | ItemKind::Berry)) {
                cell.material == Material::Chest
            } else {
                matches!(cell.material, Material::Crop(s) if s >= 4)
                    || (is_tile_passable(cell, false)
                        && drops.iter().any(|d| {
                            matches!(d.kind, ItemKind::Crop | ItemKind::Berry)
                                && (d.pos.x / TILE_PX).floor() as i32 == tx
                                && (d.pos.y / TILE_PX).floor() as i32 == ty
                        }))
            }
        }
        JobKind::FarmIrrigate => matches!(
            cell.material,
            Material::Dirt
                | Material::Grass
                | Material::Sand
                | Material::Ash
                | Material::TilledSoil
                | Material::WetSoil
        ),
        JobKind::LumberBuild => {
            if v.carrying == Some(ItemKind::Wood) {
                cell.material == Material::Chest || !cell.material.is_solid_obstacle()
            } else {
                cell.material == Material::Tree
                    || cell.material == Material::Campfire
                    || matches!(cell.material, Material::Dirt | Material::Grass)
                    || drops.iter().any(|d| d.kind == ItemKind::Wood)
            }
        }
    }
}

pub(super) fn set_villager_target(v: &mut Villager, tile: (i32, i32), adjacent: bool) {
    if v.target_tile != Some(tile) || v.target_adjacent != adjacent {
        v.target_tile = Some(tile);
        v.target_adjacent = adjacent;
        v.path.clear();
        v.path_idx = 0;
        v.needs_path = true;
    } else if v.path.is_empty() {
        v.needs_path = true;
    }
}

fn set_tracked_target(v: &mut Villager, tile: (i32, i32), adjacent: bool, _sim_time: f32) {
    #[cfg(test)]
    if v.job != JobKind::Idle {
        v.record_assignment(_sim_time, tile);
    }
    set_villager_target(v, tile, adjacent);
}

pub(super) fn assign_flee_target(
    world: &World,
    scan: &VillageScan,
    active_wolves: &[Vec2],
    v: &mut Villager,
) {
    if world.is_night() {
        let wolf_near = active_wolves
            .iter()
            .any(|&wp| wp.distance(v.pos) <= 5.5 * TILE_PX);
        if wolf_near {
            if let Some((fx, fy)) = nearest_tile_to(v.pos, &scan.lit_campfires) {
                for (dx, dy) in [(0, 1), (1, 0), (-1, 0), (0, -1), (1, 1), (-1, 1)] {
                    let tx = fx + dx;
                    let ty = fy + dy;
                    if world
                        .get(tx, ty)
                        .is_some_and(|c| is_tile_passable(c, false))
                    {
                        set_villager_target(v, (tx, ty), false);
                        return;
                    }
                }
            }
        }
    }

    let vtx = (v.pos.x / TILE_PX).floor() as i32;
    let vty = (v.pos.y / TILE_PX).floor() as i32;
    let mut repulse = Vec2::ZERO;
    for &(hx, hy) in &scan.hazards {
        let hp = tile_center(hx, hy);
        let diff = v.pos - hp;
        let d = diff.length();
        if d > 0.1 && d <= 4.5 * TILE_PX {
            repulse += diff / (d * d);
        }
    }
    if world.is_night() {
        for &wp in active_wolves {
            let diff = v.pos - wp;
            let d = diff.length();
            if d > 0.1 && d <= 5.0 * TILE_PX {
                repulse += (diff / (d * d)) * 1.4;
            }
        }
    }
    let dir = if repulse == Vec2::ZERO {
        vec2(1.0, 0.0)
    } else {
        repulse.normalize()
    };
    for step in (2..=6).rev() {
        let tx = vtx + (dir.x * step as f32).round() as i32;
        let ty = vty + (dir.y * step as f32).round() as i32;
        if let Some(tile) = find_safe_floor_near(world, tx, ty, 3) {
            set_villager_target(v, tile, false);
            return;
        }
    }
}

pub(super) fn assign_job_target(
    world: &World,
    scan: &VillageScan,
    active_wolves: &[Vec2],
    v: &mut Villager,
    colony: ColonyState,
    drops: &[ItemDrop],
    rng: &mut StdRng,
) {
    let wood_stored = colony.wood_stored;
    let food_stored = colony.food_stored;
    let villager_count = colony.villager_count;
    let sim_time = colony.sim_time;
    let cid = villager_component(&scan.reach_comp, v.pos);

    match v.job {
        JobKind::FleeHazard => {
            assign_flee_target(world, scan, active_wolves, v);
        }
        JobKind::Extinguish => {
            if v.bucket_water == 0 {
                if let Some(wt) = nearest_allowed_tile(
                    scan,
                    v,
                    cid,
                    JobKind::Extinguish,
                    &scan.water_edges,
                    sim_time,
                ) {
                    set_tracked_target(v, wt, true, sim_time);
                }
            } else if let Some(ft) = nearest_allowed_tile(
                scan,
                v,
                cid,
                JobKind::Extinguish,
                &scan.village_fires,
                sim_time,
            ) {
                set_tracked_target(v, ft, true, sim_time);
            }
        }
        JobKind::Eat => {
            if food_stored > 0 {
                if let Some(ch) =
                    nearest_allowed_tile(scan, v, cid, JobKind::Eat, &scan.chests, sim_time)
                {
                    set_tracked_target(v, ch, true, sim_time);
                    return;
                }
            }
            if let Some(b) =
                nearest_allowed_tile(scan, v, cid, JobKind::Eat, &scan.ripe_bushes, sim_time)
            {
                set_tracked_target(v, b, true, sim_time);
            } else if let Some(cr) =
                nearest_allowed_tile(scan, v, cid, JobKind::Eat, &scan.ripe_crops, sim_time)
            {
                set_tracked_target(v, cr, true, sim_time);
            }
        }
        JobKind::Warm => {
            if let Some((fx, fy)) =
                nearest_allowed_tile(scan, v, cid, JobKind::Warm, &scan.lit_campfires, sim_time)
            {
                for (dx, dy) in [(0, 1), (1, 0), (-1, 0), (0, -1), (1, 1), (-1, 1)] {
                    let tx = fx + dx;
                    let ty = fy + dy;
                    if world
                        .get(tx, ty)
                        .is_some_and(|c| !c.material.is_solid_obstacle() && !c.material.is_water())
                        && tile_allowed_for(scan, v, cid, JobKind::Warm, (tx, ty), sim_time)
                    {
                        set_tracked_target(v, (tx, ty), false, sim_time);
                        return;
                    }
                }
            }
        }
        JobKind::HarvestHaul => {
            if matches!(v.carrying, Some(ItemKind::Crop | ItemKind::Berry)) {
                if let Some(ch) =
                    nearest_allowed_tile(scan, v, cid, JobKind::HarvestHaul, &scan.chests, sim_time)
                {
                    set_tracked_target(v, ch, true, sim_time);
                }
            } else if let Some(drop) = drops
                .iter()
                .filter(|d| {
                    if !matches!(d.kind, ItemKind::Crop | ItemKind::Berry)
                        || !is_walkable(world, d.pos)
                    {
                        return false;
                    }
                    let dt = (
                        (d.pos.x / TILE_PX).floor() as i32,
                        (d.pos.y / TILE_PX).floor() as i32,
                    );
                    tile_allowed_for(scan, v, cid, JobKind::HarvestHaul, dt, sim_time)
                })
                .min_by(|a, b| {
                    a.pos
                        .distance_squared(v.pos)
                        .total_cmp(&b.pos.distance_squared(v.pos))
                })
            {
                let tx = (drop.pos.x / TILE_PX).floor() as i32;
                let ty = (drop.pos.y / TILE_PX).floor() as i32;
                set_tracked_target(v, (tx, ty), false, sim_time);
            } else if let Some(cr) = nearest_allowed_tile(
                scan,
                v,
                cid,
                JobKind::HarvestHaul,
                &scan.ripe_crops,
                sim_time,
            ) {
                set_tracked_target(v, cr, true, sim_time);
            }
        }
        JobKind::FarmIrrigate => {
            if let Some(&(px, py, _)) = scan
                .plantable_soil
                .iter()
                .filter(|&&(x, y, wet)| {
                    wet && tile_allowed_for(scan, v, cid, JobKind::FarmIrrigate, (x, y), sim_time)
                })
                .min_by_key(|&&(x, y, _)| tile_dist_sq(v.pos, x, y))
            {
                set_tracked_target(v, (px, py), true, sim_time);
                return;
            }
            if let Some(dry) = nearest_allowed_tile(
                scan,
                v,
                cid,
                JobKind::FarmIrrigate,
                &scan.dry_tilled,
                sim_time,
            ) {
                if let Some(dig) = find_next_trench_dig_tile(world, scan, dry) {
                    if tile_allowed_for(scan, v, cid, JobKind::FarmIrrigate, dig, sim_time) {
                        set_tracked_target(v, dig, true, sim_time);
                        return;
                    }
                }
            }
            if let Some(&(px, py, _)) = scan
                .plantable_soil
                .iter()
                .filter(|&&(x, y, _)| {
                    tile_allowed_for(scan, v, cid, JobKind::FarmIrrigate, (x, y), sim_time)
                })
                .min_by_key(|&&(x, y, _)| tile_dist_sq(v.pos, x, y))
            {
                set_tracked_target(v, (px, py), true, sim_time);
                return;
            }
            if let Some(till_spot) = find_tillable_spot(world, scan.center) {
                if tile_allowed_for(scan, v, cid, JobKind::FarmIrrigate, till_spot, sim_time) {
                    set_tracked_target(v, till_spot, true, sim_time);
                }
            }
        }
        JobKind::LumberBuild => {
            let desired_cabins = (villager_count / 4).max(1);
            let build_needed = scan.chests.is_empty()
                || scan.lit_campfires.is_empty()
                || scan.cabins.len() < desired_cabins;
            let have_build_wood = wood_stored >= BUILD_WOOD_COST
                || (v.carrying == Some(ItemKind::Wood) && wood_stored + 2 >= BUILD_WOOD_COST);

            if build_needed && have_build_wood {
                if let Some(&cf) = scan.unlit_campfires.first() {
                    if scan.lit_campfires.is_empty()
                        && tile_allowed_for(scan, v, cid, JobKind::LumberBuild, cf, sim_time)
                    {
                        set_tracked_target(v, cf, true, sim_time);
                        return;
                    }
                }
                if let Some(spot) = find_build_spot(world, scan.center) {
                    if tile_allowed_for(scan, v, cid, JobKind::LumberBuild, spot, sim_time) {
                        set_tracked_target(v, spot, true, sim_time);
                        return;
                    }
                }
            }
            if v.carrying == Some(ItemKind::Wood) {
                if let Some(ch) =
                    nearest_allowed_tile(scan, v, cid, JobKind::LumberBuild, &scan.chests, sim_time)
                {
                    set_tracked_target(v, ch, true, sim_time);
                    return;
                }
            }
            if let Some(drop) = drops
                .iter()
                .filter(|d| {
                    if d.kind != ItemKind::Wood || !is_walkable(world, d.pos) {
                        return false;
                    }
                    let dt = (
                        (d.pos.x / TILE_PX).floor() as i32,
                        (d.pos.y / TILE_PX).floor() as i32,
                    );
                    tile_allowed_for(scan, v, cid, JobKind::LumberBuild, dt, sim_time)
                })
                .min_by(|a, b| {
                    a.pos
                        .distance_squared(v.pos)
                        .total_cmp(&b.pos.distance_squared(v.pos))
                })
            {
                let tx = (drop.pos.x / TILE_PX).floor() as i32;
                let ty = (drop.pos.y / TILE_PX).floor() as i32;
                set_tracked_target(v, (tx, ty), false, sim_time);
                return;
            }
            if let Some(tree) =
                nearest_allowed_tile(scan, v, cid, JobKind::LumberBuild, &scan.trees, sim_time)
            {
                set_tracked_target(v, tree, true, sim_time);
            }
        }
        JobKind::Idle => {
            let ox = rng.gen_range(-4..=4);
            let oy = rng.gen_range(-4..=4);
            if let Some(tile) =
                find_safe_floor_near(world, scan.center.0 + ox, scan.center.1 + oy, 3)
            {
                set_villager_target(v, tile, false);
            }
        }
    }
}
