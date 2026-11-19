use std::collections::{BinaryHeap, VecDeque};

use macroquad::prelude::*;

use crate::world::{Cell, Material, World, GRID_H, GRID_W, TILE_PX};

use super::{
    tile_center, tile_dist_sq, Agents, AstarScratch, JobKind, OpenNode, VillageScan, Villager,
    ASTAR_MAX_EXPANSIONS, MAX_ASTAR_PER_FRAME, VILLAGE_SCAN_RADIUS,
};

pub(super) const BLACKLIST_DURATION_SECS: f32 = 30.0;

impl Agents {
    pub(super) fn compute_paths_budgeted(&mut self, world: &World) {
        let len = self.villagers.len();
        self.last_frame_astar_count = 0;
        if len == 0 {
            return;
        }

        let start = self.astar_cursor % len;
        let mut checked = 0usize;

        while checked < len && self.last_frame_astar_count < MAX_ASTAR_PER_FRAME {
            let idx = (start + checked) % len;
            checked += 1;

            if !self.villagers[idx].needs_path {
                continue;
            }
            self.villagers[idx].needs_path = false;

            let Some(goal) = self.villagers[idx].target_tile else {
                continue;
            };
            let vtx = (self.villagers[idx].pos.x / TILE_PX).floor() as i32;
            let vty = (self.villagers[idx].pos.y / TILE_PX).floor() as i32;
            let adj = self.villagers[idx].target_adjacent;
            let job = self.villagers[idx].job;
            let allow_hazards = job == JobKind::FleeHazard;

            self.last_frame_astar_count += 1;
            let path = find_path_astar(
                world,
                &mut self.astar_scratch,
                (vtx, vty),
                goal,
                adj,
                allow_hazards,
            );
            let already_there = if adj {
                (vtx - goal.0).abs() + (vty - goal.1).abs() == 1
            } else {
                vtx == goal.0 && vty == goal.1
            };
            if path.is_empty()
                && !already_there
                && !matches!(job, JobKind::Idle | JobKind::FleeHazard)
            {
                let expire = self.sim_time + BLACKLIST_DURATION_SECS;
                self.villagers[idx].blacklist_target(job, goal, expire);
                self.villagers[idx].target_tile = None;
                self.villagers[idx].path.clear();
                self.villagers[idx].path_idx = 0;
            } else {
                self.villagers[idx].path = path;
                self.villagers[idx].path_idx = 0;
            }
            self.villagers[idx].path_terrain_tick = world.last_terrain_change_tick;
        }

        self.astar_cursor = (start + checked) % len;
    }
}

/// Flood-fills passable tiles inside the village scan region from each villager (or the village
/// center if no villagers exist yet), assigning each connected component a non-zero ID.
pub(super) fn compute_reachability(world: &World, villagers: &[Villager], comp: &mut [u16]) {
    if comp.len() != GRID_W * GRID_H {
        return;
    }
    comp.fill(0);
    let center = world.village_center();
    let pad = VILLAGE_SCAN_RADIUS + 6;
    let min_x = (center.0 - pad).max(0);
    let max_x = (center.0 + pad).min(GRID_W as i32 - 1);
    let min_y = (center.1 - pad).max(0);
    let max_y = (center.1 + pad).min(GRID_H as i32 - 1);

    let mut seeds: Vec<(i32, i32)> = Vec::with_capacity(villagers.len().max(1));
    for v in villagers {
        let vx = (v.pos.x / TILE_PX).floor() as i32;
        let vy = (v.pos.y / TILE_PX).floor() as i32;
        if world
            .get(vx, vy)
            .is_some_and(|c| is_tile_passable(c, false))
        {
            seeds.push((vx, vy));
        } else {
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let nx = vx + dx;
                let ny = vy + dy;
                if world
                    .get(nx, ny)
                    .is_some_and(|c| is_tile_passable(c, false))
                {
                    seeds.push((nx, ny));
                    break;
                }
            }
        }
    }
    if seeds.is_empty() {
        if let Some(pos) = find_walkable_spawn(world, center.0, center.1, 8) {
            seeds.push((
                (pos.x / TILE_PX).floor() as i32,
                (pos.y / TILE_PX).floor() as i32,
            ));
        }
    }

    let mut queue = VecDeque::new();
    let mut next_cid = 0_u16;
    for (sx, sy) in seeds {
        if sx < min_x || sx > max_x || sy < min_y || sy > max_y {
            continue;
        }
        let sidx = sy as usize * GRID_W + sx as usize;
        if comp[sidx] != 0 {
            continue;
        }
        next_cid = next_cid.saturating_add(1).max(1);
        comp[sidx] = next_cid;
        queue.push_back((sx, sy));
        while let Some((cx, cy)) = queue.pop_front() {
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let nx = cx + dx;
                let ny = cy + dy;
                if nx < min_x || nx > max_x || ny < min_y || ny > max_y {
                    continue;
                }
                let nidx = ny as usize * GRID_W + nx as usize;
                if comp[nidx] != 0 {
                    continue;
                }
                if world
                    .get(nx, ny)
                    .is_some_and(|c| is_tile_passable(c, false))
                {
                    comp[nidx] = next_cid;
                    queue.push_back((nx, ny));
                }
            }
        }
    }
}

