use macroquad::prelude::Vec2;
use rand::Rng;

use super::*;

impl World {
    pub(super) fn step_wind(&mut self) {
        let delta_angle: f32 = self.rng.gen_range(-0.012..0.012);
        let cur_angle = self.wind.y.atan2(self.wind.x) + delta_angle;
        let target_speed = match self.weather {
            Weather::Rainstorm => 22.0,
            Weather::Heatwave => 10.0,
            Weather::Clear => 14.0,
        };
        let cur_speed = self.wind.length().clamp(WIND_MIN_SPEED, WIND_MAX_SPEED);
        let new_speed =
            cur_speed + (target_speed - cur_speed) * 0.01 + self.rng.gen_range(-0.12..0.12);
        let clamped_speed = new_speed.clamp(WIND_MIN_SPEED, WIND_MAX_SPEED);
        self.wind = Vec2::new(
            cur_angle.cos() * clamped_speed,
            cur_angle.sin() * clamped_speed,
        );
    }

    pub(super) fn step_weather_stripe(&mut self, cur_tick: u32) {
        if self.weather == Weather::Clear {
            return;
        }
        let stripe = (cur_tick as usize) % 10;
        let y_start = stripe * 10;
        let y_end = (y_start + 10).min(GRID_H);

        for y in y_start..y_end {
            for x in 0..GRID_W {
                let idx = Self::idx(x, y);
                let mat = self.cells[idx].material;
                let cx = x / CHUNK_SIZE;
                let cy = y / CHUNK_SIZE;

                match self.weather {
                    Weather::Rainstorm => {
                        if mat == Material::Fire {
                            let remnant_mat = if self.rng.gen_bool(0.65) {
                                Material::Ash
                            } else {
                                Material::Dirt
                            };
                            let variant = self.rng.gen::<u8>() & 0b1110;
                            let mut next = Cell::new(remnant_mat, variant);
                            next.moisture = 0.85;
                            next.updated_tick = cur_tick;
                            self.cells[idx] = next;
                            self.spawn_gas(x as i32, y as i32, GasKind::Smoke, 65.0, 0);
                            self.wake_around(x as i32, y as i32);
                        } else if mat.is_water_passable() {
                            if self.cells[idx].water_volume < 100
                                && self.rng.gen_bool(RAIN_WATER_ADD_CHANCE)
                            {
                                self.cells[idx].water_volume += 1;
                                #[cfg(test)]
                                {
                                    self.rain_water_added += 1;
                                }
                                self.wake_around(x as i32, y as i32);
                            }
                        } else if matches!(
                            mat,
                            Material::Dirt
                                | Material::TilledSoil
                                | Material::WetSoil
                                | Material::Grass
                                | Material::Sand
                                | Material::Crop(_)
                        ) {
                            let moist = self.cells[idx].moisture;
                            if moist < 1.0 {
                                self.cells[idx].moisture = (moist + RAIN_SOIL_WET_RATE).min(1.0);
                                self.wake_chunk(cx, cy);
                            } else if matches!(mat, Material::Dirt | Material::TilledSoil)
                                || matches!(mat, Material::Crop(1..=3))
                            {
                                self.wake_chunk(cx, cy);
                            }
                        }
                    }
                    Weather::Heatwave => {
                        let cell = self.cells[idx];
                        if mat == Material::Fire || mat == Material::Ice {
                            self.wake_chunk(cx, cy);
                        } else if matches!(
                            mat,
                            Material::Dirt
                                | Material::TilledSoil
                                | Material::WetSoil
                                | Material::Grass
                                | Material::Sand
                                | Material::Crop(_)
                        ) {
                            let temp_unsettled =
                                (cell.temperature - HEATWAVE_AMBIENT_TEMP).abs() > 0.3;
                            let drying_unsettled =
                                cell.moisture > 0.0 && self.nearest_water_dist2(x, y) > 2;
                            let growing_crop = matches!(mat, Material::Crop(1..=3));
                            let needs_revert = mat == Material::WetSoil
                                && cell.moisture < DRY_SOIL_REVERT_THRESHOLD;
                            if temp_unsettled || drying_unsettled || growing_crop || needs_revert {
                                self.wake_chunk(cx, cy);
                            }
                        } else if mat.is_water_passable()
                            && (cell.temperature - HEATWAVE_AMBIENT_TEMP).abs() > 0.3
                        {
                            self.wake_chunk(cx, cy);
                        }
                    }
                    Weather::Clear => {}
                }
            }
        }
    }

