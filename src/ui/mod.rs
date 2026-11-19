use macroquad::prelude::*;

use crate::agents::{Payload, WorldCmd};
use crate::world::{GasKind, Material, Preset, World};

mod hud;
mod layout;
mod theme;

#[cfg(test)]
mod tests;

pub use hud::*;
pub use theme::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SpeedMode {
    Pause,
    Normal,
    Fast,
    Turbo,
}

impl SpeedMode {
    pub fn multiplier(self) -> usize {
        match self {
            SpeedMode::Pause => 0,
            SpeedMode::Normal => 1,
            SpeedMode::Fast => 3,
            SpeedMode::Turbo => 5,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SpeedMode::Pause => "Pause",
            SpeedMode::Normal => "1x",
            SpeedMode::Fast => "3x",
            SpeedMode::Turbo => "5x",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BrushTool {
    Element(Material),
    AddWater,
    SpawnVillager,
    SpawnRabbit,
    SpawnWolf,
    ThrowBomb,
}

impl BrushTool {
    pub fn button_label(self) -> &'static str {
        match self {
            BrushTool::Element(Material::Crop(_)) => "Crop",
            BrushTool::Element(m) => m.name(),
            BrushTool::AddWater => "+Water",
            BrushTool::SpawnVillager => "Villager",
            BrushTool::SpawnRabbit => "Rabbit",
            BrushTool::SpawnWolf => "Wolf",
            BrushTool::ThrowBomb => "Bomb",
        }
    }

    pub fn swatch_color(self) -> Color {
        match self {
            BrushTool::Element(m) => m.color(),
            BrushTool::AddWater => Color::from_rgba(72, 168, 244, 255),
            BrushTool::SpawnVillager => Color::from_rgba(124, 178, 224, 255),
            BrushTool::SpawnRabbit => Color::from_rgba(226, 216, 198, 255),
            BrushTool::SpawnWolf => Color::from_rgba(116, 122, 138, 255),
            BrushTool::ThrowBomb => Color::from_rgba(232, 114, 52, 255),
        }
    }

    pub fn is_single_point(self) -> bool {
        matches!(
            self,
            BrushTool::SpawnVillager
                | BrushTool::SpawnRabbit
                | BrushTool::SpawnWolf
                | BrushTool::ThrowBomb
        )
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PaletteCategory {
    Terrain,
    Liquids,
    FireHeat,
    Life,
    Build,
    Spells,
    Tools,
}

impl PaletteCategory {
    pub const ALL: [PaletteCategory; 7] = [
        PaletteCategory::Terrain,
        PaletteCategory::Liquids,
        PaletteCategory::FireHeat,
        PaletteCategory::Life,
        PaletteCategory::Build,
        PaletteCategory::Spells,
        PaletteCategory::Tools,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PaletteCategory::Terrain => "Terrain",
            PaletteCategory::Liquids => "Liquids",
            PaletteCategory::FireHeat => "Fire & Heat",
            PaletteCategory::Life => "Life",
            PaletteCategory::Build => "Build",
            PaletteCategory::Spells => "Spells",
            PaletteCategory::Tools => "Tools",
        }
    }

    pub fn entries(self) -> &'static [PaletteEntry] {
        match self {
            PaletteCategory::Terrain => &TERRAIN_ENTRIES,
            PaletteCategory::Liquids => &LIQUID_ENTRIES,
            PaletteCategory::FireHeat => &FIRE_ENTRIES,
            PaletteCategory::Life => &LIFE_ENTRIES,
            PaletteCategory::Build => &BUILD_ENTRIES,
            PaletteCategory::Spells => &SPELL_ENTRIES,
            PaletteCategory::Tools => &TOOL_ENTRIES,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PaletteEntry {
    Tool(BrushTool),
    Spell(Payload),
}

impl PaletteEntry {
    pub fn label(self) -> &'static str {
        match self {
            PaletteEntry::Tool(t) => t.button_label(),
            PaletteEntry::Spell(p) => p.short_name(),
        }
    }

    pub fn fill_color(self) -> Color {
        match self {
            PaletteEntry::Tool(t) => t.swatch_color(),
            PaletteEntry::Spell(p) => p.color(),
        }
    }
}

const TERRAIN_ENTRIES: [PaletteEntry; 8] = [
    PaletteEntry::Tool(BrushTool::Element(Material::Grass)),
    PaletteEntry::Tool(BrushTool::Element(Material::Dirt)),
    PaletteEntry::Tool(BrushTool::Element(Material::Sand)),
    PaletteEntry::Tool(BrushTool::Element(Material::Stone)),
    PaletteEntry::Tool(BrushTool::Element(Material::TilledSoil)),
    PaletteEntry::Tool(BrushTool::Element(Material::WetSoil)),
    PaletteEntry::Tool(BrushTool::Element(Material::Ash)),
    PaletteEntry::Tool(BrushTool::Element(Material::Trench)),
];

const LIQUID_ENTRIES: [PaletteEntry; 6] = [
    PaletteEntry::Tool(BrushTool::Element(Material::ShallowWater)),
    PaletteEntry::Tool(BrushTool::Element(Material::DeepWater)),
    PaletteEntry::Tool(BrushTool::AddWater),
    PaletteEntry::Tool(BrushTool::Element(Material::Oil)),
    PaletteEntry::Tool(BrushTool::Element(Material::Acid)),
    PaletteEntry::Tool(BrushTool::Element(Material::Ice)),
];

const FIRE_ENTRIES: [PaletteEntry; 6] = [
    PaletteEntry::Tool(BrushTool::Element(Material::Fire)),
    PaletteEntry::Tool(BrushTool::Element(Material::Lava)),
    PaletteEntry::Tool(BrushTool::Element(Material::Magma)),
    PaletteEntry::Tool(BrushTool::Element(Material::Gunpowder)),
    PaletteEntry::Tool(BrushTool::Element(Material::Smoke)),
    PaletteEntry::Tool(BrushTool::Element(Material::Steam)),
];

const LIFE_ENTRIES: [PaletteEntry; 6] = [
    PaletteEntry::Tool(BrushTool::Element(Material::Tree)),
    PaletteEntry::Tool(BrushTool::Element(Material::BerryBush)),
    PaletteEntry::Tool(BrushTool::Element(Material::Crop(1))),
    PaletteEntry::Tool(BrushTool::SpawnVillager),
    PaletteEntry::Tool(BrushTool::SpawnRabbit),
    PaletteEntry::Tool(BrushTool::SpawnWolf),
];

const BUILD_ENTRIES: [PaletteEntry; 4] = [
    PaletteEntry::Tool(BrushTool::Element(Material::WoodWall)),
    PaletteEntry::Tool(BrushTool::Element(Material::Cabin)),
    PaletteEntry::Tool(BrushTool::Element(Material::Campfire)),
    PaletteEntry::Tool(BrushTool::Element(Material::Chest)),
];

const SPELL_ENTRIES: [PaletteEntry; 6] = [
    PaletteEntry::Spell(Payload::Fireball),
    PaletteEntry::Spell(Payload::HydroCanalJet),
    PaletteEntry::Spell(Payload::ChainLightning),
    PaletteEntry::Spell(Payload::GravityVortex),
    PaletteEntry::Spell(Payload::OvergrowthSeedBomb),
    PaletteEntry::Spell(Payload::TerraWall),
];

const TOOL_ENTRIES: [PaletteEntry; 6] = [
    PaletteEntry::Tool(BrushTool::Element(Material::Trench)),
    PaletteEntry::Tool(BrushTool::AddWater),
    PaletteEntry::Tool(BrushTool::ThrowBomb),
    PaletteEntry::Tool(BrushTool::SpawnVillager),
    PaletteEntry::Tool(BrushTool::SpawnRabbit),
    PaletteEntry::Tool(BrushTool::SpawnWolf),
];

#[derive(Clone, Debug)]
pub struct UiState {
    pub menu_open: bool,
    pub stats_open: bool,
    pub controls_open: bool,
    pub wand_open: bool,
    pub snap_zoom: bool,
    pub skip_lighting: bool,
    pub skip_bloom: bool,
    pub direct_scene: bool,
    pub smoothed_fps: f32,
    pub active_tab: PaletteCategory,
    pub selected_tool: BrushTool,
    pub brush_radius: i32,
    pub selected_rune_slot: usize,
    pub last_run_speed: SpeedMode,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            menu_open: false,
            stats_open: false,
            controls_open: false,
            wand_open: false,
            snap_zoom: true,
            skip_lighting: false,
            skip_bloom: false,
            direct_scene: true,
            smoothed_fps: 60.0,
            active_tab: PaletteCategory::Terrain,
            selected_tool: BrushTool::Element(Material::Trench),
            brush_radius: 1,
            selected_rune_slot: 0,
            last_run_speed: SpeedMode::Normal,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UiAction {
    SelectPreset(Preset),
    CycleWeather,
    AdvanceTime,
    Reset,
    ToggleMode,
    ToggleWandBuilder,
    ToggleSnapZoom,
    SelectSpell(Payload),
}

pub fn paint_circle_cmds(cx: i32, cy: i32, radius: i32, mat: Material) -> Vec<WorldCmd> {
    let mut cmds = Vec::new();
    let r = radius - 1;
    let r_sq = r * r;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r_sq {
                let x = cx + dx;
                let y = cy + dy;
                match mat {
                    Material::Fire => {
                        cmds.push(WorldCmd::PaintFire { x, y, temp: 520.0 });
                    }
                    Material::Smoke => {
                        cmds.push(WorldCmd::SpawnGas {
                            x,
                            y,
                            kind: GasKind::Smoke,
                            temp: 85.0,
                            water: 0,
                        });
                    }
                    Material::Steam => {
                        cmds.push(WorldCmd::SpawnGas {
                            x,
                            y,
                            kind: GasKind::Steam,
                            temp: 115.0,
                            water: Material::Steam.default_water(),
                        });
                    }
                    _ => {
                        cmds.push(WorldCmd::SetMaterial {
                            x,
                            y,
                            material: mat,
                        });
                    }
                }
            }
        }
    }
    cmds
}

pub fn apply_world_cmds(world: &mut World, cmds: Vec<WorldCmd>) {
    for cmd in cmds {
        match cmd {
            WorldCmd::SetMaterial { x, y, material } => {
                world.set_material(x, y, material);
            }
            WorldCmd::AddWater { x, y, amount } => {
                world.add_water(x, y, amount);
            }
            WorldCmd::Ignite { x, y, temp } => {
                world.ignite(x, y, temp);
            }
            WorldCmd::Explode {
                x,
                y,
                radius,
                power,
            } => {
                world.explode(x, y, radius, power);
            }
            WorldCmd::ScoopWater { x, y, amount } => {
                world.scoop_water(x, y, amount);
            }
            WorldCmd::SplashExtinguish { x, y, water } => {
                world.splash_extinguish(x, y, water);
            }
            WorldCmd::HarvestCrop { x, y } => {
                world.harvest_crop(x, y);
            }
            WorldCmd::HarvestBerry { x, y } => {
                world.harvest_berry(x, y);
            }
            WorldCmd::NibbleCrop { x, y } => {
                world.nibble_crop(x, y);
            }
            WorldCmd::VortexPull {
                x_px,
                y_px,
                radius_px,
                strength,
            } => {
                world.pull_particles_toward(Vec2::new(x_px, y_px), radius_px, strength);
            }
            WorldCmd::Overgrow {
                x,
                y,
                radius,
                boost_stages,
            } => {
                world.overgrow(x, y, radius, boost_stages);
            }
            WorldCmd::HydroCarve {
                x,
                y,
                amount,
                carve_trench,
            } => {
                world.hydro_carve(x, y, amount, carve_trench);
            }
            WorldCmd::HeatTile { x, y, temp, damage } => {
                world.heat_tile(x, y, temp, damage);
            }
            WorldCmd::PaintFire { x, y, temp } => {
                world.paint_fire(x, y, temp);
            }
            WorldCmd::SpawnGas {
                x,
                y,
                kind,
                temp,
                water,
            } => {
                world.spawn_gas(x, y, kind, temp, water);
            }
        }
    }
}

pub fn wind_compass_label(wind: Vec2) -> String {
    let spd = wind.length();
    let deg = wind.y.atan2(wind.x).to_degrees();
    let dir = if (-22.5..22.5).contains(&deg) {
        "E"
    } else if (22.5..67.5).contains(&deg) {
        "SE"
    } else if (67.5..112.5).contains(&deg) {
        "S"
    } else if (112.5..157.5).contains(&deg) {
        "SW"
    } else if (-67.5..-22.5).contains(&deg) {
        "NE"
    } else if (-112.5..-67.5).contains(&deg) {
        "N"
    } else if (-157.5..-112.5).contains(&deg) {
        "NW"
    } else {
        "W"
    };
    format!("{} {:.0}", dir, spd)
}