#[inline]
pub(super) fn villager_component(comp: &[u16], pos: Vec2) -> u16 {
    if comp.len() != GRID_W * GRID_H {
        return 0;
    }
    let vx = (pos.x / TILE_PX).floor() as i32;
    let vy = (pos.y / TILE_PX).floor() as i32;
    if World::in_bounds(vx, vy) {
        let c = comp[vy as usize * GRID_W + vx as usize];
        if c != 0 {
            return c;
        }
    }
    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        let nx = vx + dx;
        let ny = vy + dy;
        if World::in_bounds(nx, ny) {
            let c = comp[ny as usize * GRID_W + nx as usize];
            if c != 0 {
                return c;
            }
        }
    }
    0
}

#[inline]
pub(super) fn is_tile_reachable_any(comp: &[u16], x: i32, y: i32) -> bool {
    if comp.len() != GRID_W * GRID_H {
        return true;
    }
    if World::in_bounds(x, y) && comp[y as usize * GRID_W + x as usize] != 0 {
        return true;
    }
    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        let nx = x + dx;
        let ny = y + dy;
        if World::in_bounds(nx, ny) && comp[ny as usize * GRID_W + nx as usize] != 0 {
            return true;
        }
    }
    false
}

#[inline]
pub(super) fn is_tile_reachable_by(comp: &[u16], cid: u16, x: i32, y: i32) -> bool {
    if cid == 0 || comp.len() != GRID_W * GRID_H {
        return true;
    }
    if World::in_bounds(x, y) && comp[y as usize * GRID_W + x as usize] == cid {
        return true;
    }
    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
        let nx = x + dx;
        let ny = y + dy;
        if World::in_bounds(nx, ny) && comp[ny as usize * GRID_W + nx as usize] == cid {
            return true;
        }
    }
    false
}

