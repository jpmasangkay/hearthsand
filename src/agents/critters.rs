use ::rand::Rng;
use macroquad::prelude::*;

use crate::world::{World, TILE_PX};

use super::pathfinding::{find_nearest_crop_or_bush, find_nearest_pos};
use super::{
    clamp_world_pos, is_walkable, tile_center, velocity_to_dir, Agents, CritterKind,
    ProjectileKind, VillageScan, WorldCmd, CAMPFIRE_LIGHT_RADIUS_TILES,
};

impl Agents {
    pub(super) fn step_critters(
        &mut self,
        world: &World,
        scan: &VillageScan,
        dt: f32,
        cmds: &mut Vec<WorldCmd>,
    ) {
        let night = world.is_night();

        for c in &mut self.critters {
            c.action_tween = (c.action_tween - dt).max(0.0);
            c.nibble_cooldown = (c.nibble_cooldown - dt).max(0.0);

            // Stun handling (lightning / shock)
            if c.stun_timer > 0.0 {
                c.stun_timer = (c.stun_timer - dt).max(0.0);
                c.vel = Vec2::ZERO;
                c.hop_height = 0.0;
                continue;
            }

            // Rabbit hop vertical arc
            if c.hop_height > 0.0 || c.hop_v > 0.0 {
                c.hop_height += c.hop_v * dt;
                c.hop_v -= 220.0 * dt;
                if c.hop_height <= 0.0 {
                    c.hop_height = 0.0;
                    c.hop_v = 0.0;
                    c.action_tween = 0.14;
                }
            }

            match c.kind {
                CritterKind::Rabbit => {
                    let danger = find_nearest_pos(c.pos, &scan.hazards, 64.0);
                    if let Some(dpos) = danger {
                        let away = (c.pos - dpos).normalize_or_zero();
                        c.vel = away * 48.0;
                        if c.hop_height == 0.0 {
                            c.hop_v = 48.0;
                        }
                    } else if let Some(crop) = find_nearest_crop_or_bush(world, scan, c.pos, 18) {
                        let cpos = tile_center(crop.0, crop.1);
                        let dist = c.pos.distance(cpos);
                        if dist <= TILE_PX * 1.1 && c.nibble_cooldown <= 0.0 {
                            let angle = self.rng.gen_range(0.0..std::f32::consts::TAU);
                            c.vel = vec2(angle.cos(), angle.sin()) * 34.0;
                            c.hop_v = 40.0;
                            c.hop_timer = self.rng.gen_range(1.2..2.0);
                            c.nibble_cooldown = 5.5;
                            c.action_tween = 0.28;
                            c.dir = velocity_to_dir(cpos - c.pos);
                            cmds.push(WorldCmd::NibbleCrop {
                                x: crop.0,
                                y: crop.1,
                            });
                        } else if c.hop_timer <= 0.0 {
                            c.hop_timer = self.rng.gen_range(0.55..1.1);
                            let dir = (cpos - c.pos).normalize_or_zero();
                            c.vel = dir * 36.0;
                            c.hop_v = 42.0;
                        }
                    } else {
                        c.hop_timer -= dt;
                        if c.hop_timer <= 0.0 {
                            c.hop_timer = self.rng.gen_range(0.9..2.1);
                            if self.rng.gen_bool(0.65) {
                                let angle = self.rng.gen_range(0.0..std::f32::consts::TAU);
                                c.vel = vec2(angle.cos(), angle.sin()) * 30.0;
                                c.hop_v = 38.0;
                            } else {
                                c.vel = Vec2::ZERO;
                            }
                        }
                    }
                    c.hop_timer -= dt * 0.5;
                }
                CritterKind::Wolf => {
                    // Check lit campfire light first: wolves always flee campfire light radius!
                    let flee_fire = scan.lit_campfires.iter().find_map(|&(fx, fy)| {
                        let fp = tile_center(fx, fy);
                        let d = c.pos.distance(fp);
                        if d <= CAMPFIRE_LIGHT_RADIUS_TILES * TILE_PX {
                            Some((fp, d))
                        } else {
                            None
                        }
                    });

                    if let Some((fp, _)) = flee_fire {
                        let away = (c.pos - fp).normalize_or_zero();
                        c.vel = if away == Vec2::ZERO {
                            vec2(1.0, 0.0) * 60.0
                        } else {
                            away * 60.0
                        };
                        c.action_tween = 0.16;
                    } else if night {
                        // Night: prowl toward nearest villager (outside campfire light) or village outskirts
                        let target_villager = self
                            .villagers
                            .iter()
                            .filter(|v| {
                                !scan.lit_campfires.iter().any(|&(fx, fy)| {
                                    v.pos.distance(tile_center(fx, fy))
                                        < (CAMPFIRE_LIGHT_RADIUS_TILES - 0.8) * TILE_PX
                                })
                            })
                            .min_by(|a, b| {
                                a.pos
                                    .distance_squared(c.pos)
                                    .total_cmp(&b.pos.distance_squared(c.pos))
                            })
                            .map(|v| v.pos);

                        if let Some(vp) = target_villager {
                            let dir = (vp - c.pos).normalize_or_zero();
                            c.vel = dir * 50.0;
                            if c.pos.distance(vp) <= TILE_PX * 1.25 {
                                c.action_tween = 0.25;
                            }
                        } else {
                            let vcenter = tile_center(scan.center.0, scan.center.1);
                            let dist = c.pos.distance(vcenter);
                            if dist > 12.0 * TILE_PX {
                                let dir = (vcenter - c.pos).normalize_or_zero();
                                c.vel = dir * 44.0;
                            } else {
                                c.hop_timer -= dt;
                                if c.hop_timer <= 0.0 {
                                    c.hop_timer = self.rng.gen_range(0.8..1.8);
                                    let angle = self.rng.gen_range(0.0..std::f32::consts::TAU);
                                    c.vel = vec2(angle.cos(), angle.sin()) * 42.0;
                                }
                            }
                        }
                    } else {
                        // Day: retreat/linger far from village center
                        let vcenter = tile_center(scan.center.0, scan.center.1);
                        let dist = c.pos.distance(vcenter);
                        if dist < 20.0 * TILE_PX {
                            let away = (c.pos - vcenter).normalize_or_zero();
                            c.vel = away * 46.0;
                        } else {
                            c.hop_timer -= dt;
                            if c.hop_timer <= 0.0 {
                                c.hop_timer = self.rng.gen_range(1.4..3.2);
                                if self.rng.gen_bool(0.45) {
                                    let angle = self.rng.gen_range(0.0..std::f32::consts::TAU);
                                    c.vel = vec2(angle.cos(), angle.sin()) * 24.0;
                                } else {
                                    c.vel = Vec2::ZERO;
                                }
                            }
                        }
                    }
                }
            }

            let next = clamp_world_pos(c.pos + c.vel * dt);
            if is_walkable(world, next) {
                c.pos = next;
            } else {
                // Slide along one axis if possible
                let try_x = clamp_world_pos(vec2(c.pos.x + c.vel.x * dt, c.pos.y));
                let try_y = clamp_world_pos(vec2(c.pos.x, c.pos.y + c.vel.y * dt));
                if is_walkable(world, try_x) {
                    c.pos = try_x;
                } else if is_walkable(world, try_y) {
                    c.pos = try_y;
                } else {
                    c.vel = -c.vel;
                }
            }

            if c.vel.length_squared() > 1.0 {
                c.dir = velocity_to_dir(c.vel);
                c.anim_time += dt * 8.0;
            }
        }
    }

    pub(super) fn step_projectiles(&mut self, world: &World, dt: f32, cmds: &mut Vec<WorldCmd>) {
        let mut impacts = Vec::new();
        let mut i = 0;
        while i < self.projectiles.len() {
            let p = &mut self.projectiles[i];
            p.pos += p.vel * dt;
            p.ttl -= dt;

            let tx = (p.pos.x / TILE_PX).floor() as i32;
            let ty = (p.pos.y / TILE_PX).floor() as i32;
            let hit_obstacle = world
                .get(tx, ty)
                .is_none_or(|c| c.material.is_solid_obstacle());

            if p.ttl <= 0.0 || hit_obstacle {
                let expired = self.projectiles.swap_remove(i);
                match expired.kind {
                    ProjectileKind::Bomb => {
                        impacts.push(expired.pos);
                        cmds.push(WorldCmd::Explode {
                            x: tx,
                            y: ty,
                            radius: expired.radius,
                            power: expired.power,
                        });
                    }
                }
            } else {
                i += 1;
            }
        }
        for impact in impacts {
            self.stun_wolves_near(impact, 4.0 * TILE_PX, 2.5);
        }
    }
}
