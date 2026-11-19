use macroquad::prelude::*;

use crate::world::{Material, World, TILE_PX, WET_SOIL_THRESHOLD};

use super::{
    clamp_world_pos, tile_center, Agents, Critter, CritterKind, GravityField, LightningArc,
    Payload, Rune, RuneMods, SpellBolt, SpellImpact, WorldCmd,
};

impl Agents {
    pub(super) fn step_spells(&mut self, world: &World, dt: f32, cmds: &mut Vec<WorldCmd>) {
        let mut impacts: Vec<SpellBolt> = Vec::new();
        let mut i = 0;

        while i < self.spells.len() {
            let mut expired = false;
            let mut zap_pos: Option<(Vec2, usize)> = None;
            {
                let s = &mut self.spells[i];
                if s.mods.homing > 0 {
                    let seek_radius = 120.0 + (s.mods.homing - 1) as f32 * 45.0;
                    if let Some(target) =
                        find_homing_target(world, &self.critters, s.payload, s.pos, seek_radius)
                    {
                        let speed = s.vel.length().max(40.0);
                        let desired = (target - s.pos).normalize_or_zero() * speed;
                        let turn = (5.5 + s.mods.homing as f32 * 2.5) * dt;
                        s.vel = (s.vel.lerp(desired, turn.clamp(0.0, 0.85))).normalize_or_zero()
                            * speed;
                    }
                }

                s.pos += s.vel * dt;
                s.ttl -= dt;
                s.pulse_timer -= dt;

                if s.payload == Payload::HydroCanalJet && s.pulse_timer <= 0.0 {
                    s.pulse_timer = 0.045;
                    let tx = (s.pos.x / TILE_PX).floor() as i32;
                    let ty = (s.pos.y / TILE_PX).floor() as i32;
                    cmds.push(WorldCmd::HydroCarve {
                        x: tx,
                        y: ty,
                        amount: 22,
                        carve_trench: true,
                    });
                    if s.mods.electric > 0 {
                        zap_pos = Some((s.pos, s.mods.electric));
                    }
                }

                let tx = (s.pos.x / TILE_PX).floor() as i32;
                let ty = (s.pos.y / TILE_PX).floor() as i32;
                let hit_solid = !World::in_bounds(tx, ty)
                    || world.get(tx, ty).is_some_and(|c| {
                        c.material.is_solid_obstacle() && s.payload != Payload::HydroCanalJet
                    });
                let hit_wolf = self
                    .critters
                    .iter()
                    .any(|c| c.kind == CritterKind::Wolf && c.pos.distance_squared(s.pos) <= 144.0);

                if s.ttl <= 0.0 || hit_solid || hit_wolf {
                    expired = true;
                }
            }

            if let Some((zp, elec)) = zap_pos {
                self.stun_wolves_near(zp, 2.8 * TILE_PX, 2.0 + elec as f32);
            }

            if expired {
                let done = self.spells.swap_remove(i);
                impacts.push(done);
            } else {
                i += 1;
            }
        }

        for bolt in impacts {
            self.apply_spell_impact(
                world,
                bolt.payload,
                SpellImpact {
                    pos: bolt.pos,
                    vel: bolt.vel,
                    mods: bolt.mods,
                },
                cmds,
            );
        }
    }

