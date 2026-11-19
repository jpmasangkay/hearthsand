use noise::{Fbm, NoiseFn, Perlin};
use rand::rngs::StdRng;
use rand::Rng;

use super::*;

impl World {
    pub fn settlement_center(preset: Preset) -> (i32, i32) {
        match preset {
            Preset::LushRiverIsland => (86_i32, 50_i32),
            Preset::VolcanicCrater => (80_i32, 76_i32),
            Preset::DroughtValley => (80_i32, 50_i32),
        }
    }

    pub fn village_center(&self) -> (i32, i32) {
        Self::settlement_center(self.preset)
    }

    pub(super) fn stamp_starter_settlement(&mut self, preset: Preset) {
        let (cx, cy) = Self::settlement_center(preset);

        // Clear a cozy village plaza
        for dy in -5..=5 {
            for dx in -6..=6 {
                let x = cx + dx;
                let y = cy + dy;
                if !Self::in_bounds(x, y) {
                    continue;
                }
                let mat = if dx.abs() <= 2 && dy.abs() <= 2 {
                    Material::Dirt
                } else {
                    match preset {
                        Preset::DroughtValley => Material::Dirt,
                        _ => Material::Grass,
                    }
                };
                self.set_material(x, y, mat);
            }
        }

        // Starter structures
        self.set_material(cx, cy - 3, Material::Cabin);
        self.set_material(cx + 2, cy - 2, Material::Chest);
        self.set_material(cx, cy, Material::Campfire);
        for dx in -2..=2 {
            if dx != 0 {
                self.set_material(cx + dx, cy - 4, Material::WoodWall);
            }
        }

        // Starter dry farm plot west of plaza (near river/stream to test trench irrigation)
        let farm_x0 = cx - 12;
        let farm_y0 = cy + 1;
        for fy in 0..4 {
            for fx in 0..5 {
                let x = farm_x0 + fx;
                let y = farm_y0 + fy;
                if fy == 1 && (1..=3).contains(&fx) {
                    self.set_material(x, y, Material::Crop(1));
                } else if fy == 2 && (1..=2).contains(&fx) {
                    self.set_material(x, y, Material::Crop(2));
                } else {
                    self.set_material(x, y, Material::TilledSoil);
                }
            }
        }

        // For Volcanic Crater Village, add mountain springs near the plaza and west of the farm for firefighting/irrigation
        if preset == Preset::VolcanicCrater {
            for (sx_center, sy_center) in [(cx + 9, cy), (cx - 15, cy + 2)] {
                for dy in -1_i32..=1 {
                    for dx in -1_i32..=1 {
                        let sx = sx_center + dx;
                        let sy = sy_center + dy;
                        let mat = if dx == 0 && dy == 0 {
                            Material::DeepWater
                        } else {
                            Material::ShallowWater
                        };
                        self.set_material(sx, sy, mat);
                    }
                }
            }
            self.set_material(cx + 7, cy + 2, Material::BerryBush);
            self.set_material(cx - 7, cy - 2, Material::BerryBush);
        }
    }

    pub(super) fn prewarm_moisture(&mut self) {
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                let dist = self.nearest_water_dist2(x, y);
                let idx = Self::idx(x, y);
                if dist == 1 {
                    self.cells[idx].moisture = 0.95;
                } else if dist == 2 {
                    self.cells[idx].moisture = 0.72;
                }
                let cell = &mut self.cells[idx];
                if matches!(cell.material, Material::Dirt | Material::TilledSoil)
                    && cell.moisture > WET_SOIL_THRESHOLD
                {
                    let was_tilled = cell.material == Material::TilledSoil;
                    cell.material = Material::WetSoil;
                    cell.set_tilled(was_tilled);
                }
            }
        }
    }
}

