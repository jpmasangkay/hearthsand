use std::collections::VecDeque;

use macroquad::prelude::Vec2;
use noise::{Fbm, MultiFractal, Perlin};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

mod ca;
mod gen;
mod ops;
mod types;
mod water;

#[cfg(test)]
mod tests;

pub use types::*;

pub const GRID_W: usize = 160;
pub const GRID_H: usize = 100;
pub const TILE_PX: f32 = 16.0;
pub const CHUNK_SIZE: usize = 16;
pub const CHUNKS_X: usize = GRID_W.div_ceil(CHUNK_SIZE);
pub const CHUNKS_Y: usize = GRID_H.div_ceil(CHUNK_SIZE);

pub const SIM_DT: f32 = 1.0 / 60.0;
pub const DAY_CYCLE_SECS: f32 = 90.0;
pub const AMBIENT_TEMP: f32 = 20.0;
pub const HEATWAVE_AMBIENT_TEMP: f32 = 38.0;

// Water & irrigation reaction rates
pub const WATER_DEEP_MIN_VOLUME: u8 = 40;
pub const WATER_MIN_FLOW_DIFF: u8 = 2;
pub const WATER_WICK_MIN_VOLUME: u8 = 10;
pub const WICK_TARGET_NEAR: f32 = 1.0;
pub const WICK_TARGET_FAR: f32 = 0.78;
pub const WICK_RATE_NEAR: f32 = 0.016;
pub const WICK_RATE_FAR: f32 = 0.009;
pub const SOIL_DIFFUSE_CAP: f32 = 0.25;
pub const SOIL_DIFFUSE_RATE: f32 = 0.002;
pub const MOISTURE_DECAY_RATE: f32 = 0.0012;
pub const WET_SOIL_THRESHOLD: f32 = 0.6;
pub const DRY_SOIL_REVERT_THRESHOLD: f32 = 0.3;
pub const CROP_GROWTH_PER_TICK: f32 = 0.0007;
pub const CROP_WET_GROWTH_MULT: f32 = 4.0;
pub const TEMP_EQUILIBRATE_RATE: f32 = 0.015;
pub const DEBRIS_GRAVITY: f32 = 260.0;

// Wind & weather constants
pub const WIND_MIN_SPEED: f32 = 6.0;
pub const WIND_MAX_SPEED: f32 = 28.0;
pub const WIND_ALIGN_BASE: f32 = 1.0;
pub const WIND_ALIGN_WEIGHT: f32 = 1.25;
pub const WIND_ALIGN_MIN: f32 = 0.15;
pub const RAIN_SOIL_WET_RATE: f32 = 0.06;
pub const RAIN_WATER_ADD_CHANCE: f64 = 0.05;
pub const HEATWAVE_IGNITION_MULT: f32 = 2.0;
pub const HEATWAVE_MOISTURE_DECAY_MULT: f32 = 2.0;
pub const RAIN_IGNITION_MULT: f32 = 0.05;

// Fire flammability & lifecycle constants (chance = flammability * wind_alignment * temp_factor)
pub const FLAMMABILITY_GUNPOWDER: f32 = 0.85;
pub const FLAMMABILITY_OIL: f32 = 0.45;
pub const FLAMMABILITY_GRASS: f32 = 0.055;
pub const FLAMMABILITY_CROP: f32 = 0.06;
pub const FLAMMABILITY_BUSH: f32 = 0.05;
pub const FLAMMABILITY_TREE: f32 = 0.032;
pub const FLAMMABILITY_WOOD_WALL: f32 = 0.028;
pub const FLAMMABILITY_CABIN: f32 = 0.022;
pub const TEMP_FACTOR_BASE: f32 = 0.65;
pub const TEMP_FACTOR_SCALE: f32 = 220.0;
pub const TEMP_FACTOR_MIN: f32 = 0.4;
pub const TEMP_FACTOR_MAX: f32 = 2.6;
pub const FIRE_HEAT_NEIGHBOR: f32 = 14.0;
pub const BURN_TIME_GRASS: f32 = 1.6;
pub const BURN_TIME_WOOD: f32 = 3.0;
pub const BURN_TIME_OIL: f32 = 3.5;
pub const BURN_TIME_DEFAULT: f32 = 2.0;
pub const SMOKE_EMIT_CHANCE: f64 = 0.08;
pub const FIRE_BOIL_WATER_VOL: u8 = 15;

