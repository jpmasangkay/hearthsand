use ::rand::rngs::StdRng;
use ::rand::{Rng, SeedableRng};
use macroquad::prelude::*;

use crate::world::{World, GRID_H, GRID_W, TILE_PX};

mod critters;
mod figures;
mod mage;
mod pathfinding;
mod render;
mod spells;
mod types;
mod utility_ai;
mod villager;

#[cfg(test)]
mod tests;

pub use render::DrawSceneOpts;
pub use types::*;

pub const UTILITY_INTERVAL: f32 = 0.5;
pub const HYSTERESIS_BONUS: f32 = 1.20;
pub const MAX_ASTAR_PER_FRAME: usize = 8;
pub const EXTINGUISH_RADIUS: i32 = 18;
pub const FLEE_HAZARD_RADIUS: i32 = 3;
pub const CAMPFIRE_LIGHT_RADIUS_TILES: f32 = 6.0;
pub const BUCKET_CAPACITY: u8 = 12;
pub const BUILD_WOOD_COST: u32 = 3;
pub const MAGE_WALK_SPEED: f32 = 88.0;
pub const MAGE_DASH_SPEED: f32 = 270.0;
pub const MAGE_DASH_DURATION: f32 = 0.16;
pub const MAGE_DASH_COOLDOWN: f32 = 0.55;
pub(super) const VILLAGE_SCAN_RADIUS: i32 = 38;
pub(super) const ASTAR_MAX_EXPANSIONS: usize = 900;
const ITEM_DROP_GRAVITY: f32 = 280.0;

pub struct Agents {
    pub villagers: Vec<Villager>,
    pub critters: Vec<Critter>,
    pub projectiles: Vec<Projectile>,
    pub drops: Vec<ItemDrop>,
    pub mage_pos: Option<Vec2>,
    pub mage_dir: u8,
    pub mage_active: bool,
    pub mage: Mage,
    pub mage_ghosts: Vec<MageGhost>,
    spells: Vec<SpellBolt>,
    orbit_shards: Vec<OrbitShard>,
    vortices: Vec<GravityField>,
    lightning_arcs: Vec<LightningArc>,
    pub wood_stored: u32,
    pub food_stored: u32,
    pub total_crops_harvested: u32,
    pub last_frame_astar_count: usize,
    pub(super) sim_time: f32,
    reach_comp: Vec<u16>,
    reach_next_recompute: f32,
    next_id: usize,
    rng: StdRng,
    astar_cursor: usize,
    astar_scratch: AstarScratch,
}

impl Agents {
    pub fn new(seed: u64) -> Self {
        Self {
            villagers: Vec::new(),
            critters: Vec::new(),
            projectiles: Vec::new(),
            drops: Vec::new(),
            mage_pos: None,
            mage_dir: 0,
            mage_active: false,
            mage: Mage::default(),
            mage_ghosts: Vec::new(),
            spells: Vec::new(),
            orbit_shards: Vec::new(),
            vortices: Vec::new(),
            lightning_arcs: Vec::new(),
            wood_stored: 4,
            food_stored: 4,
            total_crops_harvested: 0,
            last_frame_astar_count: 0,
            sim_time: 0.0,
            reach_comp: vec![0; GRID_W * GRID_H],
            reach_next_recompute: 0.0,
            next_id: 1,
            rng: StdRng::seed_from_u64(seed.wrapping_add(7919)),
            astar_cursor: 0,
            astar_scratch: AstarScratch::new(),
        }
    }

    #[cfg(test)]
    pub fn sim_time(&self) -> f32 {
        self.sim_time
    }

    pub fn populate_for_world(&mut self, world: &World) {
        let (cx, cy) = world.village_center();
        let spawn = pathfinding::find_walkable_spawn(world, cx, cy + 2, 6)
            .unwrap_or_else(|| tile_center(cx, cy));
        self.mage.pos = spawn;

        // Starter villagers around plaza
        for i in 0..5 {
            let angle = (i as f32 / 5.0) * std::f32::consts::TAU;
            let dist = 2.4 + (i % 2) as f32 * 0.9;
            let tx = cx as f32 + angle.cos() * dist;
            let ty = cy as f32 + angle.sin() * dist;
            if let Some(pos) = pathfinding::find_walkable_spawn(world, tx as i32, ty as i32, 6) {
                self.spawn_villager(pos);
            }
        }

        // Starter rabbits near grass / forest clearings
        for i in 0..4 {
            let angle = i as f32 * 1.57 + 0.5;
            let rx = cx + (angle.cos() * 14.0) as i32;
            let ry = cy + (angle.sin() * 11.0) as i32;
            if let Some(pos) = pathfinding::find_walkable_spawn(world, rx, ry, 8) {
                self.spawn_critter(CritterKind::Rabbit, pos);
            }
        }

        // Starter wolves prowling farther out in the wilds
        for (ox, oy) in [(-28, -20), (26, 22)] {
            if let Some(pos) = pathfinding::find_walkable_spawn(world, cx + ox, cy + oy, 10) {
                self.spawn_critter(CritterKind::Wolf, pos);
            }
        }
    }

