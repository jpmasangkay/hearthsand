use macroquad::prelude::*;

use crate::world::{Material, World, TILE_PX};

use super::pathfinding::{
    find_next_trench_dig_tile, is_near_hazard, is_trench_diggable_cell, is_walkable_for_villager,
    nearest_tile_to, BLACKLIST_DURATION_SECS,
};
use super::utility_ai::{
    assign_flee_target, assign_job_target, is_job_target_still_valid, pick_best_job,
};
use super::{
    clamp_world_pos, tile_center, velocity_to_dir, Agents, ColonyState, ItemKind, JobKind,
    ThoughtIcon, VillageScan, Villager, WorldCmd, BUCKET_CAPACITY, BUILD_WOOD_COST,
    FLEE_HAZARD_RADIUS, UTILITY_INTERVAL,
};

impl Agents {
    pub(super) fn update_villager_decisions(
        &mut self,
        world: &World,
        scan: &VillageScan,
        active_wolves: &[Vec2],
        dt: f32,
    ) {
        let colony = ColonyState {
            wood_stored: self.wood_stored,
            food_stored: self.food_stored,
            villager_count: self.villagers.len(),
            total_crops_harvested: self.total_crops_harvested,
            sim_time: self.sim_time,
        };

        let sim_time = self.sim_time;
        for v in &mut self.villagers {
            v.prune_blacklist(sim_time);
            // Tick status timers & needs
            v.action_tween = (v.action_tween - dt).max(0.0);
            v.hit_flash = (v.hit_flash - dt).max(0.0);
            v.thought_timer = (v.thought_timer - dt).max(0.0);
            if v.thought_timer <= 0.0 {
                v.thought = v.job.thought_icon();
            }

            v.hunger = (v.hunger - dt * 0.008).max(0.0);
            let near_lit_fire = scan.lit_campfires.iter().any(|&(fx, fy)| {
                let fp = tile_center(fx, fy);
                v.pos.distance(fp) <= 3.2 * TILE_PX
            });
            if near_lit_fire {
                v.warmth = (v.warmth + dt * 0.25).min(1.0);
            } else if world.is_night() {
                v.warmth = (v.warmth - dt * 0.025).max(0.0);
            } else {
                v.warmth = (v.warmth + dt * 0.015).min(0.95);
            }

            // Invalidate A* path whenever terrain changed since the path was computed
            if v.path_terrain_tick != world.last_terrain_change_tick {
                if !v.path.is_empty() {
                    v.path.clear();
                    v.path_idx = 0;
                    v.needs_path = true;
                }
                v.path_terrain_tick = world.last_terrain_change_tick;
            }

            // Check urgent FleeHazard override between ticks if danger pops right next to villager
            let ignore_fire = v.job == JobKind::Extinguish;
            let hazard_close =
                is_near_hazard(world, v.pos, active_wolves, FLEE_HAZARD_RADIUS, ignore_fire);
            if hazard_close && v.job != JobKind::FleeHazard {
                v.job = JobKind::FleeHazard;
                v.thought = Some(ThoughtIcon::Warning);
                v.thought_timer = 0.6;
                v.work_timer = 0.0;
                assign_flee_target(world, scan, active_wolves, v);
                continue;
            }

            v.decision_timer -= dt;
            if v.decision_timer <= 0.0 {
                v.decision_timer += UTILITY_INTERVAL;
                let chosen = pick_best_job(world, scan, v, active_wolves, colony, &self.drops);
                if chosen != v.job {
                    v.job = chosen;
                    v.work_timer = 0.0;
                    v.target_tile = None;
                    v.path.clear();
                    v.path_idx = 0;
                    v.thought = chosen.thought_icon();
                }

                // Ensure valid, non-blacklisted job target
                let target_blacklisted = v
                    .target_tile
                    .is_some_and(|t| v.is_blacklisted(v.job, t, sim_time));
                if v.target_tile.is_none()
                    || target_blacklisted
                    || !is_job_target_still_valid(world, scan, v, &self.drops)
                {
                    assign_job_target(
                        world,
                        scan,
                        active_wolves,
                        v,
                        colony,
                        &self.drops,
                        &mut self.rng,
                    );
                }
            }
        }
    }

