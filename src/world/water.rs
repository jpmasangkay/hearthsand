use rand::Rng;

use super::*;

impl World {
    pub(super) fn step_water_flow(&mut self, x: usize, y: usize, cur_tick: u32) -> bool {
        let idx = Self::idx(x, y);
        let mut vol = self.cells[idx].water_volume;

        if self.cells[idx].material == Material::ShallowWater && vol < PUDDLE_SOAK_VOLUME {
            if vol > 0 {
                self.absorbed_water += vol as u64;
                self.cells[idx].water_volume = 0;
                self.cells[idx].moisture =
                    (self.cells[idx].moisture + 0.35 + vol as f32 * 0.05).min(1.0);
            } else {
                self.cells[idx].moisture = self.cells[idx].moisture.max(0.55);
            }
            self.cells[idx].material = Material::Dirt;
            self.cells[idx].structural_hp = Material::Dirt.default_hp();
            return true;
        }
        if self.cells[idx].material == Material::DeepWater && vol < WATER_DEEP_MIN_VOLUME {
            self.cells[idx].material = Material::ShallowWater;
        }
        if vol < WATER_MIN_FLOW_DIFF {
            return false;
        }

        const DIRS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let mut targets: [(usize, u8); 4] = [(0, 0); 4];
        let mut count = 0usize;

        for (dx, dy) in DIRS {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if !Self::in_bounds(nx, ny) {
                continue;
            }
            let n_idx = Self::idx(nx as usize, ny as usize);
            let n_cell = self.cells[n_idx];
            if n_cell.material.is_water_passable()
                && n_cell.water_volume + WATER_MIN_FLOW_DIFF <= vol
            {
                targets[count] = (n_idx, n_cell.water_volume);
                count += 1;
            }
        }

        if count == 0 {
            return false;
        }

        let orig_vol = vol;
        let denom = (count + 1) as u16;
        let mut moved_any = false;

        for &(n_idx, n_vol) in targets.iter().take(count) {
            if vol <= n_vol + 1 {
                continue;
            }
            let fair_share = ((orig_vol - n_vol) as u16 / denom).max(1) as u8;
            let max_safe = (vol - n_vol) / 2;
            let transfer = fair_share.min(max_safe);
            if transfer == 0 {
                continue;
            }
            vol -= transfer;
            self.cells[n_idx].water_volume = self.cells[n_idx]
                .water_volume
                .saturating_add(transfer)
                .min(100);
            self.cells[n_idx].moisture = 1.0;
            self.cells[n_idx].updated_tick = cur_tick;
            let nx = (n_idx % GRID_W) as i32;
            let ny = (n_idx / GRID_W) as i32;
            self.wake_around(nx, ny);
            moved_any = true;
        }

        self.cells[idx].water_volume = vol;
        moved_any
    }

