use macroquad::prelude::Vec2;
use rand::Rng;

use super::*;

impl World {
    fn quench_hot_rock(&mut self, x: i32, y: i32, amount: u8, is_lava: bool) {
        let boil_cap = if is_lava {
            LAVA_BOIL_WATER_VOL
        } else {
            FIRE_BOIL_WATER_VOL
        };
        let solidify_temp = if is_lava {
            LAVA_SOLIDIFY_TEMP
        } else {
            MAGMA_SOLIDIFY_TEMP
        };
        let steam_temp = if is_lava { 120.0 } else { 115.0 };
        let steam_vol = amount.min(boil_cap);
        let cooled = self
            .get_mut(x, y)
            .map(|c| {
                c.temperature -= LAVA_WATER_COOL;
                c.temperature < solidify_temp
            })
            .unwrap_or(false);
        if cooled {
            self.set_material(x, y, Material::Stone);
        }
        if steam_vol > 0 {
            self.spawn_gas(x, y, GasKind::Steam, steam_temp, steam_vol);
        }
        if amount > steam_vol {
            self.deposit_water_conserving(x, y, amount - steam_vol);
        }
        self.wake_around(x, y);
    }

    pub fn add_water(&mut self, x: i32, y: i32, amount: u8) {
        if amount == 0 {
            return;
        }
        let Some(prev_mat) = self.get(x, y).map(|c| c.material) else {
            return;
        };
        match prev_mat {
            Material::Fire => {
                self.splash_extinguish(x, y, amount);
            }
            Material::Lava => {
                self.quench_hot_rock(x, y, amount, true);
            }
            Material::Magma => {
                self.quench_hot_rock(x, y, amount, false);
            }
            Material::Cabin | Material::Chest | Material::Campfire => {
                self.deposit_water_conserving(x, y, amount);
            }
            m if m.is_water_passable() => {
                let leftover = if let Some(cell) = self.get_mut(x, y) {
                    let add = amount.min(100 - cell.water_volume);
                    cell.water_volume += add;
                    cell.moisture = 1.0;
                    amount - add
                } else {
                    0
                };
                if leftover > 0 {
                    self.deposit_water_conserving(x, y, leftover);
                }
                self.wake_around(x, y);
            }
            _ => {
                let variant = self.rng.gen::<u8>() & 0b1110;
                let mut cell = Cell::new(Material::ShallowWater, variant);
                cell.water_volume = amount.min(100);
                cell.updated_tick = self.tick;
                self.set(x, y, cell);
            }
        }
    }

    pub fn paint_fire(&mut self, x: i32, y: i32, temp: f32) {
        let Some(prev) = self.get(x, y).copied() else {
            return;
        };
        match prev.material {
            Material::Gunpowder => {
                self.detonate_gunpowder(x, y);
            }
            Material::Campfire => {
                if let Some(cell) = self.get_mut(x, y) {
                    cell.set_lit_campfire(true);
                    cell.temperature = cell.temperature.max(temp);
                }
                self.wake_around(x, y);
            }
            m if m.is_water_passable() && prev.water_volume > 0 => {
                let boil = prev.water_volume.min(FIRE_BOIL_WATER_VOL);
                if let Some(cell) = self.get_mut(x, y) {
                    cell.water_volume -= boil;
                    if cell.material == Material::ShallowWater && cell.water_volume == 0 {
                        cell.material = Material::Dirt;
                        cell.structural_hp = Material::Dirt.default_hp();
                    }
                }
                self.spawn_gas(x, y, GasKind::Steam, 115.0, boil);
                self.spawn_gas(x, y, GasKind::Smoke, 75.0, 0);
                self.wake_around(x, y);
            }
            Material::Ice => {
                if let Some(cell) = self.get_mut(x, y) {
                    cell.temperature += 45.0;
                }
                self.wake_around(x, y);
            }
            Material::Stone | Material::Magma | Material::Lava => {
                if let Some(cell) = self.get_mut(x, y) {
                    cell.temperature = cell.temperature.max(temp).min(1200.0);
                }
                self.wake_around(x, y);
            }
            Material::Fire => {
                if let Some(cell) = self.get_mut(x, y) {
                    cell.burn_timer = cell.burn_timer.max(BURN_TIME_DEFAULT);
                    cell.temperature = cell.temperature.max(temp);
                }
                self.wake_around(x, y);
            }
            _ => {
                let dur = prev.material.burn_duration();
                let variant = self.rng.gen::<u8>() & 0b1110;
                let mut fire = Cell::new(Material::Fire, variant);
                fire.burn_timer = dur;
                fire.temperature = temp.max(Material::Fire.default_temp());
                fire.updated_tick = self.tick;
                self.set(x, y, fire);
            }
        }
    }