    pub(super) fn step_orbit_shards(&mut self, world: &World, dt: f32, cmds: &mut Vec<WorldCmd>) {
        let mut finished = Vec::new();
        let mut pulses: Vec<(Payload, Vec2, RuneMods)> = Vec::new();
        let mut i = 0;

        while i < self.orbit_shards.len() {
            let (drop_shard, hit_wolf_now) = {
                let orb = &mut self.orbit_shards[i];
                orb.angle = (orb.angle + 5.2 * dt) % std::f32::consts::TAU;
                orb.ttl -= dt;
                orb.pulse_timer -= dt;
                let pos = clamp_world_pos(
                    self.mage.pos + vec2(orb.angle.cos(), orb.angle.sin()) * orb.radius,
                );
                if orb.pulse_timer <= 0.0 {
                    orb.pulse_timer = 0.28;
                    pulses.push((orb.payload, pos, orb.mods));
                }
                let hit_wolf = self
                    .critters
                    .iter()
                    .any(|c| c.kind == CritterKind::Wolf && c.pos.distance_squared(pos) <= 196.0);
                (orb.ttl <= 0.0 || hit_wolf, hit_wolf)
            };

            if drop_shard {
                let done = self.orbit_shards.swap_remove(i);
                let pos = clamp_world_pos(
                    self.mage.pos + vec2(done.angle.cos(), done.angle.sin()) * done.radius,
                );
                let tangent = vec2(-done.angle.sin(), done.angle.cos()) * 140.0;
                finished.push((done, pos, tangent, hit_wolf_now));
            } else {
                i += 1;
            }
        }

        for (payload, pos, mods) in pulses {
            let tx = (pos.x / TILE_PX).floor() as i32;
            let ty = (pos.y / TILE_PX).floor() as i32;
            match payload {
                Payload::Fireball => {
                    cmds.push(WorldCmd::Ignite {
                        x: tx,
                        y: ty,
                        temp: 460.0 + mods.electric as f32 * 180.0,
                    });
                }
                Payload::HydroCanalJet => {
                    cmds.push(WorldCmd::HydroCarve {
                        x: tx,
                        y: ty,
                        amount: 16,
                        carve_trench: false,
                    });
                }
                Payload::ChainLightning => {
                    self.fire_chain_lightning_ray(
                        world,
                        pos,
                        (pos - self.mage.pos).normalize_or_zero(),
                        RuneMods {
                            homing: mods.homing,
                            explosive: 0,
                            electric: mods.electric,
                        },
                        cmds,
                    );
                }
                Payload::GravityVortex => {
                    cmds.push(WorldCmd::VortexPull {
                        x_px: pos.x,
                        y_px: pos.y,
                        radius_px: 64.0,
                        strength: 110.0,
                    });
                    self.pull_entities_toward(pos, 64.0, 95.0, dt * 6.0);
                }
                Payload::OvergrowthSeedBomb => {
                    cmds.push(WorldCmd::Overgrow {
                        x: tx,
                        y: ty,
                        radius: 1,
                        boost_stages: 1,
                    });
                }
                Payload::TerraWall => {
                    self.stun_wolves_near(pos, 2.0 * TILE_PX, 1.5);
                }
            }
            if mods.electric > 0 {
                self.stun_wolves_near(pos, 2.8 * TILE_PX, 2.5 + mods.electric as f32);
            }
        }

        for (done, pos, tangent, hit_wolf_now) in finished {
            if !hit_wolf_now && done.mods.homing > 0 && done.payload != Payload::ChainLightning {
                let seek_r = 160.0 + (done.mods.homing - 1) as f32 * 40.0;
                if let Some(target) =
                    find_homing_target(world, &self.critters, done.payload, pos, seek_r)
                {
                    let dir = (target - pos).normalize_or_zero();
                    let speed = 210.0;
                    self.spells.push(SpellBolt {
                        payload: done.payload,
                        pos,
                        vel: if dir == Vec2::ZERO {
                            tangent
                        } else {
                            dir * speed
                        },
                        ttl: (target.distance(pos) / speed).clamp(0.15, 0.95),
                        mods: done.mods,
                        pulse_timer: 0.04,
                    });
                    continue;
                }
            }
            self.apply_spell_impact(
                world,
                done.payload,
                SpellImpact {
                    pos,
                    vel: tangent,
                    mods: done.mods,
                },
                cmds,
            );
        }
    }

    pub(super) fn step_vortices(&mut self, dt: f32, cmds: &mut Vec<WorldCmd>) {
        let mut collapsed = Vec::new();
        let mut i = 0;

        while i < self.vortices.len() {
            let v = &mut self.vortices[i];
            v.ttl -= dt;
            v.zap_timer -= dt;
            let pos = v.pos;
            let radius = v.radius;
            let electric = v.electric;
            let should_zap = electric > 0 && v.zap_timer <= 0.0;
            if should_zap {
                v.zap_timer = 0.35;
            }
            let done = v.ttl <= 0.0;

            cmds.push(WorldCmd::VortexPull {
                x_px: pos.x,
                y_px: pos.y,
                radius_px: radius,
                strength: 145.0,
            });
            self.pull_entities_toward(pos, radius, 88.0, dt);

            if should_zap {
                self.stun_wolves_near(pos, radius, 2.5 + electric as f32);
                self.lightning_arcs.push(LightningArc {
                    points: vec![pos, pos + vec2(radius * 0.45, -radius * 0.35)],
                    ttl: 0.14,
                    max_ttl: 0.14,
                    color: Rune::ElectricCharge.color(),
                });
            }

            if done {
                collapsed.push(self.vortices.swap_remove(i));
            } else {
                i += 1;
            }
        }

        for v in collapsed {
            if v.explosive > 0 {
                let tx = (v.pos.x / TILE_PX).floor() as i32;
                let ty = (v.pos.y / TILE_PX).floor() as i32;
                cmds.push(WorldCmd::Explode {
                    x: tx,
                    y: ty,
                    radius: 2 + v.explosive.min(2) as i32,
                    power: 56.0 + v.explosive as f32 * 14.0,
                });
            }
        }
    }

