use std::cmp::Ordering;

use macroquad::prelude::*;

use crate::world::{GasKind, Material, GRID_H, GRID_W, TILE_PX};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ThoughtIcon {
    Flame,
    Bucket,
    Berry,
    Hoe,
    Axe,
    Heart,
    Warning,
}

#[derive(Copy, Clone, Debug)]
pub enum WorldCmd {
    SetMaterial {
        x: i32,
        y: i32,
        material: Material,
    },
    AddWater {
        x: i32,
        y: i32,
        amount: u8,
    },
    Ignite {
        x: i32,
        y: i32,
        temp: f32,
    },
    Explode {
        x: i32,
        y: i32,
        radius: i32,
        power: f32,
    },
    ScoopWater {
        x: i32,
        y: i32,
        amount: u8,
    },
    SplashExtinguish {
        x: i32,
        y: i32,
        water: u8,
    },
    HarvestCrop {
        x: i32,
        y: i32,
    },
    HarvestBerry {
        x: i32,
        y: i32,
    },
    NibbleCrop {
        x: i32,
        y: i32,
    },
    VortexPull {
        x_px: f32,
        y_px: f32,
        radius_px: f32,
        strength: f32,
    },
    Overgrow {
        x: i32,
        y: i32,
        radius: i32,
        boost_stages: u8,
    },
    HydroCarve {
        x: i32,
        y: i32,
        amount: u8,
        carve_trench: bool,
    },
    HeatTile {
        x: i32,
        y: i32,
        temp: f32,
        damage: u8,
    },
    PaintFire {
        x: i32,
        y: i32,
        temp: f32,
    },
    SpawnGas {
        x: i32,
        y: i32,
        kind: GasKind,
        temp: f32,
        water: u8,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Payload {
    Fireball,
    HydroCanalJet,
    ChainLightning,
    GravityVortex,
    OvergrowthSeedBomb,
    TerraWall,
}

impl Payload {
    pub const ALL: [Payload; 6] = [
        Payload::Fireball,
        Payload::HydroCanalJet,
        Payload::ChainLightning,
        Payload::GravityVortex,
        Payload::OvergrowthSeedBomb,
        Payload::TerraWall,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Payload::Fireball => "Fireball",
            Payload::HydroCanalJet => "Hydro Canal Jet",
            Payload::ChainLightning => "Chain Lightning",
            Payload::GravityVortex => "Gravity Vortex",
            Payload::OvergrowthSeedBomb => "Overgrowth Seed",
            Payload::TerraWall => "Terra Wall",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Payload::Fireball => "Fireball",
            Payload::HydroCanalJet => "HydroJet",
            Payload::ChainLightning => "Lightning",
            Payload::GravityVortex => "Gravity",
            Payload::OvergrowthSeedBomb => "SeedBomb",
            Payload::TerraWall => "TerraWall",
        }
    }

    pub fn base_mana_cost(self) -> f32 {
        match self {
            Payload::Fireball => 12.0,
            Payload::HydroCanalJet => 10.0,
            Payload::ChainLightning => 16.0,
            Payload::GravityVortex => 20.0,
            Payload::OvergrowthSeedBomb => 14.0,
            Payload::TerraWall => 12.0,
        }
    }

    pub fn base_cooldown(self) -> f32 {
        match self {
            Payload::Fireball => 0.28,
            Payload::HydroCanalJet => 0.24,
            Payload::ChainLightning => 0.34,
            Payload::GravityVortex => 0.48,
            Payload::OvergrowthSeedBomb => 0.36,
            Payload::TerraWall => 0.32,
        }
    }

    pub fn color(self) -> Color {
        match self {
            Payload::Fireball => Color::from_rgba(255, 134, 42, 255),
            Payload::HydroCanalJet => Color::from_rgba(76, 188, 255, 255),
            Payload::ChainLightning => Color::from_rgba(164, 238, 255, 255),
            Payload::GravityVortex => Color::from_rgba(166, 92, 246, 255),
            Payload::OvergrowthSeedBomb => Color::from_rgba(110, 224, 92, 255),
            Payload::TerraWall => Color::from_rgba(186, 146, 98, 255),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Rune {
    MultiCast,
    Homing,
    Orbit,
    ExplosiveImpact,
    ElectricCharge,
}

impl Rune {
    pub const ALL: [Rune; 5] = [
        Rune::MultiCast,
        Rune::Homing,
        Rune::Orbit,
        Rune::ExplosiveImpact,
        Rune::ElectricCharge,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Rune::MultiCast => "MultiCast",
            Rune::Homing => "Homing",
            Rune::Orbit => "Orbit",
            Rune::ExplosiveImpact => "Explosive Impact",
            Rune::ElectricCharge => "Electric Charge",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Rune::MultiCast => "Multi3",
            Rune::Homing => "Homing",
            Rune::Orbit => "Orbit",
            Rune::ExplosiveImpact => "Explod",
            Rune::ElectricCharge => "Elec",
        }
    }

    pub fn mana_cost(self) -> f32 {
        match self {
            Rune::MultiCast => 6.0,
            Rune::Homing => 4.0,
            Rune::Orbit => 5.0,
            Rune::ExplosiveImpact => 8.0,
            Rune::ElectricCharge => 5.0,
        }
    }

    pub fn color(self) -> Color {
        match self {
            Rune::MultiCast => Color::from_rgba(255, 206, 86, 255),
            Rune::Homing => Color::from_rgba(246, 118, 178, 255),
            Rune::Orbit => Color::from_rgba(116, 212, 255, 255),
            Rune::ExplosiveImpact => Color::from_rgba(255, 112, 68, 255),
            Rune::ElectricCharge => Color::from_rgba(164, 246, 255, 255),
        }
    }
}

/// Wand Spell Combination Matrix (6 Payloads x 5 Runes):
/// Every slot stacks cleanly (multiples amplify projectile count / radius / power / duration / jumps).
///
/// | Payload \ Rune     | MultiCast (x3)          | Homing                    | Orbit                           | ExplosiveImpact            | ElectricCharge                     |
/// |--------------------|-------------------------|---------------------------|---------------------------------|----------------------------|------------------------------------|
/// | Fireball           | 3-way blazing spread    | Steers into wolves/oil    | Fiery orbital shields around Mage | Fireball crater blast      | Plasma heat (950C) + stun arcs     |
/// | HydroCanalJet      | 3-way canal fan         | Seeks fires/dry crops     | Swirling water moat & quench    | Steam-pressure hydro crater| Electrified water shock + stun     |
/// | ChainLightning     | 3-way branching arcs    | Extended lock-on chain    | Circling storm zappers          | Thunderclap blast at tip   | Overvoltage (+2 jumps, 5s stun)    |
/// | GravityVortex      | 3 orbiting singularities| Tracks critter/fire groups| Item & threat orbital attractor | Collapsing nova explosion  | Magneto-plasma continuous wolf zap |
/// | OvergrowthSeedBomb | 3-way forest carpet bomb| Seeks barren ash/dirt     | Blooming nature aura ring       | Shrapnel crater then bloom | Bio-static +2 crop stages & stun   |
/// | TerraWall          | 3 staggered stone wards | Intercepts nearest threat | Protective floating bastion arc | Seismic blast + bastion wall| Magnetized stone wall + stun arcs |
#[derive(Clone, Debug)]
pub struct Wand {
    pub payload: Payload,
    pub runes: [Option<Rune>; 3],
    pub mana: f32,
    pub max_mana: f32,
    pub mana_regen: f32,
    pub cooldown: f32,
}

impl Default for Wand {
    fn default() -> Self {
        Self::new()
    }
}

impl Wand {
    pub fn new() -> Self {
        Self {
            payload: Payload::Fireball,
            runes: [None; 3],
            mana: 100.0,
            max_mana: 100.0,
            mana_regen: 24.0,
            cooldown: 0.0,
        }
    }

    pub fn rune_count(&self, target: Rune) -> usize {
        self.runes.iter().filter(|&&r| r == Some(target)).count()
    }

    pub fn mana_cost(&self) -> f32 {
        let extra: f32 = self
            .runes
            .iter()
            .filter_map(|&r| r.map(Rune::mana_cost))
            .sum();
        (self.payload.base_mana_cost() + extra).min(self.max_mana)
    }

    pub fn cast_cooldown(&self) -> f32 {
        let extra = self.runes.iter().filter(|r| r.is_some()).count() as f32 * 0.03;
        self.payload.base_cooldown() + extra
    }

    pub fn slot_rune(&mut self, slot: usize, rune: Option<Rune>) {
        if slot < self.runes.len() {
            self.runes[slot] = rune;
        }
    }

    pub fn clear_runes(&mut self) {
        self.runes = [None; 3];
    }

    pub fn combo_hint(&self) -> &'static str {
        let primary = self.runes.iter().copied().flatten().next();
        match (self.payload, primary) {
            (Payload::Fireball, None) => "Ignites a 3x3 blaze on impact",
            (Payload::Fireball, Some(Rune::MultiCast)) => "3-way blazing spread volley",
            (Payload::Fireball, Some(Rune::Homing)) => "Steers toward wolves, oil & powder",
            (Payload::Fireball, Some(Rune::Orbit)) => "Fiery orbital shield igniting nearby",
            (Payload::Fireball, Some(Rune::ExplosiveImpact)) => {
                "Detonates an incendiary crater blast"
            }
            (Payload::Fireball, Some(Rune::ElectricCharge)) => {
                "Superheated plasma blast + wolf stun"
            }
            (Payload::HydroCanalJet, None) => "Carves watered irrigation trenches",
            (Payload::HydroCanalJet, Some(Rune::MultiCast)) => "3-way canal fan across dry soil",
            (Payload::HydroCanalJet, Some(Rune::Homing)) => "Seeks wildfires, lava & dry crops",
            (Payload::HydroCanalJet, Some(Rune::Orbit)) => "Swirling quenching water moat",
            (Payload::HydroCanalJet, Some(Rune::ExplosiveImpact)) => {
                "Pressurized hydro blast & trench"
            }
            (Payload::HydroCanalJet, Some(Rune::ElectricCharge)) => {
                "Electrified water jet stunning wolves"
            }
            (Payload::ChainLightning, None) => "Arcs across water, powder & wolves",
            (Payload::ChainLightning, Some(Rune::MultiCast)) => "3-way branching storm bolts",
            (Payload::ChainLightning, Some(Rune::Homing)) => "Extended lock-on chain jumps",
            (Payload::ChainLightning, Some(Rune::Orbit)) => "Circling storm orbs zapping threats",
            (Payload::ChainLightning, Some(Rune::ExplosiveImpact)) => {
                "Thunderclap explosion at arc tip"
            }
            (Payload::ChainLightning, Some(Rune::ElectricCharge)) => {
                "Overvoltage (+2 hops, longer stun)"
            }
            (Payload::GravityVortex, None) => "Pulls drops, critters & debris inward",
            (Payload::GravityVortex, Some(Rune::MultiCast)) => "3 clustered gravity singularities",
            (Payload::GravityVortex, Some(Rune::Homing)) => "Tracks wolves & wildfires",
            (Payload::GravityVortex, Some(Rune::Orbit)) => "Orbiting item & threat attractors",
            (Payload::GravityVortex, Some(Rune::ExplosiveImpact)) => "Collapses into a nova crater",
            (Payload::GravityVortex, Some(Rune::ElectricCharge)) => {
                "Magneto-plasma field zapping wolves"
            }
            (Payload::OvergrowthSeedBomb, None) => "Blooms grass, trees & grows crops",
            (Payload::OvergrowthSeedBomb, Some(Rune::MultiCast)) => {
                "3-way forest & crop carpet bloom"
            }
            (Payload::OvergrowthSeedBomb, Some(Rune::Homing)) => {
                "Seeks barren dirt, ash & tilled soil"
            }
            (Payload::OvergrowthSeedBomb, Some(Rune::Orbit)) => "Blooming nature aura around Mage",
            (Payload::OvergrowthSeedBomb, Some(Rune::ExplosiveImpact)) => {
                "Blasts terrain then blooms over crater"
            }
            (Payload::OvergrowthSeedBomb, Some(Rune::ElectricCharge)) => {
                "Bio-static surge (+2 stages & stun)"
            }
            (Payload::TerraWall, None) => "Raises a perpendicular stone barrier",
            (Payload::TerraWall, Some(Rune::MultiCast)) => "3 staggered defensive stone wards",
            (Payload::TerraWall, Some(Rune::Homing)) => "Intercepts nearest wolf or wildfire",
            (Payload::TerraWall, Some(Rune::Orbit)) => "Floating bastion stones around Mage",
            (Payload::TerraWall, Some(Rune::ExplosiveImpact)) => {
                "Seismic blast then raises stone wall"
            }
            (Payload::TerraWall, Some(Rune::ElectricCharge)) => {
                "Charged stone wall shocking wolves"
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Mage {
    pub pos: Vec2,
    pub vel: Vec2,
    pub aim_dir: Vec2,
    pub dir: u8,
    pub anim_time: f32,
    pub hp: f32,
    pub hit_flash: f32,
    pub dash_timer: f32,
    pub dash_cooldown: f32,
    pub dash_dir: Vec2,
    pub ghost_spawn_accum: f32,
    pub action_tween: f32,
    pub wand: Wand,
}

impl Default for Mage {
    fn default() -> Self {
        Self {
            pos: Vec2::new(86.0 * TILE_PX, 52.0 * TILE_PX),
            vel: Vec2::ZERO,
            aim_dir: Vec2::new(0.0, 1.0),
            dir: 0,
            anim_time: 0.0,
            hp: 100.0,
            hit_flash: 0.0,
            dash_timer: 0.0,
            dash_cooldown: 0.0,
            dash_dir: Vec2::new(0.0, 1.0),
            ghost_spawn_accum: 0.0,
            action_tween: 0.0,
            wand: Wand::new(),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct MageInput {
    pub move_axis: Vec2,
    pub aim_world: Vec2,
    pub dash_pressed: bool,
    pub cast_down: bool,
}

#[derive(Clone, Debug)]
pub struct MageGhost {
    pub pos: Vec2,
    pub dir: u8,
    pub alpha: f32,
    pub tint: Color,
}

#[derive(Copy, Clone, Debug)]
pub struct GlowLight {
    pub pos: Vec2,
    pub radius: f32,
    pub color: Color,
    pub bloom_alpha: f32,
}

#[derive(Copy, Clone, Debug)]
pub(super) struct RuneMods {
    pub(super) homing: usize,
    pub(super) explosive: usize,
    pub(super) electric: usize,
}

#[derive(Copy, Clone, Debug)]
pub(super) struct SpellImpact {
    pub(super) pos: Vec2,
    pub(super) vel: Vec2,
    pub(super) mods: RuneMods,
}

#[derive(Clone, Debug)]
pub(super) struct SpellBolt {
    pub(super) payload: Payload,
    pub(super) pos: Vec2,
    pub(super) vel: Vec2,
    pub(super) ttl: f32,
    pub(super) mods: RuneMods,
    pub(super) pulse_timer: f32,
}

#[derive(Clone, Debug)]
pub(super) struct OrbitShard {
    pub(super) payload: Payload,
    pub(super) angle: f32,
    pub(super) radius: f32,
    pub(super) ttl: f32,
    pub(super) mods: RuneMods,
    pub(super) pulse_timer: f32,
}

#[derive(Clone, Debug)]
pub(super) struct GravityField {
    pub(super) pos: Vec2,
    pub(super) radius: f32,
    pub(super) ttl: f32,
    pub(super) explosive: usize,
    pub(super) electric: usize,
    pub(super) zap_timer: f32,
}

#[derive(Clone, Debug)]
pub(super) struct LightningArc {
    pub(super) points: Vec<Vec2>,
    pub(super) ttl: f32,
    pub(super) max_ttl: f32,
    pub(super) color: Color,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CritterKind {
    Rabbit,
    Wolf,
}

impl CritterKind {
    pub fn name(self) -> &'static str {
        match self {
            CritterKind::Rabbit => "Rabbit",
            CritterKind::Wolf => "Wolf",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProjectileKind {
    Bomb,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobKind {
    Idle,
    FleeHazard,
    Extinguish,
    Eat,
    FarmIrrigate,
    HarvestHaul,
    LumberBuild,
    Warm,
}

impl JobKind {
    pub fn name(self) -> &'static str {
        match self {
            JobKind::Idle => "Idle",
            JobKind::FleeHazard => "Flee Hazard",
            JobKind::Extinguish => "Extinguish",
            JobKind::Eat => "Eat",
            JobKind::FarmIrrigate => "Farm/Irrigate",
            JobKind::HarvestHaul => "Harvest/Haul",
            JobKind::LumberBuild => "Lumber/Build",
            JobKind::Warm => "Warm",
        }
    }

    pub fn thought_icon(self) -> Option<ThoughtIcon> {
        match self {
            JobKind::Idle => None,
            JobKind::FleeHazard => Some(ThoughtIcon::Warning),
            JobKind::Extinguish => Some(ThoughtIcon::Bucket),
            JobKind::Eat => Some(ThoughtIcon::Berry),
            JobKind::FarmIrrigate => Some(ThoughtIcon::Hoe),
            JobKind::HarvestHaul => Some(ThoughtIcon::Berry),
            JobKind::LumberBuild => Some(ThoughtIcon::Axe),
            JobKind::Warm => Some(ThoughtIcon::Flame),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Wood,
    Crop,
    Berry,
}

impl ItemKind {
    pub fn name(self) -> &'static str {
        match self {
            ItemKind::Wood => "Wood",
            ItemKind::Crop => "Crop",
            ItemKind::Berry => "Berry",
        }
    }

    pub fn color(self) -> Color {
        match self {
            ItemKind::Wood => Color::from_rgba(166, 112, 62, 255),
            ItemKind::Crop => Color::from_rgba(246, 206, 72, 255),
            ItemKind::Berry => Color::from_rgba(226, 56, 78, 255),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ItemDrop {
    pub kind: ItemKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub height: f32,
    pub v_height: f32,
    pub age: f32,
}

#[derive(Clone, Debug)]
pub struct Villager {
    pub id: usize,
    pub pos: Vec2,
    pub vel: Vec2,
    pub dir: u8,
    pub anim_time: f32,
    pub decision_timer: f32,
    pub work_timer: f32,
    pub action_tween: f32,
    pub hit_flash: f32,
    pub hunger: f32,
    pub warmth: f32,
    pub hp: f32,
    pub job: JobKind,
    pub bucket_water: u8,
    pub carrying: Option<ItemKind>,
    pub thought: Option<ThoughtIcon>,
    pub thought_timer: f32,
    pub(crate) target_tile: Option<(i32, i32)>,
    pub(super) target_adjacent: bool,
    pub(super) path: Vec<(i32, i32)>,
    pub(super) path_idx: usize,
    pub(super) path_terrain_tick: u32,
    pub(super) needs_path: bool,
    pub(super) chop_hits: u8,
    pub(super) failed_targets: Vec<(JobKind, (i32, i32), f32)>,
    #[cfg(test)]
    pub(crate) assign_history: Vec<(f32, JobKind, (i32, i32), Vec2)>,
}

impl Villager {
    pub(super) fn is_blacklisted(&self, job: JobKind, tile: (i32, i32), sim_time: f32) -> bool {
        self.failed_targets
            .iter()
            .any(|&(j, t, exp)| j == job && t == tile && exp > sim_time)
    }

    pub(super) fn blacklist_target(&mut self, job: JobKind, tile: (i32, i32), expire_at: f32) {
        if let Some(entry) = self
            .failed_targets
            .iter_mut()
            .find(|(j, t, _)| *j == job && *t == tile)
        {
            entry.2 = entry.2.max(expire_at);
        } else {
            self.failed_targets.push((job, tile, expire_at));
        }
    }

    pub(super) fn prune_blacklist(&mut self, sim_time: f32) {
        self.failed_targets.retain(|&(_, _, exp)| exp > sim_time);
    }

    #[cfg(test)]
    pub(super) fn record_assignment(&mut self, sim_time: f32, tile: (i32, i32)) {
        self.assign_history
            .push((sim_time, self.job, tile, self.pos));
        let cutoff = sim_time - 30.0;
        self.assign_history.retain(|&(t, _, _, _)| t >= cutoff);
    }

    #[cfg(test)]
    pub fn worst_stall_reassignments(&self, sim_time: f32) -> (usize, JobKind, (i32, i32), f32) {
        let cutoff = sim_time - 30.0;
        let mut best: (usize, JobKind, (i32, i32), f32) = (0, JobKind::Idle, (0, 0), 0.0);
        for &(t0, job, tile, _) in &self.assign_history {
            if t0 < cutoff {
                continue;
            }
            let mut count = 0_usize;
            let mut t_first = f32::INFINITY;
            let mut t_last = f32::NEG_INFINITY;
            for &(t1, j1, tile1, _) in &self.assign_history {
                if t1 >= cutoff && j1 == job && tile1 == tile {
                    count += 1;
                    t_first = t_first.min(t1);
                    t_last = t_last.max(t1);
                }
            }
            if count <= best.0 {
                continue;
            }
            // True spatial diameter across all positions in [t_first, t_last]
            let mut max_disp = 0.0_f32;
            for &(ta, _, _, pa) in &self.assign_history {
                if ta < t_first || ta > t_last {
                    continue;
                }
                for &(tb, _, _, pb) in &self.assign_history {
                    if tb >= t_first && tb <= t_last {
                        max_disp = max_disp.max(pa.distance(pb));
                    }
                }
            }
            if max_disp < 2.0 * TILE_PX {
                best = (count, job, tile, max_disp);
            }
        }
        best
    }
}

#[derive(Clone, Debug)]
pub struct Critter {
    pub id: usize,
    pub kind: CritterKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub dir: u8,
    pub anim_time: f32,
    pub hop_timer: f32,
    pub hop_height: f32,
    pub hop_v: f32,
    pub action_tween: f32,
    pub stun_timer: f32,
    pub nibble_cooldown: f32,
}

#[derive(Clone, Debug)]
pub struct Projectile {
    pub id: usize,
    pub kind: ProjectileKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub ttl: f32,
    pub radius: i32,
    pub power: f32,
}

pub(super) struct AstarScratch {
    pub(super) g_score: Vec<u32>,
    pub(super) came_from: Vec<i32>,
    pub(super) closed: Vec<u32>,
    pub(super) stamp: u32,
}

impl AstarScratch {
    pub(super) fn new() -> Self {
        let n = GRID_W * GRID_H;
        Self {
            g_score: vec![u32::MAX; n],
            came_from: vec![-1; n],
            closed: vec![0; n],
            stamp: 1,
        }
    }

    pub(super) fn next_stamp(&mut self) -> u32 {
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.closed.fill(0);
            self.stamp = 1;
        }
        self.stamp
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub(super) struct OpenNode {
    pub(super) f: u32,
    pub(super) g: u32,
    pub(super) idx: usize,
}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.cmp(&self.f).then_with(|| other.g.cmp(&self.g))
    }
}

impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub(super) struct VillageScan {
    pub(super) center: (i32, i32),
    pub(super) hazards: Vec<(i32, i32)>,
    pub(super) village_fires: Vec<(i32, i32)>,
    pub(super) water_edges: Vec<(i32, i32)>,
    pub(super) lit_campfires: Vec<(i32, i32)>,
    pub(super) unlit_campfires: Vec<(i32, i32)>,
    pub(super) chests: Vec<(i32, i32)>,
    pub(super) cabins: Vec<(i32, i32)>,
    pub(super) ripe_crops: Vec<(i32, i32)>,
    pub(super) all_crops: Vec<(i32, i32)>,
    pub(super) ripe_bushes: Vec<(i32, i32)>,
    pub(super) dry_tilled: Vec<(i32, i32)>,
    pub(super) plantable_soil: Vec<(i32, i32, bool)>,
    pub(super) farm_tile_count: usize,
    pub(super) trees: Vec<(i32, i32)>,
    pub(super) reach_comp: Vec<u16>,
}

#[derive(Copy, Clone)]
pub(super) struct ColonyState {
    pub(super) wood_stored: u32,
    pub(super) food_stored: u32,
    pub(super) villager_count: usize,
    pub(super) total_crops_harvested: u32,
    pub(super) sim_time: f32,
}