    pub fn ignite(&mut self, x: i32, y: i32, temp: f32) {
        let Some(prev) = self.get(x, y).copied() else {
            return;
        };
        if prev.material == Material::Gunpowder {
            self.detonate_gunpowder(x, y);
            return;
        }
        let mut steam_vol = 0_u8;
        if let Some(cell) = self.get_mut(x, y) {
            cell.temperature = cell.temperature.max(temp);
            if cell.material == Material::Campfire {
                cell.set_lit_campfire(true);
            } else if cell.material.flammability().is_some()
                || matches!(cell.material, Material::Ash | Material::Dirt) && temp >= 350.0
            {
                let burn_time = cell.material.burn_duration();
                steam_vol = cell.water_volume;
                cell.material = Material::Fire;
                cell.burn_timer = burn_time;
                cell.water_volume = 0;
            }
        }
        if steam_vol > 0 {
            self.spawn_gas(x, y, GasKind::Steam, 115.0, steam_vol);
        }
        self.wake_around(x, y);
    }

    pub fn scoop_water(&mut self, x: i32, y: i32, amount: u8) -> u8 {
        if amount == 0 {
            return 0;
        }
        let Some(cell) = self.get_mut(x, y) else {
            return 0;
        };
        if !cell.material.is_water_passable() || cell.water_volume <= 1 {
            return 0;
        }
        let taken = amount.min(cell.water_volume - 1);
        cell.water_volume -= taken;
        self.wake_around(x, y);
        taken
    }

    pub fn splash_extinguish(&mut self, x: i32, y: i32, water: u8) {
        let Some(&prev) = self.get(x, y) else {
            return;
        };
        if prev.material == Material::Fire {
            let remnant_mat = if self.rng.gen_bool(0.65) {
                Material::Ash
            } else {
                Material::Dirt
            };
            let variant = self.rng.gen::<u8>() & 0b1110;
            let mut remnant = Cell::new(remnant_mat, variant);
            remnant.moisture = 0.85;
            remnant.updated_tick = self.tick;
            self.set(x, y, remnant);
            if water > 0 {
                self.spawn_gas(x, y, GasKind::Steam, 115.0, water);
            }
            self.spawn_gas(x, y, GasKind::Smoke, 65.0, 0);
        } else if water > 0 {
            self.deposit_water_conserving(x, y, water);
        }
    }

    pub fn harvest_crop(&mut self, x: i32, y: i32) {
        let Some(&prev) = self.get(x, y) else {
            return;
        };
        if !matches!(prev.material, Material::Crop(_)) {
            return;
        }
        let wet = prev.moisture > WET_SOIL_THRESHOLD;
        let variant = self.rng.gen::<u8>() & 0b1110;
        let mut soil = if wet {
            let mut s = Cell::new(Material::WetSoil, variant);
            s.set_tilled(true);
            s
        } else {
            Cell::new(Material::TilledSoil, variant)
        };
        soil.moisture = prev.moisture;
        soil.updated_tick = self.tick;
        self.set(x, y, soil);
    }