pub(super) fn find_next_trench_dig_tile(
    world: &World,
    scan: &VillageScan,
    dry_soil: (i32, i32),
) -> Option<(i32, i32)> {
    // 0-1 BFS outward from dry_soil: existing Trench tiles cost 0 (push_front),
    // diggable non-trench tiles cost 1 (push_back), until reaching water or a wet trench.
    let mut dist = vec![u16::MAX; GRID_W * GRID_H];
    let mut next_toward_goal = vec![(-1_i32, -1_i32); GRID_W * GRID_H];
    let mut queue = VecDeque::new();

    // Seed from tiles within Chebyshev distance 2 of dry_soil
    for dy in -2_i32..=2 {
        for dx in -2_i32..=2 {
            let sx = dry_soil.0 + dx;
            let sy = dry_soil.1 + dy;
            if !World::in_bounds(sx, sy) {
                continue;
            }
            let Some(c) = world.get(sx, sy) else {
                continue;
            };
            if c.material == Material::Trench {
                if c.water_volume >= 15 {
                    return None;
                }
                let sidx = sy as usize * GRID_W + sx as usize;
                dist[sidx] = 0;
                queue.push_front((sx, sy));
            } else if is_trench_diggable_cell(c) {
                let sidx = sy as usize * GRID_W + sx as usize;
                dist[sidx] = 1;
                queue.push_back((sx, sy));
            }
        }
    }

    let mut steps = 0usize;
    let mut reached_water: Option<(i32, i32)> = None;
    while let Some((cx, cy)) = queue.pop_front() {
        steps += 1;
        if steps > 1400 {
            break;
        }
        let cidx = cy as usize * GRID_W + cx as usize;
        let cur_d = dist[cidx];
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let nx = cx + dx;
            let ny = cy + dy;
            if !World::in_bounds(nx, ny) {
                continue;
            }
            let Some(nc) = world.get(nx, ny) else {
                continue;
            };
            if (nc.material.is_water() && nc.water_volume >= 10)
                || (nc.material == Material::Trench && nc.water_volume >= 18)
            {
                reached_water = Some((cx, cy));
                break;
            }
            let is_trench = nc.material == Material::Trench;
            if !is_trench && !is_trench_diggable_cell(nc) {
                continue;
            }
            let dist_to_center = (nx - scan.center.0).abs().max((ny - scan.center.1).abs());
            if dist_to_center > VILLAGE_SCAN_RADIUS {
                continue;
            }
            let step_cost = if is_trench { 0 } else { 1 };
            let nd = cur_d.saturating_add(step_cost);
            let nidx = ny as usize * GRID_W + nx as usize;
            if nd < dist[nidx] {
                dist[nidx] = nd;
                next_toward_goal[nidx] = (cx, cy);
                if is_trench {
                    queue.push_front((nx, ny));
                } else {
                    queue.push_back((nx, ny));
                }
            }
        }
        if reached_water.is_some() {
            break;
        }
    }

    let mut cur = reached_water?;
    // Walk from the water edge toward dry_soil and return the first non-trench diggable tile
    for _ in 0..80 {
        let cell = world.get(cur.0, cur.1)?;
        if is_trench_diggable_cell(cell) {
            return Some(cur);
        }
        let idx = cur.1 as usize * GRID_W + cur.0 as usize;
        let next = next_toward_goal[idx];
        if next.0 < 0 {
            break;
        }
        cur = next;
    }
    None
}

pub(super) fn is_trench_diggable_cell(cell: &Cell) -> bool {
    matches!(
        cell.material,
        Material::Dirt | Material::Grass | Material::Sand | Material::Ash
    ) || (cell.material == Material::WetSoil && !cell.is_tilled())
}

pub(super) fn has_nearby_structure(world: &World, x: i32, y: i32, radius: i32) -> bool {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if let Some(c) = world.get(x + dx, y + dy) {
                if matches!(
                    c.material,
                    Material::Chest | Material::Cabin | Material::Campfire
                ) {
                    return true;
                }
            }
        }
    }
    false
}

pub(super) fn find_tillable_spot(world: &World, center: (i32, i32)) -> Option<(i32, i32)> {
    for r in 3..=10_i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let x = center.0 + dx;
                let y = center.1 + dy;
                if let Some(cell) = world.get(x, y) {
                    if matches!(cell.material, Material::Dirt | Material::Grass)
                        && has_walkable_neighbor(world, x, y)
                        && !has_nearby_structure(world, x, y, 2)
                        && !is_near_hazard(world, tile_center(x, y), &[], 2, false)
                    {
                        return Some((x, y));
                    }
                }
            }
        }
    }
    None
}

pub(super) fn find_build_spot(world: &World, center: (i32, i32)) -> Option<(i32, i32)> {
    for r in 2..=7_i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let x = center.0 + dx;
                let y = center.1 + dy;
                if let Some(cell) = world.get(x, y) {
                    if matches!(cell.material, Material::Dirt | Material::Grass)
                        && !has_nearby_water(world, x, y, 0)
                        && has_walkable_neighbor(world, x, y)
                    {
                        return Some((x, y));
                    }
                }
            }
        }
    }
    None
}