// Steam, Lava, Ice, Acid & Gunpowder constants
pub const PUDDLE_SOAK_VOLUME: u8 = 6;
pub const WATER_CONDENSE_NEAR_RADIUS: i32 = 6;
pub const HEATWAVE_ICE_MELT_SECS: f32 = 10.0;
pub const HEATWAVE_ICE_WARM_PER_TICK: f32 = 10.0 / (HEATWAVE_ICE_MELT_SECS * 60.0);
pub const STEAM_RISE_SPEED: f32 = 16.0;
pub const STEAM_COOL_RATE: f32 = 26.0;
pub const STEAM_CONDENSE_TEMP: f32 = 42.0;
pub const LAVA_BOIL_WATER_VOL: u8 = 25;
pub const LAVA_WATER_COOL: f32 = 300.0;
pub const LAVA_ICE_COOL: f32 = 340.0;
pub const LAVA_SOLIDIFY_TEMP: f32 = 600.0;
pub const LAVA_STONE_HEAT_RATE: f32 = 180.0;
pub const STONE_TO_MAGMA_TEMP: f32 = 340.0;
pub const MAGMA_SOLIDIFY_TEMP: f32 = 280.0;
pub const ICE_MELT_WATER_VOL: u8 = 55;
pub const ICE_STEAM_WATER_VOL: u8 = 20;
pub const ACID_CORRODE_DAMAGE: u8 = 15;
pub const ACID_SELF_COST: u8 = 2;
pub const GUNPOWDER_DETONATE_TEMP: f32 = 200.0;
pub const GUNPOWDER_EXPLODE_RADIUS: i32 = 4;
pub const GUNPOWDER_EXPLODE_POWER: f32 = 72.0;
pub const MAX_EXPLOSIONS_PER_TICK: usize = 8;

pub struct World {
    pub preset: Preset,
    pub seed: u64,
    pub tick: u32,
    pub time_of_day: f32,
    pub wind: Vec2,
    pub weather: Weather,
    pub cells: Vec<Cell>,
    pub chunk_awake: Vec<u8>,
    pub absorbed_water: u64,
    #[cfg(test)]
    pub rain_water_added: u64,
    pub debris: Vec<Debris>,
    pub gas: Vec<GasParticle>,
    pub pending_explosions: VecDeque<PendingExplosion>,
    pub last_tick_explosions: usize,
    pub shake_impulse: f32,
    pub rng: StdRng,
    pub last_terrain_change_tick: u32,
}

impl World {
    pub fn new(preset: Preset, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let fbm_elev: Fbm<Perlin> = Fbm::new(seed as u32).set_octaves(4);
        let fbm_moist: Fbm<Perlin> = Fbm::new((seed.wrapping_add(1337)) as u32).set_octaves(3);

        let mut cells = Vec::with_capacity(GRID_W * GRID_H);
        for y in 0..GRID_H {
            for x in 0..GRID_W {
                let variant = rng.gen::<u8>() & 0b1110;
                let mat = gen::generate_tile(preset, x, y, &fbm_elev, &fbm_moist, &mut rng);
                cells.push(Cell::new(mat, variant));
            }
        }

        let mut world = Self {
            preset,
            seed,
            tick: 0,
            time_of_day: 0.28,
            wind: Vec2::new(14.0, -3.0),
            weather: Weather::Clear,
            cells,
            chunk_awake: vec![4; CHUNKS_X * CHUNKS_Y],
            absorbed_water: 0,
            #[cfg(test)]
            rain_water_added: 0,
            debris: Vec::new(),
            gas: Vec::new(),
            pending_explosions: VecDeque::new(),
            last_tick_explosions: 0,
            shake_impulse: 0.0,
            rng,
            last_terrain_change_tick: 0,
        };

        world.stamp_starter_settlement(preset);
        world.prewarm_moisture();
        world
    }