    pub fn spawn_villager(&mut self, pos: Vec2) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        let stagger = (id as f32 * 0.13) % UTILITY_INTERVAL;
        self.villagers.push(Villager {
            id,
            pos: clamp_world_pos(pos),
            vel: Vec2::ZERO,
            dir: 0,
            anim_time: 0.0,
            decision_timer: stagger,
            work_timer: 0.0,
            action_tween: 0.0,
            hit_flash: 0.0,
            hunger: 0.85,
            warmth: 0.90,
            hp: 100.0,
            job: JobKind::Idle,
            bucket_water: 0,
            carrying: None,
            thought: None,
            thought_timer: 0.0,
            target_tile: None,
            target_adjacent: false,
            path: Vec::new(),
            path_idx: 0,
            path_terrain_tick: 0,
            needs_path: false,
            chop_hits: 0,
            failed_targets: Vec::new(),
            #[cfg(test)]
            assign_history: Vec::new(),
        });
        id
    }

    pub fn spawn_critter(&mut self, kind: CritterKind, pos: Vec2) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.critters.push(Critter {
            id,
            kind,
            pos: clamp_world_pos(pos),
            vel: Vec2::ZERO,
            dir: 0,
            anim_time: 0.0,
            hop_timer: self.rng.gen_range(0.3..1.4),
            hop_height: 0.0,
            hop_v: 0.0,
            action_tween: 0.0,
            stun_timer: 0.0,
            nibble_cooldown: 0.0,
        });
        id
    }

    pub fn spawn_projectile(
        &mut self,
        kind: ProjectileKind,
        pos: Vec2,
        vel: Vec2,
        ttl: f32,
        radius: i32,
        power: f32,
    ) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.projectiles.push(Projectile {
            id,
            kind,
            pos,
            vel,
            ttl,
            radius,
            power,
        });
        id
    }

    pub fn spawn_drop(&mut self, kind: ItemKind, tile_pos: (i32, i32)) {
        let angle = self.rng.gen_range(0.0..std::f32::consts::TAU);
        let speed = self.rng.gen_range(18.0..38.0);
        let origin = vec2(
            (tile_pos.0 as f32 + 0.5) * TILE_PX,
            (tile_pos.1 as f32 + 0.5) * TILE_PX,
        );
        self.drops.push(ItemDrop {
            kind,
            pos: clamp_world_pos(origin),
            vel: vec2(angle.cos() * speed, angle.sin() * speed),
            height: 3.0,
            v_height: self.rng.gen_range(55.0..85.0),
            age: 0.0,
        });
    }

    pub fn stun_wolves_near(&mut self, world_pos: Vec2, radius_px: f32, duration: f32) {
        let r2 = radius_px * radius_px;
        for c in &mut self.critters {
            if c.kind == CritterKind::Wolf && c.pos.distance_squared(world_pos) <= r2 {
                c.stun_timer = c.stun_timer.max(duration);
                c.vel = Vec2::ZERO;
                c.action_tween = 0.25;
            }
        }
    }

    pub fn toggle_mage_mode(&mut self) -> bool {
        self.mage_active = !self.mage_active;
        self.mage_pos = if self.mage_active {
            Some(self.mage.pos)
        } else {
            None
        };
        self.mage_dir = self.mage.dir;
        self.mage_active
    }

    pub fn active_magic_count(&self) -> usize {
        self.projectiles.len() + self.spells.len() + self.orbit_shards.len() + self.vortices.len()
    }

    pub fn update(&mut self, world: &World, dt: f32) -> Vec<WorldCmd> {
        let mut cmds = Vec::new();

        // Cached flood-fill reachability from each villager (recomputed at most once per sim-second)
        if self.sim_time >= self.reach_next_recompute {
            pathfinding::compute_reachability(world, &self.villagers, &mut self.reach_comp);
            self.reach_next_recompute = self.sim_time + 1.0;
        }
        let scan = VillageScan::scan_with_reachability(world, &self.reach_comp);

        self.step_mage_resources_and_ghosts(dt);
        self.step_drops(world, dt);

        // Snapshot wolf positions before updating villagers
        let active_wolves: Vec<Vec2> = self
            .critters
            .iter()
            .filter(|c| c.kind == CritterKind::Wolf && c.stun_timer <= 0.0)
            .map(|c| c.pos)
            .collect();

        // 1. Utility AI urge scoring (every 0.5s with 20% hysteresis) + path invalidation on terrain change
        self.update_villager_decisions(world, &scan, &active_wolves, dt);

        // 2. Round-robin A* path computation strictly capped at MAX_ASTAR_PER_FRAME = 8
        self.compute_paths_budgeted(world);

        // 3. Execute villager movement, work actions, hazard contact, and item pickup
        self.step_villagers(world, &scan, &active_wolves, dt, &mut cmds);

        // 4. Critters (rabbits hop & nibble crops; wolves prowl at night, flee campfire light, or stay stunned)
        self.step_critters(world, &scan, dt, &mut cmds);

        // 5. Projectiles, wand spells, orbiting runes, gravity vortices, and lightning arcs
        self.step_projectiles(world, dt, &mut cmds);
        self.step_spells(world, dt, &mut cmds);
        self.step_orbit_shards(world, dt, &mut cmds);
        self.step_vortices(dt, &mut cmds);
        self.step_lightning_arcs(dt);

        self.sim_time += dt;
        cmds
    }

    fn step_drops(&mut self, world: &World, dt: f32) {
        for d in &mut self.drops {
            d.age += dt;
            if d.height > 0.0 || d.v_height > 0.0 {
                let next_pos = clamp_world_pos(d.pos + d.vel * dt);
                if is_walkable(world, next_pos) {
                    d.pos = next_pos;
                } else {
                    d.vel = -d.vel * 0.4;
                }
                d.height += d.v_height * dt;
                d.v_height -= ITEM_DROP_GRAVITY * dt;
                if d.height <= 0.0 {
                    d.height = 0.0;
                    if d.v_height < -25.0 {
                        d.v_height = -d.v_height * 0.42;
                        d.vel *= 0.45;
                    } else {
                        d.v_height = 0.0;
                        d.vel = Vec2::ZERO;
                    }
                }
            }
        }
        // Expire very old uncollected drops after 90s to keep list bounded
        self.drops.retain(|d| d.age < 90.0);
    }

    pub fn find_nearest_entity(&self, world_pos: Vec2, max_dist: f32) -> Option<String> {
        let mut best_dist = max_dist * max_dist;
        let mut label = None;

        for v in &self.villagers {
            let d2 = v.pos.distance_squared(world_pos);
            if d2 <= best_dist {
                best_dist = d2;
                let carry_tag = match (v.carrying, v.bucket_water > 0) {
                    (Some(item), _) => format!(" | {}", item.name()),
                    (None, true) => " | Bucket".to_owned(),
                    (None, false) => String::new(),
                };
                let hp_tag = if v.hp < 99.5 {
                    format!(" HP:{:.0}%", v.hp.max(0.0))
                } else {
                    String::new()
                };
                label = Some(format!(
                    "Villager #{} [{}]{} | H:{:.0}% W:{:.0}%{}",
                    v.id,
                    v.job.name(),
                    carry_tag,
                    v.hunger * 100.0,
                    v.warmth * 100.0,
                    hp_tag,
                ));
            }
        }
        for c in &self.critters {
            let d2 = c.pos.distance_squared(world_pos);
            if d2 <= best_dist {
                best_dist = d2;
                let state = if c.stun_timer > 0.0 { " (Stunned)" } else { "" };
                label = Some(format!("{} #{}{}", c.kind.name(), c.id, state));
            }
        }
        for p in &self.projectiles {
            let d2 = p.pos.distance_squared(world_pos);
            if d2 <= best_dist {
                best_dist = d2;
                label = Some(format!("Projectile #{}", p.id));
            }
        }
        if self.mage_active {
            let d2 = self.mage.pos.distance_squared(world_pos);
            if d2 <= best_dist {
                label = Some(format!(
                    "Mage [{}] | HP:{:.0}% Mana:{:.0}/{:.0}",
                    self.mage.wand.payload.short_name(),
                    self.mage.hp,
                    self.mage.wand.mana,
                    self.mage.wand.max_mana
                ));
            }
        }
        label
    }
}