    fn pull_entities_toward(&mut self, center: Vec2, radius: f32, strength: f32, dt: f32) {
        let r2 = radius * radius;
        for d in &mut self.drops {
            let delta = center - d.pos;
            let d2 = delta.length_squared();
            if d2 > 4.0 && d2 <= r2 {
                let dir = delta / d2.sqrt();
                d.pos = clamp_world_pos(d.pos + dir * strength * dt);
            }
        }
        for c in &mut self.critters {
            let delta = center - c.pos;
            let d2 = delta.length_squared();
            if d2 > 9.0 && d2 <= r2 {
                let dir = delta / d2.sqrt();
                c.pos = clamp_world_pos(c.pos + dir * (strength * 0.75 * dt));
            }
        }
    }

    pub(super) fn step_lightning_arcs(&mut self, dt: f32) {
        for arc in &mut self.lightning_arcs {
            arc.ttl -= dt;
        }
        self.lightning_arcs.retain(|arc| arc.ttl > 0.0);
    }

    pub(super) fn fire_chain_lightning_ray(
        &mut self,
        world: &World,
        origin: Vec2,
        dir: Vec2,
        mods: RuneMods,
        cmds: &mut Vec<WorldCmd>,
    ) {
        let dir = if dir == Vec2::ZERO {
            vec2(1.0, 0.0)
        } else {
            dir.normalize()
        };
        let max_dist = 180.0 + mods.homing as f32 * 45.0;
        let mut strike = origin + dir * 24.0;
        let step_px = 10.0;
        let mut travelled = 12.0;

        while travelled <= max_dist {
            let mut probe = origin + dir * travelled;
            if mods.homing > 0 {
                if let Some(target) = find_homing_target(
                    world,
                    &self.critters,
                    Payload::ChainLightning,
                    probe,
                    75.0 + mods.homing as f32 * 30.0,
                ) {
                    let pull = (target - probe).normalize_or_zero() * 5.0;
                    probe += pull;
                }
            }
            strike = clamp_world_pos(probe);
            let tx = (strike.x / TILE_PX).floor() as i32;
            let ty = (strike.y / TILE_PX).floor() as i32;
            let hit_wolf = self
                .critters
                .iter()
                .any(|c| c.kind == CritterKind::Wolf && c.pos.distance_squared(strike) <= 225.0);
            let hit_tile = world.get(tx, ty).is_some_and(|c| {
                c.material.is_solid_obstacle()
                    || matches!(
                        c.material,
                        Material::Gunpowder | Material::Oil | Material::ShallowWater
                    )
            });
            if hit_wolf || hit_tile {
                break;
            }
            travelled += step_px;
        }

        let mut points = vec![origin, strike];
        let mut cur = strike;
        let hops = 3 + mods.electric * 2;
        let hop_range = if mods.homing > 0 {
            7.5 * TILE_PX
        } else {
            4.5 * TILE_PX
        };
        let stun_secs = 3.0 + mods.electric as f32 * 1.5;

        for _ in 0..hops {
            let stx = (cur.x / TILE_PX).floor() as i32;
            let sty = (cur.y / TILE_PX).floor() as i32;
            cmds.push(WorldCmd::HeatTile {
                x: stx,
                y: sty,
                temp: 320.0 + mods.electric as f32 * 150.0,
                damage: 12,
            });
            self.stun_wolves_near(cur, (3.5 + mods.electric as f32) * TILE_PX, stun_secs);

            let next = find_chain_hop(world, &self.critters, cur, hop_range, &points);
            let Some(next_pos) = next else {
                break;
            };
            cur = next_pos;
            points.push(cur);
        }

        if mods.explosive > 0 {
            let tx = (cur.x / TILE_PX).floor() as i32;
            let ty = (cur.y / TILE_PX).floor() as i32;
            cmds.push(WorldCmd::Explode {
                x: tx,
                y: ty,
                radius: 1 + mods.explosive.min(2) as i32,
                power: 45.0 + mods.explosive as f32 * 12.0,
            });
        }

        self.lightning_arcs.push(LightningArc {
            points,
            ttl: 0.30,
            max_ttl: 0.30,
            color: Payload::ChainLightning.color(),
        });
    }