    pub fn harvest_berry(&mut self, x: i32, y: i32) {
        let cur_tick = self.tick;
        let mut wake = false;
        if let Some(cell) = self.get_mut(x, y) {
            if cell.material == Material::BerryBush && cell.has_berries() {
                cell.set_berries(false);
                cell.growth = 0.0;
                cell.updated_tick = cur_tick;
                wake = true;
            }
        }
        if wake {
            self.wake_around(x, y);
        }
    }

    pub fn nibble_crop(&mut self, x: i32, y: i32) {
        let Some(&prev) = self.get(x, y) else {
            return;
        };
        match prev.material {
            Material::Crop(s) if s > 1 => {
                let mut next = Cell::new(Material::Crop(s - 1), prev.variant);
                next.moisture = prev.moisture;
                next.set_wet_crop(prev.is_wet_crop());
                next.updated_tick = self.tick;
                self.set(x, y, next);
            }
            Material::Crop(_) => {
                self.harvest_crop(x, y);
            }
            Material::BerryBush if prev.has_berries() => {
                self.harvest_berry(x, y);
            }
            _ => {}
        }
    }

    pub fn heat_tile(&mut self, x: i32, y: i32, temp: f32, damage: u8) {
        let Some(prev) = self.get(x, y).copied() else {
            return;
        };
        if prev.material == Material::Gunpowder {
            self.detonate_gunpowder(x, y);
            return;
        }
        if temp >= 360.0 && prev.material.flammability().is_some() {
            self.ignite(x, y, temp);
            return;
        }
        if let Some(cell) = self.get_mut(x, y) {
            cell.temperature = cell.temperature.max(temp).min(1200.0);
            if damage > 0 {
                cell.structural_hp = cell.structural_hp.saturating_sub(damage);
            }
        }
        if prev.structural_hp <= damage
            && damage > 0
            && !matches!(
                prev.material,
                Material::DeepWater | Material::ShallowWater | Material::Chest | Material::Cabin
            )
        {
            self.set_material(x, y, Material::Dirt);
        } else {
            self.wake_around(x, y);
        }
    }

    pub fn hydro_carve(&mut self, x: i32, y: i32, amount: u8, carve_trench: bool) {
        let Some(prev) = self.get(x, y).copied() else {
            return;
        };
        match prev.material {
            Material::Fire => {
                self.splash_extinguish(x, y, amount);
            }
            Material::Lava => {
                self.quench_hot_rock(x, y, amount, true);
            }
            Material::Magma => {
                self.quench_hot_rock(x, y, amount, false);
            }
            Material::TilledSoil | Material::WetSoil | Material::Crop(_) => {
                if let Some(cell) = self.get_mut(x, y) {
                    cell.moisture = (cell.moisture + 0.45).min(1.0);
                    if matches!(cell.material, Material::Crop(_)) {
                        cell.set_wet_crop(true);
                    }
                }
                self.absorbed_water += amount as u64;
                self.wake_around(x, y);
            }
            m if m.is_water_passable() => {
                self.add_water(x, y, amount);
            }
            Material::Dirt | Material::Grass | Material::Sand | Material::Ash if carve_trench => {
                self.set_material(x, y, Material::Trench);
                self.add_water(x, y, amount);
            }
            _ => {
                self.deposit_water_conserving(x, y, amount);
            }
        }
    }