    pub fn is_night(&self) -> bool {
        self.time_of_day < 0.20 || self.time_of_day >= 0.72
    }

    pub fn day_phase_name(&self) -> &'static str {
        if (0.20..0.28).contains(&self.time_of_day) {
            "Dawn"
        } else if (0.28..0.64).contains(&self.time_of_day) {
            "Day"
        } else if (0.64..0.72).contains(&self.time_of_day) {
            "Dusk"
        } else {
            "Night"
        }
    }

    pub fn advance_time_phase(&mut self) {
        self.time_of_day = (self.time_of_day + 0.25).rem_euclid(1.0);
    }

    #[inline]
    pub fn in_bounds(x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < GRID_W && (y as usize) < GRID_H
    }

    #[inline]
    pub(super) fn idx(x: usize, y: usize) -> usize {
        y * GRID_W + x
    }

    #[inline]
    pub(super) fn chunk_idx(cx: usize, cy: usize) -> usize {
        cy * CHUNKS_X + cx
    }

    pub fn get(&self, x: i32, y: i32) -> Option<&Cell> {
        if !Self::in_bounds(x, y) {
            return None;
        }
        Some(&self.cells[Self::idx(x as usize, y as usize)])
    }

    pub fn get_mut(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        if !Self::in_bounds(x, y) {
            return None;
        }
        Some(&mut self.cells[Self::idx(x as usize, y as usize)])
    }

    pub fn set(&mut self, x: i32, y: i32, cell: Cell) -> bool {
        if !Self::in_bounds(x, y) {
            return false;
        }
        let ux = x as usize;
        let uy = y as usize;
        self.cells[Self::idx(ux, uy)] = cell;
        self.wake_around(x, y);
        self.last_terrain_change_tick = self.tick;
        true
    }

    pub fn set_material(&mut self, x: i32, y: i32, material: Material) -> bool {
        let Some(&prev) = self.get(x, y) else {
            return false;
        };
        let variant_bits = self.rng.gen::<u8>() & 0b1110;
        let mut cell = Cell::new(material, variant_bits);

        match material {
            Material::TilledSoil => {
                cell.moisture = prev.moisture.min(0.55);
            }
            Material::WetSoil => {
                cell.set_tilled(prev.material == Material::TilledSoil || prev.is_tilled());
                cell.moisture = prev.moisture.max(0.85);
            }
            Material::Crop(_) => {
                let wet_under =
                    prev.material == Material::WetSoil || prev.moisture > WET_SOIL_THRESHOLD;
                cell.moisture = prev.moisture;
                cell.set_wet_crop(wet_under);
            }
            Material::Trench => {
                cell.water_volume = if prev.material.is_water_passable() {
                    prev.water_volume
                } else {
                    0
                };
                cell.moisture = prev.moisture;
            }
            Material::Dirt | Material::Sand | Material::Grass => {
                cell.moisture = prev.moisture;
            }
            _ => {}
        }

        cell.updated_tick = self.tick;
        self.set(x, y, cell)
    }

    pub fn wake_around(&mut self, x: i32, y: i32) {
        let min_x = (x - 2).max(0) as usize / CHUNK_SIZE;
        let max_x = ((x + 2).min(GRID_W as i32 - 1).max(0) as usize / CHUNK_SIZE).min(CHUNKS_X - 1);
        let min_y = (y - 2).max(0) as usize / CHUNK_SIZE;
        let max_y = ((y + 2).min(GRID_H as i32 - 1).max(0) as usize / CHUNK_SIZE).min(CHUNKS_Y - 1);
        for cy in min_y..=max_y {
            for cx in min_x..=max_x {
                self.chunk_awake[Self::chunk_idx(cx, cy)] = 4;
            }
        }
    }

    pub fn wake_chunk(&mut self, cx: usize, cy: usize) {
        if cx < CHUNKS_X && cy < CHUNKS_Y {
            self.chunk_awake[Self::chunk_idx(cx, cy)] = 4;
        }
    }

    pub fn active_chunk_count(&self) -> usize {
        self.chunk_awake.iter().filter(|&&t| t > 0).count()
    }

    #[cfg(test)]
    pub fn total_water_units(&self) -> u64 {
        let grid_water: u64 = self.cells.iter().map(|c| c.water_volume as u64).sum();
        let gas_water: u64 = self.gas.iter().map(|g| g.water_volume as u64).sum();
        grid_water + gas_water + self.absorbed_water
    }

    pub fn spawn_gas(
        &mut self,
        tx: i32,
        ty: i32,
        kind: GasKind,
        temperature: f32,
        water_volume: u8,
    ) {
        if kind != GasKind::Steam && self.gas.len() >= 900 {
            if let Some(pos) = self.gas.iter().position(|g| g.kind != GasKind::Steam) {
                self.gas.swap_remove(pos);
            }
        }
        let jitter_x = self.rng.gen_range(3.0..13.0);
        let jitter_y = self.rng.gen_range(3.0..13.0);
        let max_x = GRID_W as f32 * TILE_PX - 1.0;
        let max_y = GRID_H as f32 * TILE_PX - 1.0;
        let pos = Vec2::new(
            (tx as f32 * TILE_PX + jitter_x).clamp(1.0, max_x),
            (ty as f32 * TILE_PX + jitter_y).clamp(1.0, max_y),
        );
        let ttl = match kind {
            GasKind::Steam => 5.5,
            GasKind::Smoke => 2.8,
            GasKind::AcidGas => 1.8,
        };
        self.gas.push(GasParticle {
            pos,
            height: 2.0,
            v_height: STEAM_RISE_SPEED + self.rng.gen_range(0.0..6.0),
            temperature,
            ttl,
            water_volume,
            kind,
        });
    }

    pub fn step(&mut self) {
        self.tick = self.tick.wrapping_add(1).max(1);
        self.time_of_day = (self.time_of_day + SIM_DT / DAY_CYCLE_SECS).rem_euclid(1.0);
        let cur_tick = self.tick;
        self.step_wind();

        // Drain queued gunpowder chain explosions (max 8 per tick)
        let pop_count = self.pending_explosions.len().min(MAX_EXPLOSIONS_PER_TICK);
        self.last_tick_explosions = 0;
        for _ in 0..pop_count {
            let Some(exp) = self.pending_explosions.pop_front() else {
                break;
            };
            self.last_tick_explosions += 1;
            self.explode(exp.x, exp.y, exp.radius, exp.power);
        }

        // Apply rolling weather stripe across the grid (covers all 100 rows every 10 ticks)
        self.step_weather_stripe(cur_tick);

        let left_to_right = cur_tick.is_multiple_of(2);
        let top_to_bottom = (cur_tick / 2).is_multiple_of(2);

        let cur_awake = self.chunk_awake.clone();
        for t in &mut self.chunk_awake {
            *t = t.saturating_sub(1);
        }

        for cy_step in 0..CHUNKS_Y {
            let cy = if top_to_bottom {
                cy_step
            } else {
                CHUNKS_Y - 1 - cy_step
            };
            for cx_step in 0..CHUNKS_X {
                let cx = if left_to_right {
                    cx_step
                } else {
                    CHUNKS_X - 1 - cx_step
                };
                if cur_awake[Self::chunk_idx(cx, cy)] == 0 {
                    continue;
                }

                let x_start = cx * CHUNK_SIZE;
                let x_end = (x_start + CHUNK_SIZE).min(GRID_W);
                let y_start = cy * CHUNK_SIZE;
                let y_end = (y_start + CHUNK_SIZE).min(GRID_H);

                for y_off in 0..(y_end - y_start) {
                    let y = if top_to_bottom {
                        y_start + y_off
                    } else {
                        y_end - 1 - y_off
                    };
                    for x_off in 0..(x_end - x_start) {
                        let x = if left_to_right {
                            x_start + x_off
                        } else {
                            x_end - 1 - x_off
                        };
                        self.step_cell(x, y, cur_tick);
                    }
                }
            }
        }

        self.step_gas(SIM_DT);
        self.step_debris(SIM_DT);
    }
}