    pub(super) fn apply_spell_impact(
        &mut self,
        world: &World,
        payload: Payload,
        impact: SpellImpact,
        cmds: &mut Vec<WorldCmd>,
    ) {
        let tx = (impact.pos.x / TILE_PX).floor() as i32;
        let ty = (impact.pos.y / TILE_PX).floor() as i32;
        let mods = impact.mods;

        match payload {
            Payload::Fireball => {
                for dy in -1_i32..=1 {
                    for dx in -1_i32..=1 {
                        cmds.push(WorldCmd::Ignite {
                            x: tx + dx,
                            y: ty + dy,
                            temp: 680.0 + mods.electric as f32 * 250.0,
                        });
                    }
                }
            }
            Payload::HydroCanalJet => {
                for dy in -1_i32..=1 {
                    for dx in -1_i32..=1 {
                        cmds.push(WorldCmd::HydroCarve {
                            x: tx + dx,
                            y: ty + dy,
                            amount: 42,
                            carve_trench: dx == 0 && dy == 0,
                        });
                    }
                }
            }
            Payload::ChainLightning => {
                self.fire_chain_lightning_ray(world, impact.pos, impact.vel, mods, cmds);
            }
            Payload::GravityVortex => {
                self.vortices.push(GravityField {
                    pos: impact.pos,
                    radius: 78.0 + mods.homing as f32 * 12.0,
                    ttl: 2.3,
                    explosive: mods.explosive,
                    electric: mods.electric,
                    zap_timer: 0.15,
                });
                return;
            }
            Payload::OvergrowthSeedBomb => {
                if mods.explosive > 0 {
                    cmds.push(WorldCmd::Explode {
                        x: tx,
                        y: ty,
                        radius: 2,
                        power: 40.0 + mods.explosive as f32 * 10.0,
                    });
                }
                cmds.push(WorldCmd::Overgrow {
                    x: tx,
                    y: ty,
                    radius: 2 + mods.homing.min(1) as i32,
                    boost_stages: 1 + mods.electric as u8,
                });
                if mods.electric > 0 {
                    self.stun_wolves_near(impact.pos, 4.0 * TILE_PX, 3.0 + mods.electric as f32);
                }
                return;
            }
            Payload::TerraWall => {
                if mods.explosive > 0 {
                    cmds.push(WorldCmd::Explode {
                        x: tx,
                        y: ty,
                        radius: 2,
                        power: 46.0 + mods.explosive as f32 * 12.0,
                    });
                }
                let dir = if impact.vel.length_squared() > 0.1 {
                    impact.vel.normalize()
                } else {
                    vec2(1.0, 0.0)
                };
                let perp = vec2(-dir.y, dir.x);
                let step_x = perp.x.round() as i32;
                let step_y = perp.y.round() as i32;
                let (sx, sy) = if step_x == 0 && step_y == 0 {
                    (0, 1)
                } else {
                    (step_x, step_y)
                };
                let mage_tx = (self.mage.pos.x / TILE_PX).floor() as i32;
                let mage_ty = (self.mage.pos.y / TILE_PX).floor() as i32;
                for k in -2_i32..=2 {
                    let wx = tx + sx * k;
                    let wy = ty + sy * k;
                    if self.mage_active && wx == mage_tx && wy == mage_ty {
                        continue;
                    }
                    let blocked_landmark = world.get(wx, wy).is_some_and(|c| {
                        matches!(
                            c.material,
                            Material::Cabin | Material::Chest | Material::Campfire
                        )
                    });
                    if !blocked_landmark {
                        cmds.push(WorldCmd::SetMaterial {
                            x: wx,
                            y: wy,
                            material: Material::Stone,
                        });
                        if mods.electric > 0 {
                            cmds.push(WorldCmd::HeatTile {
                                x: wx,
                                y: wy,
                                temp: 120.0,
                                damage: 0,
                            });
                        }
                    }
                }
                if mods.electric > 0 {
                    self.stun_wolves_near(impact.pos, 4.0 * TILE_PX, 3.5 + mods.electric as f32);
                    self.lightning_arcs.push(LightningArc {
                        points: vec![impact.pos, impact.pos + perp * 28.0],
                        ttl: 0.24,
                        max_ttl: 0.24,
                        color: Rune::ElectricCharge.color(),
                    });
                }
                return;
            }
        }

        if mods.explosive > 0 {
            cmds.push(WorldCmd::Explode {
                x: tx,
                y: ty,
                radius: 2 + mods.explosive.min(2) as i32,
                power: 52.0 + mods.explosive as f32 * 14.0,
            });
        }
        if mods.electric > 0 {
            self.stun_wolves_near(impact.pos, 4.2 * TILE_PX, 3.0 + mods.electric as f32);
            self.lightning_arcs.push(LightningArc {
                points: vec![
                    impact.pos,
                    impact.pos + vec2(22.0, -14.0),
                    impact.pos + vec2(-18.0, 18.0),
                ],
                ttl: 0.22,
                max_ttl: 0.22,
                color: Rune::ElectricCharge.color(),
            });
        }
    }
}

