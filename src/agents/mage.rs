use macroquad::prelude::*;

use crate::world::{Material, World, TILE_PX};

use super::pathfinding::find_walkable_spawn;
use super::{
    clamp_world_pos, tile_center, velocity_to_dir, Agents, GlowLight, MageGhost, MageInput,
    OrbitShard, Payload, Rune, RuneMods, SpellBolt, WorldCmd, MAGE_DASH_COOLDOWN,
    MAGE_DASH_DURATION, MAGE_DASH_SPEED, MAGE_WALK_SPEED,
};

impl Agents {
    pub fn collect_glow_lights(&self) -> Vec<GlowLight> {
        let mut lights = Vec::new();
        if let Some(mp) = self.mage_pos {
            let dash_boost = if self.mage.dash_timer > 0.0 {
                20.0
            } else {
                0.0
            };
            lights.push(GlowLight {
                pos: mp,
                radius: 78.0 + dash_boost,
                color: self.mage.wand.payload.color(),
                bloom_alpha: 0.24,
            });
        }
        for s in &self.spells {
            lights.push(GlowLight {
                pos: s.pos,
                radius: 44.0,
                color: s.payload.color(),
                bloom_alpha: 0.42,
            });
        }
        for orb in &self.orbit_shards {
            let opos = self.mage.pos + vec2(orb.angle.cos(), orb.angle.sin()) * orb.radius;
            lights.push(GlowLight {
                pos: opos,
                radius: 36.0,
                color: orb.payload.color(),
                bloom_alpha: 0.40,
            });
        }
        for v in &self.vortices {
            lights.push(GlowLight {
                pos: v.pos,
                radius: v.radius * 0.9,
                color: Payload::GravityVortex.color(),
                bloom_alpha: 0.36,
            });
        }
        for arc in &self.lightning_arcs {
            let ratio = (arc.ttl / arc.max_ttl.max(0.05)).clamp(0.0, 1.0);
            for pt in &arc.points {
                lights.push(GlowLight {
                    pos: *pt,
                    radius: 46.0 * ratio,
                    color: arc.color,
                    bloom_alpha: 0.55 * ratio,
                });
            }
        }
        for p in &self.projectiles {
            lights.push(GlowLight {
                pos: p.pos,
                radius: 26.0,
                color: Color::from_rgba(255, 174, 72, 255),
                bloom_alpha: 0.35,
            });
        }
        lights
    }