    pub(super) fn step_villagers(
        &mut self,
        world: &World,
        scan: &VillageScan,
        active_wolves: &[Vec2],
        dt: f32,
        cmds: &mut Vec<WorldCmd>,
    ) {
        let mut pending_drops: Vec<(ItemKind, (i32, i32))> = Vec::new();
        let mut colony = ColonyState {
            wood_stored: self.wood_stored,
            food_stored: self.food_stored,
            villager_count: self.villagers.len(),
            total_crops_harvested: self.total_crops_harvested,
            sim_time: self.sim_time,
        };

        for i in 0..self.villagers.len() {
            // Check standing tile hazard damage or wolf bite
            let vpos = self.villagers[i].pos;
            let vtx = (vpos.x / TILE_PX).floor() as i32;
            let vty = (vpos.y / TILE_PX).floor() as i32;
            let on_hazard = world.get(vtx, vty).is_some_and(|c| {
                matches!(c.material, Material::Fire | Material::Lava | Material::Acid)
            });
            let wolf_close = world.is_night()
                && active_wolves
                    .iter()
                    .any(|&wp| wp.distance(vpos) <= TILE_PX * 1.15);

            if self.villagers[i].hp <= 0.0 {
                continue;
            }
            if on_hazard || wolf_close {
                self.villagers[i].hp -= dt * if on_hazard { 22.0 } else { 14.0 };
                self.villagers[i].hit_flash = 0.22;
                self.villagers[i].action_tween = 0.22;
                if self.villagers[i].hp <= 0.0 {
                    continue;
                }
            } else if self.villagers[i].hunger > 0.4 && self.villagers[i].warmth > 0.4 {
                self.villagers[i].hp = (self.villagers[i].hp + dt * 1.5).min(100.0);
            }

            // Pick up nearby uncarried drops cleanly (18px covers full 16x16 tile from arrival offset)
            if self.villagers[i].carrying.is_none() {
                if let Some(d_idx) = self
                    .drops
                    .iter()
                    .position(|d| d.height <= 3.0 && d.pos.distance(vpos) <= 18.0)
                {
                    let drop = self.drops.swap_remove(d_idx);
                    self.villagers[i].carrying = Some(drop.kind);
                    self.villagers[i].action_tween = 0.22;
                }
            }

            // Move along A* path or execute target work action
            let v = &mut self.villagers[i];
            if v.work_timer > 0.0 {
                v.work_timer -= dt;
                v.vel = Vec2::ZERO;
                if v.work_timer > 0.0 {
                    continue;
                }
            }

            // Follow path waypoints
            let speed = match v.job {
                JobKind::FleeHazard => 62.0,
                JobKind::Extinguish => 48.0,
                _ => 38.0,
            };

            let mut arrived = false;
            if let Some((gx, gy)) = v.target_tile {
                let d_tile = (vtx - gx).abs() + (vty - gy).abs();
                let dist_center = v.pos.distance(tile_center(gx, gy));
                if (v.target_adjacent && d_tile == 1) || (!v.target_adjacent && dist_center <= 5.5)
                {
                    arrived = true;
                }
            }

            if !arrived {
                if v.path_idx < v.path.len() {
                    let wp = v.path[v.path_idx];
                    let wp_px = tile_center(wp.0, wp.1);
                    let delta = wp_px - v.pos;
                    let dist = delta.length();
                    if dist <= 3.5 {
                        v.path_idx += 1;
                        v.vel = Vec2::ZERO;
                    } else {
                        let dir_vec = delta / dist;
                        let step_pos = clamp_world_pos(v.pos + dir_vec * speed * dt);
                        if is_walkable_for_villager(world, step_pos, v.job == JobKind::FleeHazard) {
                            v.pos = step_pos;
                            v.vel = dir_vec * speed;
                            v.dir = velocity_to_dir(v.vel);
                            v.anim_time += dt * 8.0;
                        } else {
                            v.vel = Vec2::ZERO;
                            v.needs_path = true;
                        }
                    }
                } else if v.target_tile.is_some() && !v.needs_path {
                    // Direct short-step nudge when path has ended 1 step away
                    if let Some((gx, gy)) = v.target_tile {
                        let target_px = tile_center(gx, gy);
                        let delta = target_px - v.pos;
                        let dist = delta.length();
                        if (!v.target_adjacent && dist > 4.0)
                            || (v.target_adjacent && dist > TILE_PX * 1.15)
                        {
                            let dir_vec = delta / dist.max(0.001);
                            let step_pos = clamp_world_pos(v.pos + dir_vec * speed * dt);
                            if is_walkable_for_villager(
                                world,
                                step_pos,
                                v.job == JobKind::FleeHazard,
                            ) {
                                v.pos = step_pos;
                                v.vel = dir_vec * speed;
                                v.dir = velocity_to_dir(v.vel);
                                v.anim_time += dt * 8.0;
                            } else {
                                if !matches!(v.job, JobKind::Idle | JobKind::FleeHazard) {
                                    v.blacklist_target(
                                        v.job,
                                        (gx, gy),
                                        colony.sim_time + BLACKLIST_DURATION_SECS,
                                    );
                                }
                                v.target_tile = None;
                            }
                        } else {
                            arrived = true;
                        }
                    }
                } else {
                    v.vel = Vec2::ZERO;
                }
            }

            if arrived {
                v.vel = Vec2::ZERO;
                v.anim_time = 0.0;
                if let Some(target) = v.target_tile {
                    v.dir = velocity_to_dir(tile_center(target.0, target.1) - v.pos);
                    execute_arrived_action(
                        world,
                        scan,
                        v,
                        target,
                        &mut colony,
                        &mut pending_drops,
                        cmds,
                    );
                }
            }
        }

        self.wood_stored = colony.wood_stored;
        self.food_stored = colony.food_stored;
        self.total_crops_harvested = colony.total_crops_harvested;

        // Spill carried water & items before removing dead villagers so water/goods are conserved
        for v in &self.villagers {
            if v.hp <= 0.0 {
                let vtx = (v.pos.x / TILE_PX).floor() as i32;
                let vty = (v.pos.y / TILE_PX).floor() as i32;
                if v.bucket_water > 0 {
                    cmds.push(WorldCmd::SplashExtinguish {
                        x: vtx,
                        y: vty,
                        water: v.bucket_water,
                    });
                }
                if let Some(kind) = v.carrying {
                    pending_drops.push((kind, (vtx, vty)));
                }
            }
        }
        self.villagers.retain(|v| v.hp > 0.0);

        for (kind, tile) in pending_drops {
            self.spawn_drop(kind, tile);
        }
    }
}