    pub(super) fn nearest_water_dist2(&self, x: usize, y: usize) -> u8 {
        let mut best = u8::MAX;
        for dy in -2_i32..=2 {
            for dx in -2_i32..=2 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if !Self::in_bounds(nx, ny) {
                    continue;
                }
                let n_cell = self.cells[Self::idx(nx as usize, ny as usize)];
                if n_cell.water_volume >= WATER_WICK_MIN_VOLUME {
                    let cheb = dx.abs().max(dy.abs()) as u8;
                    if cheb < best {
                        best = cheb;
                    }
                }
            }
        }
        best
    }

    pub(super) fn step_soil_and_crop(&mut self, x: usize, y: usize) -> bool {
        let idx = Self::idx(x, y);
        let prev_moisture = self.cells[idx].moisture;
        let prev_mat = self.cells[idx].material;

        let water_dist = self.nearest_water_dist2(x, y);
        let mut moisture = prev_moisture;

        if self.weather == Weather::Rainstorm {
            moisture = (moisture + RAIN_SOIL_WET_RATE).min(1.0);
        } else if water_dist == 1 {
            moisture = (moisture + WICK_RATE_NEAR).min(WICK_TARGET_NEAR);
        } else if water_dist == 2 {
            if moisture < WICK_TARGET_FAR {
                moisture = (moisture + WICK_RATE_FAR).min(WICK_TARGET_FAR);
            } else {
                moisture = (moisture - MOISTURE_DECAY_RATE * 0.5).max(WICK_TARGET_FAR);
            }
        } else {
            let mut has_wet_neighbor = false;
            for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if let Some(nc) = self.get(nx, ny) {
                    if nc.material == Material::WetSoil || nc.moisture > WET_SOIL_THRESHOLD {
                        has_wet_neighbor = true;
                        break;
                    }
                }
            }
            let heat_scale =
                (1.0 + ((self.cells[idx].temperature - AMBIENT_TEMP) / 25.0).max(0.0)).min(4.0);
            let weather_decay = if self.weather == Weather::Heatwave {
                HEATWAVE_MOISTURE_DECAY_MULT
            } else {
                1.0
            };
            let decay = MOISTURE_DECAY_RATE * heat_scale * weather_decay;
            if has_wet_neighbor && SOIL_DIFFUSE_RATE > decay {
                if moisture < SOIL_DIFFUSE_CAP {
                    moisture = (moisture + (SOIL_DIFFUSE_RATE - decay)).min(SOIL_DIFFUSE_CAP);
                } else {
                    moisture = (moisture - decay).max(SOIL_DIFFUSE_CAP);
                }
            } else {
                let effective_decay = if has_wet_neighbor {
                    (decay - SOIL_DIFFUSE_RATE * 0.5).max(MOISTURE_DECAY_RATE)
                } else {
                    decay
                };
                moisture = (moisture - effective_decay).max(0.0);
            }
        }

        self.cells[idx].moisture = moisture;

        match prev_mat {
            Material::Dirt if moisture > WET_SOIL_THRESHOLD => {
                self.cells[idx].material = Material::WetSoil;
                self.cells[idx].set_tilled(false);
            }
            Material::TilledSoil if moisture > WET_SOIL_THRESHOLD => {
                self.cells[idx].material = Material::WetSoil;
                self.cells[idx].set_tilled(true);
            }
            Material::WetSoil if moisture < DRY_SOIL_REVERT_THRESHOLD => {
                self.cells[idx].material = if self.cells[idx].is_tilled() {
                    Material::TilledSoil
                } else {
                    Material::Dirt
                };
            }
            Material::Crop(stage) => {
                if moisture > WET_SOIL_THRESHOLD {
                    self.cells[idx].set_wet_crop(true);
                } else if moisture < DRY_SOIL_REVERT_THRESHOLD {
                    self.cells[idx].set_wet_crop(false);
                }
                if stage < 4 {
                    let mult = if self.cells[idx].is_wet_crop() {
                        CROP_WET_GROWTH_MULT
                    } else {
                        1.0
                    };
                    self.cells[idx].growth += CROP_GROWTH_PER_TICK * mult;
                    if self.cells[idx].growth >= 1.0 {
                        self.cells[idx].growth -= 1.0;
                        self.cells[idx].material = Material::Crop((stage + 1).min(4));
                    }
                    return true;
                }
            }
            _ => {}
        }

        self.cells[idx].material != prev_mat || (moisture - prev_moisture).abs() > 0.0005
    }

    pub(super) fn step_gas(&mut self, dt: f32) {
        let max_x = GRID_W as f32 * TILE_PX - 1.0;
        let max_y = GRID_H as f32 * TILE_PX - 1.0;
        let mut condensed = Vec::new();
        let mut i = 0;

        while i < self.gas.len() {
            let g = &mut self.gas[i];
            g.pos += self.wind * dt;
            g.pos.x = g.pos.x.clamp(1.0, max_x);
            g.pos.y = g.pos.y.clamp(1.0, max_y);
            g.height += g.v_height * dt;
            g.v_height *= 0.985;
            g.temperature -= STEAM_COOL_RATE * dt;
            g.ttl -= dt;

            let expired =
                g.ttl <= 0.0 || (g.kind == GasKind::Steam && g.temperature <= STEAM_CONDENSE_TEMP);

            if expired {
                let item = self.gas.swap_remove(i);
                if item.water_volume > 0 {
                    condensed.push((item.pos, item.water_volume));
                }
            } else {
                i += 1;
            }
        }

        for (pos, water) in condensed {
            let tx = (pos.x / TILE_PX).floor() as i32;
            let ty = (pos.y / TILE_PX).floor() as i32;
            self.deposit_water_conserving(tx, ty, water);
        }
    }

    fn for_each_in_ring(cx: i32, cy: i32, r: i32, mut f: impl FnMut(i32, i32) -> bool) -> bool {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let x = cx + dx;
                let y = cy + dy;
                if Self::in_bounds(x, y) && !f(x, y) {
                    return false;
                }
            }
        }
        true
    }

    fn deposit_into_water_ring(&mut self, cx: i32, cy: i32, r: i32, amount: &mut u8) {
        Self::for_each_in_ring(cx, cy, r, |x, y| {
            let idx = Self::idx(x as usize, y as usize);
            if self.cells[idx].material.is_water_passable() && self.cells[idx].water_volume < 100 {
                let add = (*amount).min(100 - self.cells[idx].water_volume);
                self.cells[idx].water_volume += add;
                self.cells[idx].moisture = 1.0;
                self.wake_around(x, y);
                *amount -= add;
            }
            *amount > 0
        });
    }

    fn deposit_into_ground_ring(&mut self, cx: i32, cy: i32, r: i32, amount: &mut u8) {
        Self::for_each_in_ring(cx, cy, r, |x, y| {
            let idx = Self::idx(x as usize, y as usize);
            let mat = self.cells[idx].material;
            // Never overwrite TilledSoil, WetSoil, or Crop cells
            if matches!(
                mat,
                Material::Sand | Material::Dirt | Material::Ash | Material::Grass
            ) {
                let give = (*amount).min(100);
                let variant = self.rng.gen::<u8>() & 0b1110;
                let mut shallow = Cell::new(Material::ShallowWater, variant);
                shallow.water_volume = give;
                shallow.updated_tick = self.tick;
                self.cells[idx] = shallow;
                self.wake_around(x, y);
                self.last_terrain_change_tick = self.tick;
                *amount -= give;
            }
            *amount > 0
        });
    }

    fn soak_into_soil_ring(&mut self, cx: i32, cy: i32, r: i32, amount: &mut u8) {
        Self::for_each_in_ring(cx, cy, r, |x, y| {
            let idx = Self::idx(x as usize, y as usize);
            let mat = self.cells[idx].material;
            if matches!(
                mat,
                Material::Sand
                    | Material::Dirt
                    | Material::Ash
                    | Material::Grass
                    | Material::TilledSoil
                    | Material::WetSoil
                    | Material::Crop(_)
            ) {
                let add = *amount;
                let wet_boost = (add as f32 * 0.08).clamp(0.05, 0.45);
                self.cells[idx].moisture = (self.cells[idx].moisture + wet_boost).min(1.0);
                self.absorbed_water += add as u64;
                self.wake_around(x, y);
                *amount = 0;
            }
            *amount > 0
        });
    }

    pub(super) fn deposit_water_conserving(&mut self, cx: i32, cy: i32, mut amount: u8) {
        if amount == 0 {
            return;
        }
        let max_r = GRID_W.max(GRID_H) as i32;

        // 1. Prefer existing nearby water cells (DeepWater / ShallowWater / Trench)
        for r in 0..=WATER_CONDENSE_NEAR_RADIUS {
            self.deposit_into_water_ring(cx, cy, r, &mut amount);
            if amount == 0 {
                return;
            }
        }

        // 2. Puddles under 6 volume soak into the soil as moisture & count as absorbed_water
        if amount < PUDDLE_SOAK_VOLUME {
            for r in 0..=WATER_CONDENSE_NEAR_RADIUS {
                self.soak_into_soil_ring(cx, cy, r, &mut amount);
                if amount == 0 {
                    return;
                }
            }
        }

        // 3. Condense larger amounts onto Sand / Dirt / Ash / Grass (never TilledSoil/WetSoil/Crop)
        if amount >= PUDDLE_SOAK_VOLUME {
            for r in 0..=WATER_CONDENSE_NEAR_RADIUS {
                self.deposit_into_ground_ring(cx, cy, r, &mut amount);
                if amount == 0 {
                    return;
                }
            }
        }

        // 4. Wider search: existing water first, then Sand / Dirt / Ash / Grass, then soil soak
        for r in (WATER_CONDENSE_NEAR_RADIUS + 1)..=max_r {
            self.deposit_into_water_ring(cx, cy, r, &mut amount);
            if amount == 0 {
                return;
            }
        }
        if amount >= PUDDLE_SOAK_VOLUME {
            for r in (WATER_CONDENSE_NEAR_RADIUS + 1)..=max_r {
                self.deposit_into_ground_ring(cx, cy, r, &mut amount);
                if amount == 0 {
                    return;
                }
            }
        }
        for r in 0..=max_r {
            self.soak_into_soil_ring(cx, cy, r, &mut amount);
            if amount == 0 {
                return;
            }
        }

        // 5. Fallback: preserve any remaining water in Steam so water is never silently deleted
        if amount > 0 {
            self.spawn_gas(cx, cy, GasKind::Steam, 75.0, amount);
        }
    }

    pub(super) fn step_debris(&mut self, dt: f32) {
        let mut landed = Vec::new();
        let mut i = 0;
        while i < self.debris.len() {
            let d = &mut self.debris[i];
            d.pos += d.vel * dt;
            d.height += d.v_height * dt;
            d.v_height -= DEBRIS_GRAVITY * dt;
            d.vel *= 0.99;

            if d.height <= 0.0 {
                let item = self.debris.swap_remove(i);
                landed.push(item);
            } else {
                i += 1;
            }
        }

        for item in landed {
            let tx = (item.pos.x / TILE_PX).floor() as i32;
            let ty = (item.pos.y / TILE_PX).floor() as i32;
            if let Some((dx, dy)) = self.find_nearest_deposit_cell(tx, ty) {
                let displaced_water = self.get(dx, dy).map_or(0, |c| c.water_volume);
                self.set_material(dx, dy, item.material);
                if displaced_water > 0 {
                    self.deposit_water_conserving(dx, dy, displaced_water);
                }
            }
        }
    }

    fn find_nearest_deposit_cell(&self, cx: i32, cy: i32) -> Option<(i32, i32)> {
        let mut best: Option<(i32, i32, i32)> = None;
        for r in 0..=2_i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    let x = cx + dx;
                    let y = cy + dy;
                    if let Some(cell) = self.get(x, y) {
                        if cell.material.can_deposit_debris() {
                            let d_sq = dx * dx + dy * dy;
                            if best.is_none_or(|(_, _, bd)| d_sq < bd) {
                                best = Some((x, y, d_sq));
                            }
                        }
                    }
                }
            }
        }
        best.map(|(x, y, _)| (x, y))
    }
}