    pub fn overgrow(&mut self, cx: i32, cy: i32, radius: i32, boost_stages: u8) {
        let r_sq = radius * radius;
        let stages = boost_stages.max(1);
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy > r_sq {
                    continue;
                }
                let x = cx + dx;
                let y = cy + dy;
                let Some(prev) = self.get(x, y).copied() else {
                    continue;
                };
                match prev.material {
                    Material::Crop(s) => {
                        let next_s = (s + stages).min(4);
                        let mut grown = Cell::new(Material::Crop(next_s), prev.variant);
                        grown.moisture = (prev.moisture + 0.35).min(1.0);
                        grown.set_wet_crop(grown.moisture > WET_SOIL_THRESHOLD);
                        grown.growth = if next_s >= 4 { 1.0 } else { prev.growth };
                        grown.updated_tick = self.tick;
                        self.set(x, y, grown);
                    }
                    Material::TilledSoil => {
                        self.set_material(x, y, Material::Crop(1));
                    }
                    Material::WetSoil if prev.is_tilled() => {
                        self.set_material(x, y, Material::Crop(1));
                    }
                    Material::BerryBush if !prev.has_berries() => {
                        if let Some(c) = self.get_mut(x, y) {
                            c.set_berries(true);
                            c.growth = 0.0;
                        }
                        self.wake_around(x, y);
                    }
                    Material::Dirt | Material::Sand | Material::Ash | Material::Fire => {
                        let d2 = dx * dx + dy * dy;
                        let target = if dx == 0 && dy == 0 && self.rng.gen_bool(0.55) {
                            Material::Tree
                        } else if d2 <= 2 && self.rng.gen_bool(0.22) {
                            Material::BerryBush
                        } else {
                            Material::Grass
                        };
                        self.set_material(x, y, target);
                    }
                    Material::Grass if dx == 0 && dy == 0 && self.rng.gen_bool(0.45) => {
                        self.set_material(x, y, Material::BerryBush);
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn pull_particles_toward(&mut self, center_px: Vec2, radius_px: f32, strength: f32) {
        let r2 = radius_px * radius_px;
        let max_x = GRID_W as f32 * TILE_PX - 1.0;
        let max_y = GRID_H as f32 * TILE_PX - 1.0;
        for d in &mut self.debris {
            let delta = center_px - d.pos;
            let dist2 = delta.length_squared();
            if dist2 > 4.0 && dist2 <= r2 {
                let dir = delta / dist2.sqrt();
                d.vel += dir * strength * SIM_DT;
            }
        }
        for g in &mut self.gas {
            let delta = center_px - g.pos;
            let dist2 = delta.length_squared();
            if dist2 > 4.0 && dist2 <= r2 {
                let dir = delta / dist2.sqrt();
                g.pos += dir * (strength * 0.45 * SIM_DT);
                g.pos.x = g.pos.x.clamp(1.0, max_x);
                g.pos.y = g.pos.y.clamp(1.0, max_y);
            }
        }
    }

    pub fn explode(&mut self, cx: i32, cy: i32, radius: i32, power: f32) {
        if radius <= 0 || power <= 0.0 {
            return;
        }
        self.shake_impulse = (self.shake_impulse + (power / 80.0).clamp(0.2, 1.0)).min(1.5);
        let r_sq = radius * radius;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let d_sq = dx * dx + dy * dy;
                if d_sq > r_sq {
                    continue;
                }
                let x = cx + dx;
                let y = cy + dy;
                if !Self::in_bounds(x, y) {
                    continue;
                }
                let dist = (d_sq as f32).sqrt();
                let falloff = (1.0 - dist / (radius as f32 + 0.5)).max(0.0);
                let damage = power * falloff;
                let idx = Self::idx(x as usize, y as usize);
                let prev = self.cells[idx];
                if prev.material == Material::DeepWater {
                    continue;
                }
                if prev.material == Material::Gunpowder && (dx != 0 || dy != 0) {
                    self.detonate_gunpowder(x, y);
                    continue;
                }
                if damage >= prev.structural_hp as f32 {
                    if prev.water_volume > 0 {
                        self.spawn_gas(x, y, GasKind::Steam, 120.0, prev.water_volume);
                    }
                    if self.debris.len() < 400 && self.rng.gen_bool(0.65) {
                        let debris_mat = match prev.material {
                            Material::Stone | Material::Magma => Material::Stone,
                            Material::WoodWall | Material::Cabin | Material::Tree => Material::Ash,
                            _ => Material::Dirt,
                        };
                        let angle = if dx == 0 && dy == 0 {
                            self.rng.gen_range(0.0..std::f32::consts::TAU)
                        } else {
                            (dy as f32).atan2(dx as f32) + self.rng.gen_range(-0.35..0.35)
                        };
                        let speed = self.rng.gen_range(28.0..95.0) * (0.4 + falloff);
                        self.debris.push(Debris {
                            pos: Vec2::new((x as f32 + 0.5) * TILE_PX, (y as f32 + 0.5) * TILE_PX),
                            vel: Vec2::new(angle.cos() * speed, angle.sin() * speed),
                            height: 2.0,
                            v_height: self.rng.gen_range(60.0..135.0),
                            material: debris_mat,
                            color: debris_mat.color(),
                        });
                    }
                    let crater_mat = if dist < radius as f32 * 0.55 {
                        Material::Ash
                    } else {
                        Material::Dirt
                    };
                    let variant = self.rng.gen::<u8>() & 0b1110;
                    let mut crater = Cell::new(crater_mat, variant);
                    crater.temperature = AMBIENT_TEMP + 140.0 * falloff;
                    crater.updated_tick = self.tick;
                    self.cells[idx] = crater;
                } else {
                    let hp_loss = (damage as u8).max(1);
                    self.cells[idx].structural_hp = prev.structural_hp.saturating_sub(hp_loss);
                    self.cells[idx].temperature = (prev.temperature + 60.0 * falloff).min(800.0);
                }
                self.wake_around(x, y);
            }
        }
        self.last_terrain_change_tick = self.tick;
    }

    pub(super) fn detonate_gunpowder(&mut self, x: i32, y: i32) {
        if !Self::in_bounds(x, y) {
            return;
        }
        let idx = Self::idx(x as usize, y as usize);
        let prev_water = self.cells[idx].water_volume;
        let variant = self.rng.gen::<u8>() & 0b1110;
        let mut ash = Cell::new(Material::Ash, variant);
        ash.temperature = 320.0;
        ash.updated_tick = self.tick;
        self.cells[idx] = ash;
        if prev_water > 0 {
            self.spawn_gas(x, y, GasKind::Steam, 120.0, prev_water);
        }
        self.pending_explosions.push_back(PendingExplosion {
            x,
            y,
            radius: GUNPOWDER_EXPLODE_RADIUS,
            power: GUNPOWDER_EXPLODE_POWER,
        });
        self.wake_around(x, y);
    }

    pub fn take_shake_impulse(&mut self) -> f32 {
        let s = self.shake_impulse;
        self.shake_impulse = 0.0;
        s
    }

    pub fn set_weather(&mut self, weather: Weather) {
        self.weather = weather;
        for cy in 0..CHUNKS_Y {
            for cx in 0..CHUNKS_X {
                if self.chunk_has_weather_subject(cx, cy) {
                    self.wake_chunk(cx, cy);
                }
            }
        }
    }

    pub fn cycle_weather(&mut self) {
        self.set_weather(self.weather.next());
    }

    fn chunk_has_weather_subject(&self, cx: usize, cy: usize) -> bool {
        let x_start = cx * CHUNK_SIZE;
        let x_end = (x_start + CHUNK_SIZE).min(GRID_W);
        let y_start = cy * CHUNK_SIZE;
        let y_end = (y_start + CHUNK_SIZE).min(GRID_H);
        for y in y_start..y_end {
            for x in x_start..x_end {
                let mat = self.cells[Self::idx(x, y)].material;
                if matches!(
                    mat,
                    Material::Fire
                        | Material::Dirt
                        | Material::TilledSoil
                        | Material::WetSoil
                        | Material::Grass
                        | Material::Sand
                        | Material::Crop(_)
                        | Material::Ice
                ) || mat.is_water_passable()
                {
                    return true;
                }
            }
        }
        false
    }
}