fn execute_arrived_action(
    world: &World,
    scan: &VillageScan,
    v: &mut Villager,
    target: (i32, i32),
    colony: &mut ColonyState,
    pending_drops: &mut Vec<(ItemKind, (i32, i32))>,
    cmds: &mut Vec<WorldCmd>,
) {
    let (tx, ty) = target;
    let villager_count = colony.villager_count;
    let wood_stored = &mut colony.wood_stored;
    let food_stored = &mut colony.food_stored;
    let total_crops_harvested = &mut colony.total_crops_harvested;
    let Some(cell) = world.get(tx, ty) else {
        v.target_tile = None;
        return;
    };

    match v.job {
        JobKind::FleeHazard | JobKind::Idle => {
            v.target_tile = None;
        }
        JobKind::Warm => {
            v.thought = Some(if v.warmth > 0.75 {
                ThoughtIcon::Heart
            } else {
                ThoughtIcon::Flame
            });
            v.thought_timer = 0.6;
        }
        JobKind::Extinguish => {
            if v.bucket_water == 0 && cell.material.is_water() && cell.water_volume > 1 {
                let scoop = BUCKET_CAPACITY.min(cell.water_volume - 1);
                v.bucket_water = scoop;
                v.action_tween = 0.30;
                v.work_timer = 0.25;
                v.thought = Some(ThoughtIcon::Bucket);
                v.thought_timer = 0.8;
                v.target_tile = None;
                cmds.push(WorldCmd::ScoopWater {
                    x: tx,
                    y: ty,
                    amount: scoop,
                });
            } else if v.bucket_water > 0 && cell.material == Material::Fire {
                let water = v.bucket_water;
                v.bucket_water = 0;
                v.action_tween = 0.30;
                v.work_timer = 0.25;
                v.target_tile = None;
                cmds.push(WorldCmd::SplashExtinguish {
                    x: tx,
                    y: ty,
                    water,
                });
            } else {
                v.target_tile = None;
            }
        }
        JobKind::Eat => {
            if cell.material == Material::Chest && *food_stored > 0 {
                *food_stored -= 1;
                v.hunger = (v.hunger + 0.65).min(1.0);
                v.action_tween = 0.28;
                v.work_timer = 0.4;
                v.thought = Some(ThoughtIcon::Heart);
                v.thought_timer = 1.0;
            } else if cell.material == Material::BerryBush && cell.has_berries() {
                v.hunger = (v.hunger + 0.50).min(1.0);
                v.action_tween = 0.28;
                v.work_timer = 0.4;
                v.thought = Some(ThoughtIcon::Heart);
                v.thought_timer = 1.0;
                pending_drops.push((ItemKind::Berry, (tx, ty)));
                cmds.push(WorldCmd::HarvestBerry { x: tx, y: ty });
            } else if matches!(cell.material, Material::Crop(s) if s >= 4) {
                v.hunger = (v.hunger + 0.55).min(1.0);
                v.action_tween = 0.28;
                v.work_timer = 0.4;
                v.thought = Some(ThoughtIcon::Heart);
                v.thought_timer = 1.0;
                cmds.push(WorldCmd::HarvestCrop { x: tx, y: ty });
            }
            v.target_tile = None;
        }
        JobKind::HarvestHaul => {
            if cell.material == Material::Chest
                && matches!(v.carrying, Some(ItemKind::Crop | ItemKind::Berry))
            {
                v.carrying = None;
                *food_stored += 2;
                *total_crops_harvested += 1;
                v.action_tween = 0.25;
                v.work_timer = 0.25;
                v.thought = Some(ThoughtIcon::Heart);
                v.thought_timer = 0.8;
                v.target_tile = None;
            } else if matches!(cell.material, Material::Crop(s) if s >= 4) {
                v.action_tween = 0.30;
                v.work_timer = 0.35;
                if v.carrying.is_none() {
                    v.carrying = Some(ItemKind::Crop);
                } else {
                    pending_drops.push((ItemKind::Crop, (tx, ty)));
                }
                cmds.push(WorldCmd::HarvestCrop { x: tx, y: ty });
                v.target_tile = None;
            } else {
                if !v.target_adjacent && v.carrying.is_none() {
                    v.blacklist_target(
                        JobKind::HarvestHaul,
                        (tx, ty),
                        colony.sim_time + BLACKLIST_DURATION_SECS,
                    );
                }
                v.target_tile = None;
            }
        }
        JobKind::FarmIrrigate => {
            v.action_tween = 0.30;
            v.work_timer = 0.40;
            let adj_water_or_trench = [(0, -1), (1, 0), (0, 1), (-1, 0)].iter().any(|&(dx, dy)| {
                world.get(tx + dx, ty + dy).is_some_and(|c| {
                    c.material == Material::Trench
                        || (c.material.is_water() && c.water_volume >= 10)
                })
            });
            let should_dig_trench = !scan.dry_tilled.is_empty()
                && is_trench_diggable_cell(cell)
                && (adj_water_or_trench
                    || nearest_tile_to(v.pos, &scan.dry_tilled).is_some_and(|dry| {
                        find_next_trench_dig_tile(world, scan, dry) == Some((tx, ty))
                    }));
            if should_dig_trench {
                cmds.push(WorldCmd::SetMaterial {
                    x: tx,
                    y: ty,
                    material: Material::Trench,
                });
                v.target_tile = None;
                return;
            }
            if cell.material == Material::TilledSoil
                || (cell.material == Material::WetSoil && cell.is_tilled())
            {
                cmds.push(WorldCmd::SetMaterial {
                    x: tx,
                    y: ty,
                    material: Material::Crop(1),
                });
            } else if matches!(cell.material, Material::Dirt | Material::Grass)
                || (cell.material == Material::WetSoil && !cell.is_tilled())
            {
                cmds.push(WorldCmd::SetMaterial {
                    x: tx,
                    y: ty,
                    material: Material::TilledSoil,
                });
            } else {
                v.blacklist_target(
                    JobKind::FarmIrrigate,
                    (tx, ty),
                    colony.sim_time + BLACKLIST_DURATION_SECS,
                );
            }
            v.target_tile = None;
        }
        JobKind::LumberBuild => {
            let desired_cabins = (villager_count / 4).max(1);
            if cell.material == Material::Campfire && !cell.is_lit_campfire() {
                v.action_tween = 0.30;
                v.work_timer = 0.35;
                cmds.push(WorldCmd::Ignite {
                    x: tx,
                    y: ty,
                    temp: 220.0,
                });
                v.target_tile = None;
                return;
            }
            if cell.material == Material::Chest && v.carrying == Some(ItemKind::Wood) {
                v.carrying = None;
                *wood_stored += 2;
                v.action_tween = 0.25;
                v.work_timer = 0.25;
                v.target_tile = None;
                return;
            }
            if cell.material == Material::Tree {
                v.action_tween = 0.30;
                v.work_timer = 0.32;
                v.chop_hits += 1;
                if v.chop_hits >= 3 {
                    v.chop_hits = 0;
                    if v.carrying.is_none() {
                        v.carrying = Some(ItemKind::Wood);
                    } else {
                        pending_drops.push((ItemKind::Wood, (tx, ty)));
                    }
                    cmds.push(WorldCmd::SetMaterial {
                        x: tx,
                        y: ty,
                        material: Material::Grass,
                    });
                    v.target_tile = None;
                }
                return;
            }
            if matches!(cell.material, Material::Dirt | Material::Grass) {
                let to_build = if scan.chests.is_empty() {
                    Some(Material::Chest)
                } else if scan.lit_campfires.is_empty() {
                    Some(Material::Campfire)
                } else if scan.cabins.len() < desired_cabins {
                    Some(Material::Cabin)
                } else {
                    None
                };
                if let Some(structure) = to_build {
                    if *wood_stored >= BUILD_WOOD_COST {
                        *wood_stored -= BUILD_WOOD_COST;
                        v.action_tween = 0.30;
                        v.work_timer = 0.45;
                        cmds.push(WorldCmd::SetMaterial {
                            x: tx,
                            y: ty,
                            material: structure,
                        });
                    } else if v.carrying == Some(ItemKind::Wood)
                        && *wood_stored + 2 >= BUILD_WOOD_COST
                    {
                        v.carrying = None;
                        *wood_stored = (*wood_stored + 2).saturating_sub(BUILD_WOOD_COST);
                        v.action_tween = 0.30;
                        v.work_timer = 0.45;
                        cmds.push(WorldCmd::SetMaterial {
                            x: tx,
                            y: ty,
                            material: structure,
                        });
                    }
                }
            }
            v.target_tile = None;
        }
    }
}