    pub fn drive_mage(&mut self, world: &World, input: MageInput, dt: f32) -> Vec<WorldCmd> {
        let mut cmds = Vec::new();
        if !self.mage_active {
            return cmds;
        }

        let aim_delta = input.aim_world - self.mage.pos;
        if aim_delta.length_squared() > 1.0 {
            self.mage.aim_dir = aim_delta.normalize();
        }

        self.mage.dash_cooldown = (self.mage.dash_cooldown - dt).max(0.0);
        self.mage.action_tween = (self.mage.action_tween - dt).max(0.0);
        self.mage.hit_flash = (self.mage.hit_flash - dt).max(0.0);

        if input.dash_pressed && self.mage.dash_cooldown <= 0.0 {
            let dir = if input.move_axis.length_squared() > 0.01 {
                input.move_axis.normalize()
            } else if self.mage.aim_dir.length_squared() > 0.01 {
                self.mage.aim_dir
            } else {
                vec2(0.0, 1.0)
            };
            self.mage.dash_dir = dir;
            self.mage.dash_timer = MAGE_DASH_DURATION;
            self.mage.dash_cooldown = MAGE_DASH_COOLDOWN;
            self.mage.ghost_spawn_accum = 0.0;
            self.mage.action_tween = 0.25;
            self.mage_ghosts.push(MageGhost {
                pos: self.mage.pos,
                dir: self.mage.dir,
                alpha: 0.78,
                tint: self.mage.wand.payload.color(),
            });
        }

        let desired_vel = if self.mage.dash_timer > 0.0 {
            self.mage.dash_timer = (self.mage.dash_timer - dt).max(0.0);
            self.mage.ghost_spawn_accum += dt;
            if self.mage.ghost_spawn_accum >= 0.025 {
                self.mage.ghost_spawn_accum = 0.0;
                self.mage_ghosts.push(MageGhost {
                    pos: self.mage.pos,
                    dir: self.mage.dir,
                    alpha: 0.78,
                    tint: self.mage.wand.payload.color(),
                });
            }
            self.mage.dash_dir * MAGE_DASH_SPEED
        } else if input.move_axis.length_squared() > 0.01 {
            input.move_axis.normalize() * MAGE_WALK_SPEED
        } else {
            Vec2::ZERO
        };

        self.mage.vel = desired_vel;
        if desired_vel.length_squared() > 1.0 {
            let total_disp = desired_vel * dt;
            let dist = total_disp.length();
            let substeps = ((dist / 4.0).ceil() as usize).clamp(1, 16);
            let step = total_disp / substeps as f32;
            for _ in 0..substeps {
                let try_pos = clamp_world_pos(self.mage.pos + step);
                if is_walkable_for_mage(world, try_pos) {
                    self.mage.pos = try_pos;
                } else {
                    let try_x = clamp_world_pos(vec2(self.mage.pos.x + step.x, self.mage.pos.y));
                    let try_y = clamp_world_pos(vec2(self.mage.pos.x, self.mage.pos.y + step.y));
                    if is_walkable_for_mage(world, try_x) {
                        self.mage.pos = try_x;
                    } else if is_walkable_for_mage(world, try_y) {
                        self.mage.pos = try_y;
                    } else {
                        break;
                    }
                }
            }
            self.mage.dir = velocity_to_dir(desired_vel);
            self.mage.anim_time += dt * 9.0;
        } else {
            self.mage.dir = velocity_to_dir(self.mage.aim_dir);
            self.mage.anim_time = 0.0;
        }

        // Mild hazard damage with sanctuary blink recovery if knocked out
        let mtx = (self.mage.pos.x / TILE_PX).floor() as i32;
        let mty = (self.mage.pos.y / TILE_PX).floor() as i32;
        let on_hazard = world
            .get(mtx, mty)
            .is_some_and(|c| matches!(c.material, Material::Lava | Material::Acid));
        if on_hazard && self.mage.dash_timer <= 0.0 {
            self.mage.hp -= dt * 18.0;
            self.mage.hit_flash = 0.18;
            if self.mage.hp <= 0.0 {
                let (cx, cy) = world.village_center();
                let safe = find_walkable_spawn(world, cx, cy + 2, 8)
                    .unwrap_or_else(|| tile_center(cx, cy));
                self.mage_ghosts.push(MageGhost {
                    pos: self.mage.pos,
                    dir: self.mage.dir,
                    alpha: 0.9,
                    tint: Color::from_rgba(255, 160, 120, 255),
                });
                self.mage.pos = safe;
                self.mage.hp = 100.0;
            }
        } else {
            self.mage.hp = (self.mage.hp + dt * 4.5).min(100.0);
        }

        self.mage_pos = Some(self.mage.pos);
        self.mage_dir = self.mage.dir;

        if input.cast_down {
            let mut cast_cmds = self.cast_wand(world, input.aim_world);
            cmds.append(&mut cast_cmds);
        }

        cmds
    }