pub(super) fn tile_dist_sq(pos: Vec2, tx: i32, ty: i32) -> i32 {
    let p = tile_center(tx, ty);
    let dx = (pos.x - p.x) as i32;
    let dy = (pos.y - p.y) as i32;
    dx * dx + dy * dy
}

pub(super) fn tile_center(tx: i32, ty: i32) -> Vec2 {
    vec2((tx as f32 + 0.5) * TILE_PX, (ty as f32 + 0.5) * TILE_PX)
}

pub(super) fn velocity_to_dir(vel: Vec2) -> u8 {
    if vel.x.abs() > vel.y.abs() {
        if vel.x < 0.0 {
            2
        } else {
            3
        }
    } else if vel.y < 0.0 {
        1
    } else {
        0
    }
}

pub(super) fn is_walkable(world: &World, pos: Vec2) -> bool {
    let tx = (pos.x / TILE_PX).floor() as i32;
    let ty = (pos.y / TILE_PX).floor() as i32;
    match world.get(tx, ty) {
        Some(cell) => !cell.material.is_solid_obstacle() && !cell.material.is_water(),
        None => false,
    }
}

pub(super) fn clamp_world_pos(pos: Vec2) -> Vec2 {
    let max_x = GRID_W as f32 * TILE_PX - 8.0;
    let max_y = GRID_H as f32 * TILE_PX - 8.0;
    Vec2::new(pos.x.clamp(8.0, max_x), pos.y.clamp(8.0, max_y))
}