fn find_homing_target(
    world: &World,
    critters: &[Critter],
    payload: Payload,
    pos: Vec2,
    max_dist: f32,
) -> Option<Vec2> {
    let max_d2 = max_dist * max_dist;
    if matches!(
        payload,
        Payload::Fireball | Payload::ChainLightning | Payload::GravityVortex | Payload::TerraWall
    ) {
        if let Some(wolf) = critters
            .iter()
            .filter(|c| c.kind == CritterKind::Wolf)
            .filter_map(|c| {
                let d2 = c.pos.distance_squared(pos);
                (d2 <= max_d2).then_some((d2, c.pos))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
        {
            return Some(wolf.1);
        }
    }

    let r = (max_dist / TILE_PX).ceil() as i32;
    let cx = (pos.x / TILE_PX).floor() as i32;
    let cy = (pos.y / TILE_PX).floor() as i32;
    let mut best: Option<(f32, Vec2)> = None;

    for dy in -r..=r {
        for dx in -r..=r {
            let x = cx + dx;
            let y = cy + dy;
            let Some(cell) = world.get(x, y) else {
                continue;
            };
            let matches_target = match payload {
                Payload::Fireball => matches!(
                    cell.material,
                    Material::Gunpowder | Material::Oil | Material::Tree
                ),
                Payload::HydroCanalJet => {
                    matches!(cell.material, Material::Fire | Material::Lava)
                        || (matches!(cell.material, Material::TilledSoil | Material::Crop(_))
                            && cell.moisture < WET_SOIL_THRESHOLD)
                }
                Payload::ChainLightning => matches!(
                    cell.material,
                    Material::Gunpowder | Material::Oil | Material::ShallowWater
                ),
                Payload::GravityVortex | Payload::TerraWall => {
                    matches!(cell.material, Material::Fire | Material::Lava)
                }
                Payload::OvergrowthSeedBomb => matches!(
                    cell.material,
                    Material::TilledSoil | Material::Ash | Material::Dirt
                ),
            };
            if matches_target {
                let tp = tile_center(x, y);
                let d2 = tp.distance_squared(pos);
                if d2 <= max_d2 && best.is_none_or(|(bd, _)| d2 < bd) {
                    best = Some((d2, tp));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

fn find_chain_hop(
    world: &World,
    critters: &[Critter],
    from: Vec2,
    max_dist: f32,
    visited: &[Vec2],
) -> Option<Vec2> {
    let max_d2 = max_dist * max_dist;
    let is_fresh = |p: Vec2| visited.iter().all(|v| v.distance_squared(p) >= 144.0);

    if let Some((_, wp)) = critters
        .iter()
        .filter(|c| c.kind == CritterKind::Wolf && is_fresh(c.pos))
        .filter_map(|c| {
            let d2 = c.pos.distance_squared(from);
            (d2 <= max_d2).then_some((d2, c.pos))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
    {
        return Some(wp);
    }

    let r = (max_dist / TILE_PX).ceil() as i32;
    let cx = (from.x / TILE_PX).floor() as i32;
    let cy = (from.y / TILE_PX).floor() as i32;
    let mut best: Option<(f32, Vec2)> = None;

    for dy in -r..=r {
        for dx in -r..=r {
            let x = cx + dx;
            let y = cy + dy;
            let Some(cell) = world.get(x, y) else {
                continue;
            };
            let conductive = cell.material.is_water_passable()
                || matches!(
                    cell.material,
                    Material::Gunpowder | Material::Oil | Material::Tree | Material::Fire
                );
            if conductive {
                let tp = tile_center(x, y);
                let d2 = tp.distance_squared(from);
                if d2 <= max_d2 && is_fresh(tp) && best.is_none_or(|(bd, _)| d2 < bd) {
                    best = Some((d2, tp));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}