fn find_path_astar(
    world: &World,
    scratch: &mut AstarScratch,
    start: (i32, i32),
    goal: (i32, i32),
    adjacent: bool,
    allow_hazards: bool,
) -> Vec<(i32, i32)> {
    if !World::in_bounds(start.0, start.1) || !World::in_bounds(goal.0, goal.1) {
        return Vec::new();
    }

    let is_goal = |x: i32, y: i32| {
        if adjacent {
            (x - goal.0).abs() + (y - goal.1).abs() == 1
        } else {
            x == goal.0 && y == goal.1
        }
    };

    if is_goal(start.0, start.1) {
        return Vec::new();
    }

    let stamp = scratch.next_stamp();
    let start_idx = start.1 as usize * GRID_W + start.0 as usize;
    scratch.closed[start_idx] = stamp;
    scratch.g_score[start_idx] = 0;
    scratch.came_from[start_idx] = -1;

    let mut open = BinaryHeap::new();
    open.push(OpenNode {
        f: manhattan_cost(start, goal),
        g: 0,
        idx: start_idx,
    });

    let mut best_goal_idx: Option<usize> = None;
    let mut expansions = 0usize;

    while let Some(cur) = open.pop() {
        let cx = (cur.idx % GRID_W) as i32;
        let cy = (cur.idx / GRID_W) as i32;
        if is_goal(cx, cy) {
            best_goal_idx = Some(cur.idx);
            break;
        }
        expansions += 1;
        if expansions >= ASTAR_MAX_EXPANSIONS {
            break;
        }
        if cur.g > scratch.g_score[cur.idx] {
            continue;
        }

        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let nx = cx + dx;
            let ny = cy + dy;
            if !World::in_bounds(nx, ny) {
                continue;
            }
            let Some(ncell) = world.get(nx, ny) else {
                continue;
            };
            if !is_tile_passable(ncell, allow_hazards) {
                continue;
            }
            let nidx = ny as usize * GRID_W + nx as usize;
            let step_cost = match ncell.material {
                Material::Trench => 14,
                Material::Crop(_) => 13,
                Material::Fire | Material::Lava | Material::Acid => 35,
                _ => 10,
            };
            let tentative_g = cur.g.saturating_add(step_cost);
            if scratch.closed[nidx] != stamp || tentative_g < scratch.g_score[nidx] {
                scratch.closed[nidx] = stamp;
                scratch.g_score[nidx] = tentative_g;
                scratch.came_from[nidx] = cur.idx as i32;
                open.push(OpenNode {
                    f: tentative_g.saturating_add(manhattan_cost((nx, ny), goal)),
                    g: tentative_g,
                    idx: nidx,
                });
            }
        }
    }

    let mut path = Vec::new();
    let mut cur = match best_goal_idx {
        Some(idx) => idx as i32,
        None => return path,
    };
    while cur >= 0 && cur as usize != start_idx {
        let ux = (cur as usize % GRID_W) as i32;
        let uy = (cur as usize / GRID_W) as i32;
        path.push((ux, uy));
        cur = scratch.came_from[cur as usize];
        if path.len() > GRID_W * 2 {
            break;
        }
    }
    path.reverse();
    path
}

fn manhattan_cost(a: (i32, i32), b: (i32, i32)) -> u32 {
    ((a.0 - b.0).abs() + (a.1 - b.1).abs()) as u32 * 10
}

pub(super) fn is_tile_passable(cell: &Cell, allow_hazards: bool) -> bool {
    if cell.material.is_solid_obstacle() || cell.material.is_water() {
        return false;
    }
    if !allow_hazards
        && matches!(
            cell.material,
            Material::Fire | Material::Lava | Material::Acid
        )
    {
        return false;
    }
    true
}

pub(super) fn is_near_hazard(
    world: &World,
    pos: Vec2,
    active_wolves: &[Vec2],
    radius_tiles: i32,
    ignore_fire: bool,
) -> bool {
    let tx = (pos.x / TILE_PX).floor() as i32;
    let ty = (pos.y / TILE_PX).floor() as i32;
    for dy in -radius_tiles..=radius_tiles {
        for dx in -radius_tiles..=radius_tiles {
            if let Some(cell) = world.get(tx + dx, ty + dy) {
                let is_fire = cell.material == Material::Fire;
                if matches!(cell.material, Material::Lava | Material::Acid)
                    || (is_fire && (!ignore_fire || (dx == 0 && dy == 0)))
                {
                    return true;
                }
            }
        }
    }
    if world.is_night() {
        let max_px = radius_tiles as f32 * TILE_PX;
        if active_wolves.iter().any(|&wp| wp.distance(pos) <= max_px) {
            return true;
        }
    }
    false
}