    pub(super) fn step_cell(&mut self, x: usize, y: usize, cur_tick: u32) {
        let idx = Self::idx(x, y);
        if self.cells[idx].updated_tick == cur_tick {
            return;
        }
        self.cells[idx].updated_tick = cur_tick;

        let mut changed = false;
        let mat = self.cells[idx].material;

        // Lift brush-placed Smoke or Steam cells into airborne gas particles while conserving water
        if mat == Material::Steam || mat == Material::Smoke {
            let water = self.cells[idx].water_volume;
            let temp = self.cells[idx].temperature;
            let kind = if mat == Material::Steam {
                GasKind::Steam
            } else {
                GasKind::Smoke
            };
            let variant = self.rng.gen::<u8>() & 0b1110;
            let mut base = Cell::new(Material::Dirt, variant);
            base.updated_tick = cur_tick;
            self.cells[idx] = base;
            self.spawn_gas(x as i32, y as i32, kind, temp, water);
            self.wake_around(x as i32, y as i32);
            return;
        }

        // 1. Water flow across DeepWater, ShallowWater, and Trench cells
        if mat.is_water_passable() && self.step_water_flow(x, y, cur_tick) {
            changed = true;
        }

        // 2. Elemental reactions
        match self.cells[idx].material {
            Material::Fire => {
                if self.step_fire(x, y, cur_tick) {
                    changed = true;
                }
            }
            Material::Lava => {
                if self.step_lava(x, y, cur_tick) {
                    changed = true;
                }
            }
            Material::Magma => {
                if self.cells[idx].temperature < MAGMA_SOLIDIFY_TEMP {
                    self.cells[idx].material = Material::Stone;
                    self.cells[idx].structural_hp = Material::Stone.default_hp();
                    changed = true;
                } else {
                    for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        let Some(n_cell) = self.get(nx, ny).copied() else {
                            continue;
                        };
                        if n_cell.material == Material::Ice {
                            if let Some(nc) = self.get_mut(nx, ny) {
                                nc.temperature += 25.0;
                            }
                            self.wake_around(nx, ny);
                            changed = true;
                        } else if n_cell.material.is_water_passable() && n_cell.water_volume > 0 {
                            let boil = n_cell.water_volume.min(FIRE_BOIL_WATER_VOL);
                            if let Some(nc) = self.get_mut(nx, ny) {
                                nc.water_volume -= boil;
                                if nc.material == Material::ShallowWater && nc.water_volume == 0 {
                                    nc.material = Material::Dirt;
                                    nc.structural_hp = Material::Dirt.default_hp();
                                }
                            }
                            self.spawn_gas(nx, ny, GasKind::Steam, 115.0, boil);
                            self.cells[idx].temperature -= LAVA_WATER_COOL;
                            self.wake_around(nx, ny);
                            changed = true;
                        }
                    }
                    if self.cells[idx].temperature < MAGMA_SOLIDIFY_TEMP {
                        self.cells[idx].material = Material::Stone;
                        self.cells[idx].structural_hp = Material::Stone.default_hp();
                    }
                }
            }
            Material::Ice => {
                if self.weather == Weather::Heatwave {
                    self.cells[idx].temperature += HEATWAVE_ICE_WARM_PER_TICK;
                    changed = true;
                }
                if self.cells[idx].temperature > 0.0 {
                    let variant = self.rng.gen::<u8>() & 0b1110;
                    let mut water = Cell::new(Material::ShallowWater, variant);
                    water.water_volume = ICE_MELT_WATER_VOL + ICE_STEAM_WATER_VOL;
                    water.temperature = 10.0;
                    water.updated_tick = cur_tick;
                    self.cells[idx] = water;
                    changed = true;
                }
            }
            Material::Acid => {
                if self.step_acid(x, y, cur_tick) {
                    changed = true;
                }
            }
            Material::Gunpowder => {
                if self.step_gunpowder(x, y) {
                    changed = true;
                }
            }
            Material::Campfire if self.cells[idx].is_lit_campfire() && self.rng.gen_bool(0.01) => {
                self.spawn_gas(x as i32, y as i32, GasKind::Smoke, 80.0, 0);
            }
            Material::BerryBush if !self.cells[idx].has_berries() => {
                self.cells[idx].growth += CROP_GROWTH_PER_TICK * 1.4;
                if self.cells[idx].growth >= 1.0 {
                    self.cells[idx].growth = 0.0;
                    self.cells[idx].set_berries(true);
                }
                changed = true;
            }
            _ => {}
        }

