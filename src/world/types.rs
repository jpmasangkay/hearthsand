use macroquad::prelude::{Color, Vec2};

use super::*;

const VARIANT_TILLED_BIT: u8 = 1 << 0;
const VARIANT_WET_CROP_BIT: u8 = 1 << 0;
const VARIANT_CAMPFIRE_LIT_BIT: u8 = 1 << 0;
const VARIANT_BUSH_EMPTY_BIT: u8 = 1 << 2;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    DeepWater,
    ShallowWater,
    Sand,
    Dirt,
    Trench,
    TilledSoil,
    WetSoil,
    Grass,
    Tree,
    BerryBush,
    Crop(u8),
    Stone,
    Magma,
    WoodWall,
    Cabin,
    Campfire,
    Chest,
    Ice,
    Fire,
    Smoke,
    Steam,
    Lava,
    Acid,
    Gunpowder,
    Oil,
    Ash,
}

impl Material {
    #[cfg(test)]
    pub const PALETTE: [Material; 26] = [
        Material::DeepWater,
        Material::ShallowWater,
        Material::Sand,
        Material::Dirt,
        Material::Trench,
        Material::TilledSoil,
        Material::WetSoil,
        Material::Grass,
        Material::Tree,
        Material::BerryBush,
        Material::Crop(1),
        Material::Stone,
        Material::Magma,
        Material::WoodWall,
        Material::Cabin,
        Material::Campfire,
        Material::Chest,
        Material::Ice,
        Material::Fire,
        Material::Smoke,
        Material::Steam,
        Material::Lava,
        Material::Acid,
        Material::Gunpowder,
        Material::Oil,
        Material::Ash,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Material::DeepWater => "Deep Water",
            Material::ShallowWater => "Shallow Water",
            Material::Sand => "Sand",
            Material::Dirt => "Dirt",
            Material::Trench => "Trench",
            Material::TilledSoil => "Tilled Soil",
            Material::WetSoil => "Wet Soil",
            Material::Grass => "Grass",
            Material::Tree => "Tree",
            Material::BerryBush => "Berry Bush",
            Material::Crop(1) => "Crop (Sprout)",
            Material::Crop(2) => "Crop (Leafy)",
            Material::Crop(3) => "Crop (Flowering)",
            Material::Crop(4) => "Crop (Golden)",
            Material::Crop(_) => "Crop",
            Material::Stone => "Stone",
            Material::Magma => "Magma",
            Material::WoodWall => "Wood Wall",
            Material::Cabin => "Cabin",
            Material::Campfire => "Campfire",
            Material::Chest => "Chest",
            Material::Ice => "Ice",
            Material::Fire => "Fire",
            Material::Smoke => "Smoke",
            Material::Steam => "Steam",
            Material::Lava => "Lava",
            Material::Acid => "Acid",
            Material::Gunpowder => "Gunpowder",
            Material::Oil => "Oil",
            Material::Ash => "Ash",
        }
    }

    pub fn default_hp(self) -> u8 {
        match self {
            Material::Stone | Material::Magma => 120,
            Material::Cabin => 90,
            Material::WoodWall | Material::Chest | Material::Acid => 60,
            Material::Tree => 50,
            Material::Campfire => 40,
            Material::Ice => 35,
            Material::BerryBush => 25,
            Material::Dirt
            | Material::Sand
            | Material::Grass
            | Material::Trench
            | Material::TilledSoil
            | Material::WetSoil => 20,
            Material::Crop(_) | Material::Gunpowder | Material::Ash => 10,
            Material::DeepWater
            | Material::ShallowWater
            | Material::Fire
            | Material::Smoke
            | Material::Steam
            | Material::Lava
            | Material::Oil => 5,
        }
    }

    pub fn default_temp(self) -> f32 {
        match self {
            Material::Lava => 1200.0,
            Material::Magma => 650.0,
            Material::Fire => 520.0,
            Material::Campfire => 180.0,
            Material::Steam => 115.0,
            Material::Smoke => 85.0,
            Material::Ice => -10.0,
            _ => AMBIENT_TEMP,
        }
    }

    pub fn default_water(self) -> u8 {
        match self {
            Material::DeepWater => 100,
            Material::ShallowWater => 75,
            Material::Steam => 25,
            _ => 0,
        }
    }

    pub fn flammability(self) -> Option<f32> {
        match self {
            Material::Gunpowder => Some(FLAMMABILITY_GUNPOWDER),
            Material::Oil => Some(FLAMMABILITY_OIL),
            Material::Grass => Some(FLAMMABILITY_GRASS),
            Material::Crop(_) => Some(FLAMMABILITY_CROP),
            Material::BerryBush => Some(FLAMMABILITY_BUSH),
            Material::Tree => Some(FLAMMABILITY_TREE),
            Material::WoodWall => Some(FLAMMABILITY_WOOD_WALL),
            Material::Cabin => Some(FLAMMABILITY_CABIN),
            _ => None,
        }
    }

    pub fn burn_duration(self) -> f32 {
        match self {
            Material::Oil => BURN_TIME_OIL,
            Material::Tree | Material::WoodWall | Material::Cabin => BURN_TIME_WOOD,
            Material::Grass | Material::Crop(_) | Material::BerryBush => BURN_TIME_GRASS,
            _ => BURN_TIME_DEFAULT,
        }
    }

    pub fn is_water(self) -> bool {
        matches!(self, Material::DeepWater | Material::ShallowWater)
    }

    pub fn is_water_passable(self) -> bool {
        matches!(
            self,
            Material::DeepWater | Material::ShallowWater | Material::Trench
        )
    }

    pub fn is_solid_obstacle(self) -> bool {
        matches!(
            self,
            Material::Tree
                | Material::Stone
                | Material::Magma
                | Material::WoodWall
                | Material::Cabin
                | Material::Chest
        )
    }

    pub fn can_deposit_debris(self) -> bool {
        matches!(
            self,
            Material::Dirt | Material::Sand | Material::Grass | Material::Ash | Material::Trench
        )
    }

    pub fn color(self) -> Color {
        match self {
            Material::DeepWater => Color::from_rgba(28, 76, 156, 255),
            Material::ShallowWater => Color::from_rgba(62, 148, 204, 255),
            Material::Sand => Color::from_rgba(222, 198, 134, 255),
            Material::Dirt => Color::from_rgba(130, 92, 62, 255),
            Material::Trench => Color::from_rgba(88, 60, 40, 255),
            Material::TilledSoil => Color::from_rgba(104, 68, 42, 255),
            Material::WetSoil => Color::from_rgba(68, 44, 28, 255),
            Material::Grass => Color::from_rgba(74, 152, 68, 255),
            Material::Tree => Color::from_rgba(40, 108, 44, 255),
            Material::BerryBush => Color::from_rgba(52, 128, 54, 255),
            Material::Crop(1) => Color::from_rgba(116, 188, 72, 255),
            Material::Crop(2) => Color::from_rgba(86, 174, 58, 255),
            Material::Crop(3) => Color::from_rgba(156, 194, 78, 255),
            Material::Crop(_) => Color::from_rgba(232, 192, 64, 255),
            Material::Stone => Color::from_rgba(132, 136, 144, 255),
            Material::Magma => Color::from_rgba(162, 66, 38, 255),
            Material::WoodWall => Color::from_rgba(148, 100, 56, 255),
            Material::Cabin => Color::from_rgba(176, 78, 58, 255),
            Material::Campfire => Color::from_rgba(240, 142, 46, 255),
            Material::Chest => Color::from_rgba(196, 144, 56, 255),
            Material::Ice => Color::from_rgba(178, 230, 248, 255),
            Material::Fire => Color::from_rgba(255, 118, 34, 255),
            Material::Smoke => Color::from_rgba(92, 92, 100, 255),
            Material::Steam => Color::from_rgba(220, 232, 242, 255),
            Material::Lava => Color::from_rgba(248, 92, 22, 255),
            Material::Acid => Color::from_rgba(128, 238, 62, 255),
            Material::Gunpowder => Color::from_rgba(56, 56, 62, 255),
            Material::Oil => Color::from_rgba(44, 38, 52, 255),
            Material::Ash => Color::from_rgba(110, 108, 114, 255),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Preset {
    LushRiverIsland,
    VolcanicCrater,
    DroughtValley,
}

impl Preset {
    pub const ALL: [Preset; 3] = [
        Preset::LushRiverIsland,
        Preset::VolcanicCrater,
        Preset::DroughtValley,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Preset::LushRiverIsland => "Lush River Island",
            Preset::VolcanicCrater => "Volcanic Crater Village",
            Preset::DroughtValley => "Drought Valley",
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Weather {
    Clear,
    Rainstorm,
    Heatwave,
}

impl Weather {
    pub const ALL: [Weather; 3] = [Weather::Clear, Weather::Rainstorm, Weather::Heatwave];

    pub fn name(self) -> &'static str {
        match self {
            Weather::Clear => "Clear",
            Weather::Rainstorm => "Rainstorm",
            Weather::Heatwave => "Heatwave",
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&w| w == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn ignition_multiplier(self) -> f32 {
        match self {
            Weather::Clear => 1.0,
            Weather::Rainstorm => RAIN_IGNITION_MULT,
            Weather::Heatwave => HEATWAVE_IGNITION_MULT,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Cell {
    pub material: Material,
    pub water_volume: u8,
    pub moisture: f32,
    pub temperature: f32,
    pub burn_timer: f32,
    pub structural_hp: u8,
    pub growth: f32,
    pub variant: u8,
    pub updated_tick: u32,
}

impl Cell {
    pub fn new(material: Material, variant: u8) -> Self {
        let normalized_mat = match material {
            Material::Crop(s) => Material::Crop(s.clamp(1, 4)),
            other => other,
        };
        let moisture = match normalized_mat {
            Material::DeepWater | Material::ShallowWater => 1.0,
            Material::WetSoil => 0.85,
            Material::Grass => 0.35,
            _ => 0.1,
        };
        let growth = match normalized_mat {
            Material::Crop(4) => 1.0,
            _ => 0.0,
        };
        let burn_timer = match normalized_mat {
            Material::Fire => BURN_TIME_DEFAULT,
            _ => 0.0,
        };
        let normalized_variant = match normalized_mat {
            Material::Campfire => variant | VARIANT_CAMPFIRE_LIT_BIT,
            Material::BerryBush => variant & !VARIANT_BUSH_EMPTY_BIT,
            _ => variant,
        };
        Self {
            material: normalized_mat,
            water_volume: normalized_mat.default_water(),
            moisture,
            temperature: normalized_mat.default_temp(),
            burn_timer,
            structural_hp: normalized_mat.default_hp(),
            growth,
            variant: normalized_variant,
            updated_tick: 0,
        }
    }

    fn has_flag(&self, bit: u8) -> bool {
        (self.variant & bit) != 0
    }

    fn set_flag(&mut self, bit: u8, on: bool) {
        if on {
            self.variant |= bit;
        } else {
            self.variant &= !bit;
        }
    }

    pub fn is_tilled(&self) -> bool {
        self.has_flag(VARIANT_TILLED_BIT)
    }

    pub fn set_tilled(&mut self, tilled: bool) {
        self.set_flag(VARIANT_TILLED_BIT, tilled);
    }

    pub fn is_wet_crop(&self) -> bool {
        self.has_flag(VARIANT_WET_CROP_BIT)
    }

    pub fn set_wet_crop(&mut self, wet: bool) {
        self.set_flag(VARIANT_WET_CROP_BIT, wet);
    }

    pub fn is_lit_campfire(&self) -> bool {
        self.has_flag(VARIANT_CAMPFIRE_LIT_BIT)
    }

    pub fn set_lit_campfire(&mut self, lit: bool) {
        self.set_flag(VARIANT_CAMPFIRE_LIT_BIT, lit);
    }

    pub fn has_berries(&self) -> bool {
        !self.has_flag(VARIANT_BUSH_EMPTY_BIT)
    }

    pub fn set_berries(&mut self, has_berries: bool) {
        self.set_flag(VARIANT_BUSH_EMPTY_BIT, !has_berries);
    }
}

#[derive(Clone, Debug)]
pub struct Debris {
    pub pos: Vec2,
    pub vel: Vec2,
    pub height: f32,
    pub v_height: f32,
    pub material: Material,
    pub color: Color,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GasKind {
    Smoke,
    Steam,
    AcidGas,
}

#[derive(Clone, Debug)]
pub struct GasParticle {
    pub pos: Vec2,
    pub height: f32,
    pub v_height: f32,
    pub temperature: f32,
    pub ttl: f32,
    pub water_volume: u8,
    pub kind: GasKind,
}

#[derive(Copy, Clone, Debug)]
pub struct PendingExplosion {
    pub x: i32,
    pub y: i32,
    pub radius: i32,
    pub power: f32,
}