pub(super) fn has_walkable_neighbor(world: &World, x: i32, y: i32) -> bool {
    [(0, -1), (1, 0), (0, 1), (-1, 0)].iter().any(|&(dx, dy)| {
        world.get(x + dx, y + dy).is_some_and(|c| {
            !c.material.is_solid_obstacle()
                && !c.material.is_water()
                && !matches!(c.material, Material::Fire | Material::Lava | Material::Acid)
        })
    })
}

pub(super) fn has_nearby_water(world: &World, x: i32, y: i32, radius: i32) -> bool {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if let Some(c) = world.get(x + dx, y + dy) {
                if c.water_volume >= 10 {
                    return true;
                }
            }
        }
    }
    false
}

pub(super) fn find_safe_floor_near(
    world: &World,
    cx: i32,
    cy: i32,
    max_r: i32,
) -> Option<(i32, i32)> {
    for r in 0..=max_r {
        for dy in -r..=r {
            for dx in -r..=r {
                let x = cx + dx;
                let y = cy + dy;
                if let Some(c) = world.get(x, y) {
                    if is_tile_passable(c, false) {
                        return Some((x, y));
                    }
                }
            }
        }
    }
    None
}

pub(super) fn find_walkable_spawn(world: &World, cx: i32, cy: i32, max_r: i32) -> Option<Vec2> {
    find_safe_floor_near(world, cx, cy, max_r).map(|(x, y)| tile_center(x, y))
}

pub(super) fn nearest_tile_to(pos: Vec2, candidates: &[(i32, i32)]) -> Option<(i32, i32)> {
    candidates
        .iter()
        .copied()
        .min_by_key(|&(x, y)| tile_dist_sq(pos, x, y))
}

pub(super) fn find_nearest_pos(
    pos: Vec2,
    candidates: &[(i32, i32)],
    max_dist_px: f32,
) -> Option<Vec2> {
    let max_d2 = max_dist_px * max_dist_px;
    let mut best: Option<(f32, Vec2)> = None;
    for &(x, y) in candidates {
        let tp = tile_center(x, y);
        let d2 = tp.distance_squared(pos);
        if d2 <= max_d2 && best.is_none_or(|(bd, _)| d2 < bd) {
            best = Some((d2, tp));
        }
    }
    best.map(|(_, p)| p)
}

pub(super) fn find_nearest_crop_or_bush(
    world: &World,
    scan: &VillageScan,
    pos: Vec2,
    radius_tiles: i32,
) -> Option<(i32, i32)> {
    let max_px = radius_tiles as f32 * TILE_PX;
    let mut best: Option<(i32, i32)> = None;
    let mut best_d2 = max_px * max_px;

    for &(x, y) in &scan.all_crops {
        if !world
            .get(x, y)
            .is_some_and(|c| matches!(c.material, Material::Crop(s) if s >= 2))
        {
            continue;
        }
        let d2 = tile_center(x, y).distance_squared(pos);
        if d2 < best_d2 {
            best_d2 = d2;
            best = Some((x, y));
        }
    }
    if best.is_some() {
        return best;
    }

    // Fallback: local crop check if rabbit is far from village scan
    let tx = (pos.x / TILE_PX).floor() as i32;
    let ty = (pos.y / TILE_PX).floor() as i32;
    for dy in -10..=10 {
        for dx in -10..=10 {
            let x = tx + dx;
            let y = ty + dy;
            if let Some(c) = world.get(x, y) {
                if matches!(c.material, Material::Crop(s) if s >= 2)
                    || (c.material == Material::BerryBush && c.has_berries())
                {
                    let d2 = tile_center(x, y).distance_squared(pos);
                    if d2 < best_d2 {
                        best_d2 = d2;
                        best = Some((x, y));
                    }
                }
            }
        }
    }
    best
}

pub(super) fn is_walkable_for_villager(world: &World, pos: Vec2, allow_hazards: bool) -> bool {
    let tx = (pos.x / TILE_PX).floor() as i32;
    let ty = (pos.y / TILE_PX).floor() as i32;
    match world.get(tx, ty) {
        Some(cell) => is_tile_passable(cell, allow_hazards),
        None => false,
    }
}