    pub fn cast_wand(&mut self, world: &World, aim_world: Vec2) -> Vec<WorldCmd> {
        let mut cmds = Vec::new();
        let cost = self.mage.wand.mana_cost();
        if self.mage.wand.cooldown > 0.0 || self.mage.wand.mana < cost {
            return cmds;
        }

        self.mage.wand.mana -= cost;
        self.mage.wand.cooldown = self.mage.wand.cast_cooldown();
        self.mage.action_tween = 0.25;

        let origin = self.mage.pos;
        let base_dir = {
            let d = aim_world - origin;
            if d.length_squared() > 1.0 {
                d.normalize()
            } else {
                self.mage.aim_dir
            }
        };
        let payload = self.mage.wand.payload;
        let multi = self.mage.wand.rune_count(Rune::MultiCast);
        let orbit = self.mage.wand.rune_count(Rune::Orbit);
        let mods = RuneMods {
            homing: self.mage.wand.rune_count(Rune::Homing),
            explosive: self.mage.wand.rune_count(Rune::ExplosiveImpact),
            electric: self.mage.wand.rune_count(Rune::ElectricCharge),
        };

        let count = 1 + 2 * multi;
        let step_angle = 0.24_f32;
        let base_angle = base_dir.y.atan2(base_dir.x);
        let half = (count as f32 - 1.0) * 0.5;

        if orbit > 0 {
            let orbit_ttl = 1.45 + orbit as f32 * 0.45;
            for i in 0..count {
                let a = (i as f32 / count as f32) * std::f32::consts::TAU;
                self.orbit_shards.push(OrbitShard {
                    payload,
                    angle: a,
                    radius: 26.0 + (orbit.saturating_sub(1)) as f32 * 5.0,
                    ttl: orbit_ttl,
                    mods,
                    pulse_timer: 0.12,
                });
            }
            return cmds;
        }

        if payload == Payload::ChainLightning {
            for i in 0..count {
                let offset = (i as f32 - half) * step_angle;
                let angle = base_angle + offset;
                let dir = vec2(angle.cos(), angle.sin());
                self.fire_chain_lightning_ray(world, origin, dir, mods, &mut cmds);
            }
            return cmds;
        }

        let speed = match payload {
            Payload::Fireball => 220.0,
            Payload::HydroCanalJet => 245.0,
            Payload::ChainLightning => 320.0,
            Payload::GravityVortex => 155.0,
            Payload::OvergrowthSeedBomb => 185.0,
            Payload::TerraWall => 235.0,
        };
        let travel = (aim_world.distance(origin) - 10.0).clamp(10.0, 220.0);
        let base_ttl = match payload {
            Payload::GravityVortex | Payload::OvergrowthSeedBomb | Payload::TerraWall => {
                (travel / speed).clamp(0.05, 1.35)
            }
            _ => 1.15,
        };

        for i in 0..count {
            let offset = (i as f32 - half) * step_angle;
            let angle = base_angle + offset;
            let dir = vec2(angle.cos(), angle.sin());
            self.spells.push(SpellBolt {
                payload,
                pos: clamp_world_pos(origin + dir * 10.0),
                vel: dir * speed,
                ttl: base_ttl,
                mods,
                pulse_timer: 0.04,
            });
        }

        cmds
    }

    pub(super) fn step_mage_resources_and_ghosts(&mut self, dt: f32) {
        self.mage.wand.cooldown = (self.mage.wand.cooldown - dt).max(0.0);
        self.mage.wand.mana =
            (self.mage.wand.mana + self.mage.wand.mana_regen * dt).min(self.mage.wand.max_mana);
        for g in &mut self.mage_ghosts {
            g.alpha -= dt * 2.6;
        }
        self.mage_ghosts.retain(|g| g.alpha > 0.04);
    }
}

fn is_walkable_for_mage(world: &World, pos: Vec2) -> bool {
    const HALF: f32 = 4.0;
    for (ox, oy) in [(-HALF, -HALF), (HALF, -HALF), (-HALF, HALF), (HALF, HALF)] {
        let tx = ((pos.x + ox) / TILE_PX).floor() as i32;
        let ty = ((pos.y + oy) / TILE_PX).floor() as i32;
        let ok = world
            .get(tx, ty)
            .is_some_and(|c| !c.material.is_solid_obstacle() && c.material != Material::DeepWater);
        if !ok {
            return false;
        }
    }
    true
}