pub(super) fn generate_tile(
    preset: Preset,
    x: usize,
    y: usize,
    fbm_elev: &Fbm<Perlin>,
    fbm_moist: &Fbm<Perlin>,
    rng: &mut StdRng,
) -> Material {
    let nx = x as f64 * 0.04;
    let ny = y as f64 * 0.04;
    let elev = fbm_elev.get([nx, ny]) as f32;
    let moist = fbm_moist.get([nx * 1.4 + 17.3, ny * 1.4 - 9.1]) as f32;

    let fx = (x as f32 / GRID_W as f32) * 2.0 - 1.0;
    let fy = (y as f32 / GRID_H as f32) * 2.0 - 1.0;
    let radial = (fx * fx * 0.85 + fy * fy).sqrt();

    match preset {
        Preset::LushRiverIsland => {
            let (vcx, vcy) = World::settlement_center(Preset::LushRiverIsland);
            let vdx = x as i32 - vcx;
            let vdy = y as i32 - vcy;
            let near_village = vdx * vdx + vdy * vdy <= 12 * 12;

            // Meander the river with sin + Fbm while keeping clear of the starter farm plot (x = 74..78)
            let dy_norm = ((y as f32 - vcy as f32) / 36.0).clamp(-1.0, 1.0);
            let env = 0.48 + 0.52 * (dy_norm * dy_norm);
            let fbm_meander = fbm_elev.get([0.41, y as f64 * 0.032]) as f32;
            let sin_meander = ((y as f32 - vcy as f32) * 0.088).sin() * 8.4
                + ((y as f32 - vcy as f32) * 0.17).sin() * 2.6;
            let raw_river = 68.5 + (sin_meander + fbm_meander * 6.5 + elev * 2.8) * env;
            let river_center = if (46..=56).contains(&y) {
                raw_river.clamp(63.0, 70.2)
            } else {
                raw_river.clamp(54.0, 79.0)
            };
            let river_dist = (x as f32 - river_center).abs();
            let island_height = 0.72 - radial * 0.88 + elev * 0.38;
            let outcrop_detail = fbm_elev.get([nx * 3.1 + 13.7, ny * 3.1 - 8.3]) as f32;
            let outcrop_field = outcrop_detail * 0.85 - moist * 0.18;

            if island_height < -0.14 || river_dist < 1.6 {
                Material::DeepWater
            } else if island_height < -0.02 || river_dist < 2.8 {
                Material::ShallowWater
            } else if island_height < 0.07 || river_dist < 3.9 {
                Material::Sand
            } else if !near_village
                && island_height > 0.14
                && river_dist > 5.0
                && outcrop_field > 0.340
            {
                Material::Stone
            } else if moist > 0.22 && rng.gen_bool(0.24) {
                Material::Tree
            } else if moist > 0.05 && rng.gen_bool(0.06) {
                Material::BerryBush
            } else if moist < -0.24 {
                Material::Dirt
            } else {
                Material::Grass
            }
        }
        Preset::VolcanicCrater => {
            let (vcx, vcy) = World::settlement_center(Preset::VolcanicCrater);
            let vdx = x as i32 - vcx;
            let vdy = y as i32 - vcy;
            let v_dist_sq = vdx * vdx + vdy * vdy;

            let wx = fbm_elev.get([nx * 0.85 + 31.7, ny * 0.85 - 19.3]) as f32;
            let wy = fbm_moist.get([nx * 0.85 - 11.4, ny * 0.85 + 27.9]) as f32;
            let fx_w = fx + wx * 0.13;
            let fy_w = fy + wy * 0.13;
            let crater_dist = ((fx_w * 1.35).powi(2) + (fy_w * 1.1 + 0.15).powi(2)).sqrt();
            let ring_warp = elev * 0.08 + wx * 0.05;

            if v_dist_sq > 10 * 10 && crater_dist < 0.17 + ring_warp {
                Material::Lava
            } else if v_dist_sq > 10 * 10 && crater_dist < 0.27 + ring_warp {
                Material::Magma
            } else if crater_dist < 0.41 + ring_warp {
                Material::Stone
            } else if crater_dist < 0.56 + ring_warp {
                Material::Ash
            } else if radial + wx * 0.07 > 0.92 {
                Material::ShallowWater
            } else if v_dist_sq > 12 * 12 && elev > 0.26 && moist < 0.08 {
                // Scattered basalt outcrops on the slopes
                Material::Stone
            } else if v_dist_sq > 10 * 10 && crater_dist < 0.78 && (elev * 0.65 + wy * 0.55) > 0.18
            {
                // Wind-blown ash drifts extending down the foothills
                Material::Ash
            } else if moist > 0.2 && rng.gen_bool(0.14) {
                Material::Tree
            } else if elev > 0.15 {
                Material::Dirt
            } else {
                Material::Grass
            }
        }
        Preset::DroughtValley => {
            let (vcx, vcy) = World::settlement_center(Preset::DroughtValley);
            let vdx = x as i32 - vcx;
            let vdy = y as i32 - vcy;
            let near_village = vdx * vdx + vdy * vdy <= 12 * 12;

            let stream_center = 54.0 + (y as f32 * 0.09).cos() * 10.0 + elev * 5.0;
            let stream_dist = (x as f32 - stream_center).abs();
            let edge_warp = fbm_elev.get([nx * 0.75 - 23.1, ny * 0.75 + 14.7]) as f32;
            let edge = radial + edge_warp * 0.14 + moist * 0.06;

            if stream_dist < 0.85 {
                Material::DeepWater
            } else if stream_dist < 1.9 || edge > 0.98 {
                Material::ShallowWater
            } else if edge > 0.90 {
                Material::Sand
            } else if !near_village
                && ((edge > 0.80 && (elev + edge_warp * 0.5) > -0.06) || elev > 0.48)
            {
                Material::Stone
            } else if stream_dist < 5.0 {
                Material::Dirt
            } else if moist > 0.25 && rng.gen_bool(0.08) {
                Material::BerryBush
            } else if moist > 0.35 && rng.gen_bool(0.08) {
                Material::Tree
            } else if elev < -0.05 {
                Material::Sand
            } else {
                Material::Dirt
            }
        }
    }
}