        // 3. Soil moisture wicking, wet/dry transitions, and crop growth
        let cur_mat = self.cells[idx].material;
        if matches!(
            cur_mat,
            Material::Dirt
                | Material::TilledSoil
                | Material::WetSoil
                | Material::Grass
                | Material::Sand
                | Material::Crop(_)
        ) && self.step_soil_and_crop(x, y)
        {
            changed = true;
        }

        // 4. Temperature relaxation toward material target (skip self-cooling Lava & heatwave Ice)
        let cur_mat = self.cells[idx].material;
        if cur_mat != Material::Lava
            && !(cur_mat == Material::Ice && self.weather == Weather::Heatwave)
        {
            let mut target_temp = cur_mat.default_temp();
            if self.weather == Weather::Heatwave && (target_temp - AMBIENT_TEMP).abs() < 0.1 {
                target_temp = HEATWAVE_AMBIENT_TEMP;
            }
            let diff = target_temp - self.cells[idx].temperature;
            if diff.abs() > 0.25 {
                self.cells[idx].temperature += diff * TEMP_EQUILIBRATE_RATE;
                changed = true;
            } else if diff.abs() > 0.0 {
                self.cells[idx].temperature = target_temp;
            }
        }

        if changed {
            self.wake_around(x as i32, y as i32);
        }
    }

    fn step_fire(&mut self, x: usize, y: usize, cur_tick: u32) -> bool {
        let idx = Self::idx(x, y);
        let ix = x as i32;
        let iy = y as i32;

        // Extinguish and boil water if touching water
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let nx = ix + dx;
            let ny = iy + dy;
            if !Self::in_bounds(nx, ny) {
                continue;
            }
            let n_idx = Self::idx(nx as usize, ny as usize);
            let n_cell = self.cells[n_idx];
            if n_cell.material.is_water_passable() && n_cell.water_volume > 0 {
                let boil = n_cell.water_volume.min(FIRE_BOIL_WATER_VOL);
                self.cells[n_idx].water_volume -= boil;
                if self.cells[n_idx].material == Material::ShallowWater
                    && self.cells[n_idx].water_volume == 0
                {
                    self.cells[n_idx].material = Material::Dirt;
                    self.cells[n_idx].structural_hp = Material::Dirt.default_hp();
                }
                self.spawn_gas(nx, ny, GasKind::Steam, 115.0, boil);
                self.wake_around(nx, ny);

                let variant = self.rng.gen::<u8>() & 0b1110;
                let mut ash = Cell::new(Material::Ash, variant);
                ash.updated_tick = cur_tick;
                self.cells[idx] = ash;
                self.spawn_gas(ix, iy, GasKind::Smoke, 75.0, 0);
                return true;
            }
        }

        self.cells[idx].burn_timer -= SIM_DT;
        if self.rng.gen_bool(SMOKE_EMIT_CHANCE) {
            self.spawn_gas(ix, iy, GasKind::Smoke, 85.0, 0);
        }

        if self.cells[idx].burn_timer <= 0.0 {
            let mat = if self.rng.gen_bool(0.65) {
                Material::Ash
            } else {
                Material::Dirt
            };
            let variant = self.rng.gen::<u8>() & 0b1110;
            let mut remnant = Cell::new(mat, variant);
            remnant.temperature = 65.0;
            remnant.updated_tick = cur_tick;
            self.cells[idx] = remnant;
            self.spawn_gas(ix, iy, GasKind::Smoke, 80.0, 0);
            return true;
        }

        // Ignite flammable neighbors: chance = flammability * wind alignment * temperature factor
        let wind_dir = self.wind.normalize_or_zero();
        let wind_strength = (self.wind.length() / WIND_MAX_SPEED).clamp(0.0, 1.0);
        let weather_mult = self.weather.ignition_multiplier();

        for dy in -1_i32..=1 {
            for dx in -1_i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = ix + dx;
                let ny = iy + dy;
                if !Self::in_bounds(nx, ny) {
                    continue;
                }
                let n_idx = Self::idx(nx as usize, ny as usize);
                let n_mat = self.cells[n_idx].material;

                if n_mat == Material::Ice {
                    self.cells[n_idx].temperature += FIRE_HEAT_NEIGHBOR;
                    self.wake_around(nx, ny);
                    continue;
                }

                let Some(flam) = n_mat.flammability() else {
                    continue;
                };

                self.cells[n_idx].temperature =
                    (self.cells[n_idx].temperature + FIRE_HEAT_NEIGHBOR).min(800.0);

                if n_mat == Material::Gunpowder {
                    self.detonate_gunpowder(nx, ny);
                    continue;
                }

                let dir_vec = Vec2::new(dx as f32, dy as f32).normalize_or_zero();
                let dot = dir_vec.dot(wind_dir);
                let wind_alignment =
                    (WIND_ALIGN_BASE + dot * wind_strength * WIND_ALIGN_WEIGHT).max(WIND_ALIGN_MIN);
                let temp_factor = ((self.cells[n_idx].temperature - AMBIENT_TEMP)
                    / TEMP_FACTOR_SCALE
                    + TEMP_FACTOR_BASE)
                    .clamp(TEMP_FACTOR_MIN, TEMP_FACTOR_MAX);
                let chance = (flam * wind_alignment * temp_factor * weather_mult).clamp(0.0, 1.0);

                if self.rng.gen_bool(chance as f64) {
                    let dur = n_mat.burn_duration();
                    let next_temp = if n_mat == Material::Oil { 700.0 } else { 520.0 };
                    let variant = self.rng.gen::<u8>() & 0b1110;
                    let mut fire = Cell::new(Material::Fire, variant);
                    fire.burn_timer = dur;
                    fire.temperature = next_temp;
                    fire.updated_tick = cur_tick;
                    self.cells[n_idx] = fire;
                    self.wake_around(nx, ny);
                }
            }
        }

        true
    }

    fn step_lava(&mut self, x: usize, y: usize, cur_tick: u32) -> bool {
        let idx = Self::idx(x, y);
        let ix = x as i32;
        let iy = y as i32;
        let mut active = false;

        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let nx = ix + dx;
            let ny = iy + dy;
            if !Self::in_bounds(nx, ny) {
                continue;
            }
            let n_idx = Self::idx(nx as usize, ny as usize);
            let n_cell = self.cells[n_idx];

            if n_cell.material == Material::Stone {
                self.cells[n_idx].temperature += LAVA_STONE_HEAT_RATE;
                if self.cells[n_idx].temperature >= STONE_TO_MAGMA_TEMP {
                    let variant = self.rng.gen::<u8>() & 0b1110;
                    let mut magma = Cell::new(Material::Magma, variant);
                    magma.updated_tick = cur_tick;
                    self.cells[n_idx] = magma;
                }
                self.wake_around(nx, ny);
                active = true;
            } else if n_cell.material == Material::Ice {
                let variant = self.rng.gen::<u8>() & 0b1110;
                let mut shallow = Cell::new(Material::ShallowWater, variant);
                shallow.water_volume = ICE_MELT_WATER_VOL;
                shallow.temperature = 45.0;
                shallow.updated_tick = cur_tick;
                self.cells[n_idx] = shallow;
                self.spawn_gas(nx, ny, GasKind::Steam, 115.0, ICE_STEAM_WATER_VOL);
                self.cells[idx].temperature -= LAVA_ICE_COOL;
                self.wake_around(nx, ny);
                active = true;
            } else if n_cell.material.is_water_passable() && n_cell.water_volume > 0 {
                let boil = n_cell.water_volume.min(LAVA_BOIL_WATER_VOL);
                self.cells[n_idx].water_volume -= boil;
                if self.cells[n_idx].material == Material::ShallowWater
                    && self.cells[n_idx].water_volume == 0
                {
                    let variant = self.rng.gen::<u8>() & 0b1110;
                    let mut stone = Cell::new(Material::Stone, variant);
                    stone.temperature = 140.0;
                    stone.updated_tick = cur_tick;
                    self.cells[n_idx] = stone;
                }
                self.spawn_gas(nx, ny, GasKind::Steam, 120.0, boil);
                self.cells[idx].temperature -= LAVA_WATER_COOL;
                self.wake_around(nx, ny);
                active = true;
            } else if n_cell.material == Material::Gunpowder {
                self.detonate_gunpowder(nx, ny);
                active = true;
            } else if n_cell.material.flammability().is_some() {
                self.cells[n_idx].temperature += 90.0;
                if self.cells[n_idx].temperature >= 180.0 {
                    let dur = n_cell.material.burn_duration();
                    let variant = self.rng.gen::<u8>() & 0b1110;
                    let mut fire = Cell::new(Material::Fire, variant);
                    fire.burn_timer = dur;
                    fire.updated_tick = cur_tick;
                    self.cells[n_idx] = fire;
                }
                self.wake_around(nx, ny);
                active = true;
            }
        }

        if self.cells[idx].temperature < LAVA_SOLIDIFY_TEMP {
            let variant = self.rng.gen::<u8>() & 0b1110;
            let mut stone = Cell::new(Material::Stone, variant);
            stone.temperature = 250.0;
            stone.updated_tick = cur_tick;
            self.cells[idx] = stone;
            return true;
        }

        active
    }

    fn step_acid(&mut self, x: usize, y: usize, cur_tick: u32) -> bool {
        let idx = Self::idx(x, y);
        let ix = x as i32;
        let iy = y as i32;
        let mut dissolved_any = false;

        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let nx = ix + dx;
            let ny = iy + dy;
            if !Self::in_bounds(nx, ny) {
                continue;
            }
            let n_idx = Self::idx(nx as usize, ny as usize);
            let n_mat = self.cells[n_idx].material;
            if matches!(n_mat, Material::Stone | Material::WoodWall | Material::Tree) {
                let next_hp = self.cells[n_idx]
                    .structural_hp
                    .saturating_sub(ACID_CORRODE_DAMAGE);
                self.cells[n_idx].structural_hp = next_hp;
                self.spawn_gas(nx, ny, GasKind::AcidGas, 45.0, 0);

                if next_hp == 0 {
                    let variant = self.rng.gen::<u8>() & 0b1110;
                    let mut dirt = Cell::new(Material::Dirt, variant);
                    dirt.updated_tick = cur_tick;
                    self.cells[n_idx] = dirt;
                }
                self.wake_around(nx, ny);
                self.cells[idx].structural_hp =
                    self.cells[idx].structural_hp.saturating_sub(ACID_SELF_COST);
                dissolved_any = true;
                if self.cells[idx].structural_hp == 0 {
                    break;
                }
            }
        }

        if self.cells[idx].structural_hp == 0 {
            let variant = self.rng.gen::<u8>() & 0b1110;
            let mut dirt = Cell::new(Material::Dirt, variant);
            dirt.updated_tick = cur_tick;
            self.cells[idx] = dirt;
            return true;
        }

        dissolved_any
    }

    fn step_gunpowder(&mut self, x: usize, y: usize) -> bool {
        let idx = Self::idx(x, y);
        let ix = x as i32;
        let iy = y as i32;
        let mut trigger = self.cells[idx].temperature > GUNPOWDER_DETONATE_TEMP;

        if !trigger {
            for dy in -1_i32..=1 {
                for dx in -1_i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    if let Some(nc) = self.get(ix + dx, iy + dy) {
                        if matches!(nc.material, Material::Fire | Material::Lava) {
                            trigger = true;
                            break;
                        }
                    }
                }
                if trigger {
                    break;
                }
            }
        }

        if trigger {
            self.detonate_gunpowder(ix, iy);
            return true;
        }
        false
    }
}
