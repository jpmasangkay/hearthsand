use super::*;

#[test]
fn bounds_and_trench_water_flow() {
    let mut w = World::new(Preset::DroughtValley, 1);
    assert!(w.get(-1, 0).is_none());
    assert!(!w.set(GRID_W as i32, 0, Cell::new(Material::Dirt, 0)));

    for x in 20..=26 {
        w.set_material(x, 20, Material::Trench);
    }
    w.set_material(20, 20, Material::DeepWater);
    for _ in 0..120 {
        w.step();
    }
    let end = w.get(25, 20).expect("in bounds");
    assert_eq!(end.material, Material::Trench);
    assert!(end.water_volume > 10);
}

#[test]
fn soil_wicking_revert_and_crop_4x_growth() {
    let mut w = World::new(Preset::DroughtValley, 2);
    for dy in -4..=4 {
        for dx in -4..=4 {
            w.set_material(90 + dx, 30 + dy, Material::Dirt);
        }
    }
    w.set_material(90, 30, Material::DeepWater);
    w.set_material(92, 30, Material::TilledSoil);
    w.set_material(92, 31, Material::Crop(1));
    w.set_material(120, 30, Material::Crop(1));
    w.set_material(125, 30, Material::WetSoil);

    for _ in 0..120 {
        w.step();
    }
    assert_eq!(
        w.get(92, 30).expect("in bounds").material,
        Material::WetSoil
    );
    let wet_crop = w.get(92, 31).expect("in bounds");
    let dry_crop = w.get(120, 30).expect("in bounds");
    assert!(wet_crop.is_wet_crop());
    assert!(!dry_crop.is_wet_crop());
    assert!(wet_crop.growth > dry_crop.growth * 2.5);

    for _ in 0..600 {
        w.step();
    }
    assert_eq!(w.get(125, 30).expect("in bounds").material, Material::Dirt);
}

#[test]
fn explode_spawns_and_deposits_debris() {
    let mut w = World::new(Preset::LushRiverIsland, 3);
    w.set_material(80, 50, Material::Dirt);
    w.explode(80, 50, 3, 60.0);
    assert!(!w.debris.is_empty());
    for _ in 0..180 {
        w.step();
    }
    assert!(w.debris.is_empty());
}

#[test]
fn downwind_fire_spread() {
    let mut w = World::new(Preset::DroughtValley, 11);
    for y in 20..=36 {
        for x in 80..=120 {
            w.set_material(x, y, Material::Grass);
        }
    }
    for dy in -1..=1 {
        w.set_material(100, 28 + dy, Material::Fire);
    }
    for _ in 0..70 {
        w.wind = Vec2::new(26.0, 0.0);
        w.step();
    }

    let mut burnt_east = 0;
    let mut burnt_west = 0;
    for y in 22..=34 {
        for dx in 1..=18 {
            if let Some(c) = w.get(100 + dx, y) {
                if matches!(c.material, Material::Fire | Material::Ash | Material::Dirt) {
                    burnt_east += 1;
                }
            }
            if let Some(c) = w.get(100 - dx, y) {
                if matches!(c.material, Material::Fire | Material::Ash | Material::Dirt) {
                    burnt_west += 1;
                }
            }
        }
    }
    assert!(burnt_east > burnt_west);
}

#[test]
fn lava_plus_ice_and_stone() {
    let mut w = World::new(Preset::DroughtValley, 12);
    for dy in -2..=2 {
        for dx in -2..=2 {
            w.set_material(100 + dx, 40 + dy, Material::Dirt);
        }
    }
    w.set_material(99, 40, Material::Stone);
    w.set_material(100, 40, Material::Lava);
    w.set_material(101, 40, Material::Ice);

    for _ in 0..45 {
        w.step();
    }
    assert_eq!(w.get(99, 40).expect("in bounds").material, Material::Magma);
    assert_eq!(
        w.get(101, 40).expect("in bounds").material,
        Material::ShallowWater
    );
    assert!(w.gas.iter().any(|g| g.kind == GasKind::Steam));
}

#[test]
fn gunpowder_chain_capped_at_eight_per_tick() {
    let mut w = World::new(Preset::DroughtValley, 13);
    for x in 90..120 {
        w.set_material(x, 60, Material::Gunpowder);
    }
    w.ignite(90, 60, 500.0);
    assert_eq!(w.pending_explosions.len(), 1);

    w.step();
    assert_eq!(w.last_tick_explosions, 1);
    assert!(w.shake_impulse > 0.0);

    for _ in 0..20 {
        w.step();
        assert!(w.last_tick_explosions <= MAX_EXPLOSIONS_PER_TICK);
    }
    for _ in 0..180 {
        w.step();
        assert!(w.last_tick_explosions <= MAX_EXPLOSIONS_PER_TICK);
    }
    for x in 90..120 {
        assert_ne!(
            w.get(x, 60).expect("in bounds").material,
            Material::Gunpowder
        );
    }
}

#[test]
fn gunpowder_10x10_block_hits_cap_eight() {
    let mut w = World::new(Preset::DroughtValley, 18);
    for y in 40..50 {
        for x in 90..100 {
            w.set_material(x, y, Material::Gunpowder);
        }
    }
    w.ignite(90, 40, 500.0);

    let mut saw_eight = false;
    for _ in 0..240 {
        w.step();
        assert!(w.last_tick_explosions <= MAX_EXPLOSIONS_PER_TICK);
        if w.last_tick_explosions == MAX_EXPLOSIONS_PER_TICK {
            saw_eight = true;
        }
        if w.pending_explosions.is_empty() {
            break;
        }
    }

    assert!(saw_eight);
    assert!(w.pending_explosions.is_empty());
    for y in 40..50 {
        for x in 90..100 {
            assert_ne!(
                w.get(x, y).expect("in bounds").material,
                Material::Gunpowder
            );
        }
    }
}

#[test]
fn acid_dissolves_stone_and_emits_green_gas() {
    let mut w = World::new(Preset::DroughtValley, 14);
    w.set_material(95, 45, Material::Acid);
    w.set_material(96, 45, Material::Stone);
    w.set_material(95, 46, Material::WoodWall);
    w.set_material(94, 45, Material::Tree);

    for _ in 0..25 {
        w.step();
    }
    assert_ne!(w.get(96, 45).expect("in bounds").material, Material::Stone);
    assert_ne!(
        w.get(95, 46).expect("in bounds").material,
        Material::WoodWall
    );
    assert_ne!(w.get(94, 45).expect("in bounds").material, Material::Tree);
    assert!(w.gas.iter().any(|g| g.kind == GasKind::AcidGas));
}

#[test]
fn steam_rises_drifts_and_condenses() {
    let mut w = World::new(Preset::DroughtValley, 15);
    for y in 20..=35 {
        for x in 90..=115 {
            w.set_material(x, y, Material::Dirt);
        }
    }
    w.wind = Vec2::new(22.0, 0.0);
    w.spawn_gas(95, 28, GasKind::Steam, 115.0, 40);

    let start_x = w.gas[0].pos.x;
    let start_h = w.gas[0].height;
    for _ in 0..15 {
        w.wind = Vec2::new(22.0, 0.0);
        w.step();
    }
    assert!(w.gas[0].height > start_h);
    assert!(w.gas[0].pos.x > start_x);

    for _ in 0..300 {
        w.wind = Vec2::new(22.0, 0.0);
        w.step();
    }
    assert!(!w.gas.iter().any(|g| g.kind == GasKind::Steam));

    let mut condensed_water = 0_u64;
    for y in 20..=35 {
        for x in 95..=115 {
            condensed_water += w.get(x, y).expect("in bounds").water_volume as u64;
        }
    }
    assert_eq!(condensed_water, 40);
}

#[test]
fn condensation_skips_farm_prefers_water_and_soaks_small_puddles() {
    let mut w = World::new(Preset::DroughtValley, 19);
    // Farm patch at (98..=104, 28..=32) with TilledSoil, WetSoil, Crop, plus nearby Dirt & ShallowWater
    for y in 26..=34 {
        for x in 96..=108 {
            w.set_material(x, y, Material::Dirt);
        }
    }
    w.set_material(100, 30, Material::TilledSoil);
    w.set_material(101, 30, Material::WetSoil);
    w.set_material(102, 30, Material::Crop(2));
    w.set_material(103, 30, Material::ShallowWater); // vol = 75
    if let Some(c) = w.get_mut(103, 30) {
        c.water_volume = 50;
    }

    // Condense steam over the farm patch: prefers existing water cell (103, 30), never overwrites farm
    let base_total = w.total_water_units();
    w.deposit_water_conserving(100, 30, 20);
    assert_eq!(
        w.get(100, 30).expect("in bounds").material,
        Material::TilledSoil
    );
    assert_eq!(
        w.get(101, 30).expect("in bounds").material,
        Material::WetSoil
    );
    assert_eq!(
        w.get(102, 30).expect("in bounds").material,
        Material::Crop(2)
    );
    assert_eq!(w.get(103, 30).expect("in bounds").water_volume, 70);
    assert_eq!(w.total_water_units(), base_total + 20);

    // Even with no nearby water room, condensing onto farm tiles never overwrites them; uses Dirt instead
    if let Some(c) = w.get_mut(103, 30) {
        c.water_volume = 100;
    }
    let before_second = w.total_water_units();
    w.deposit_water_conserving(100, 30, 15);
    assert_eq!(
        w.get(100, 30).expect("in bounds").material,
        Material::TilledSoil
    );
    assert_eq!(
        w.get(101, 30).expect("in bounds").material,
        Material::WetSoil
    );
    assert_eq!(
        w.get(102, 30).expect("in bounds").material,
        Material::Crop(2)
    );
    assert_eq!(w.total_water_units(), before_second + 15);

    // ShallowWater puddle < 6 volume soaks into soil as moisture & adds to absorbed_water
    w.set_material(120, 30, Material::Dirt);
    if let Some(d) = w.get_mut(120, 30) {
        d.moisture = 0.05;
    }
    w.set_material(120, 30, Material::ShallowWater);
    if let Some(p) = w.get_mut(120, 30) {
        p.water_volume = 4;
        p.moisture = 0.1;
    }
    let before_soak = w.total_water_units();
    for _ in 0..5 {
        w.step();
    }
    let soaked = w.get(120, 30).expect("in bounds");
    assert_ne!(soaked.material, Material::ShallowWater);
    assert!(soaked.moisture > 0.4);
    assert!(w.absorbed_water >= 4);
    assert_eq!(w.total_water_units(), before_soak);
}

#[test]
fn water_conservation_and_weather_effects() {
    // Explicit allowed water sources in Hearthsand:
    // 1. Brush painting (set_material / add_water with water materials)
    // 2. Ice melting (each melted Ice cell produces ICE_MELT_WATER_VOL + ICE_STEAM_WATER_VOL = 75)
    // 3. Rain (Weather::Rainstorm adds rain water volume to water bodies)
    let mut w = World::new(Preset::DroughtValley, 16);
    for dy in -4..=4 {
        for dx in -4..=4 {
            w.set_material(100 + dx, 50 + dy, Material::Dirt);
        }
    }
    w.set_material(100, 50, Material::ShallowWater);
    w.set_material(101, 50, Material::ShallowWater);
    w.set_material(102, 50, Material::Lava);
    w.set_material(99, 50, Material::Fire);

    // Closed simulation (no rain, no ice melt, no brush painting): total_water_units is exact
    let initial_water = w.total_water_units();
    for _ in 0..420 {
        w.step();
    }
    assert_eq!(w.total_water_units(), initial_water);

    // Source 1: Brush painting increases total water by painted amount
    w.set_material(108, 50, Material::Dirt);
    w.add_water(108, 50, 30);
    let after_paint = w.total_water_units();
    assert_eq!(after_paint, initial_water + 30);

    // Source 2: Heatwave ice melt takes ~10s (600 ticks) and releases ice water units
    w.set_material(120, 50, Material::WetSoil);
    w.set_material(124, 50, Material::Ice);
    w.set_weather(Weather::Heatwave);
    for _ in 0..540 {
        // 9.0 seconds: ice still warming toward 0 C
        w.step();
    }
    assert_eq!(w.get(124, 50).expect("in bounds").material, Material::Ice);
    for _ in 0..120 {
        // 11.0 seconds total: melted to ShallowWater
        w.step();
    }
    assert_eq!(
        w.get(124, 50).expect("in bounds").material,
        Material::ShallowWater
    );
    let ice_water = (ICE_MELT_WATER_VOL + ICE_STEAM_WATER_VOL) as u64;
    assert_eq!(w.total_water_units(), after_paint + ice_water);

    // Heatwave doubles moisture decay compared to Clear weather
    let mut clear_w = World::new(Preset::DroughtValley, 17);
    let mut heat_w = World::new(Preset::DroughtValley, 17);
    clear_w.set_material(120, 50, Material::WetSoil);
    heat_w.set_material(120, 50, Material::WetSoil);
    heat_w.set_weather(Weather::Heatwave);
    for _ in 0..120 {
        clear_w.step();
        heat_w.step();
    }
    assert!(
        heat_w.get(120, 50).expect("in bounds").moisture
            < clear_w.get(120, 50).expect("in bounds").moisture
    );

    // Source 3: Rainstorm wets dry soil, extinguishes fire, and adds rain water
    w.set_weather(Weather::Rainstorm);
    w.set_material(115, 50, Material::TilledSoil);
    w.set_material(116, 50, Material::Fire);
    let before_rain = w.total_water_units();
    for _ in 0..180 {
        w.step();
    }
    assert_eq!(
        w.get(115, 50).expect("in bounds").material,
        Material::WetSoil
    );
    assert_ne!(w.get(116, 50).expect("in bounds").material, Material::Fire);
    assert!(w.total_water_units() > before_rain);

    // Weather wake pass only wakes fire/soil/crops/water chunks and lets settled/stone chunks sleep
    let mut sleep_w = World::new(Preset::DroughtValley, 20);
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            let mat = if x < 16 && y < 16 {
                Material::Dirt
            } else if (16..32).contains(&x) && y < 16 {
                Material::DeepWater
            } else {
                Material::Stone
            };
            sleep_w.set_material(x as i32, y as i32, mat);
        }
    }
    sleep_w.set_weather(Weather::Rainstorm);
    for _ in 0..240 {
        sleep_w.step();
    }
    // WetSoil & DeepWater(100) settle and go back to sleep along with Stone
    assert_eq!(sleep_w.active_chunk_count(), 0);
    sleep_w.set_weather(Weather::Heatwave);
    for _ in 0..420 {
        sleep_w.step();
    }
    // After soil warms & dries away from water, settled chunks sleep again
    let stone_chunk = sleep_w.chunk_awake[World::chunk_idx(4, 4)];
    assert_eq!(stone_chunk, 0);
    assert!(sleep_w.active_chunk_count() <= 4);
}

#[test]
fn spell_world_ops_and_preset_names() {
    assert_eq!(Preset::VolcanicCrater.name(), "Volcanic Crater Village");
    let mut w = World::new(Preset::VolcanicCrater, 25);
    let (cx, cy) = w.village_center();
    assert_eq!(
        w.get(cx + 9, cy).expect("in bounds").material,
        Material::DeepWater
    );

    // HydroCarve quenches fire into ash/dirt + steam and carves dry dirt into Trench + water
    w.set_material(cx + 15, cy, Material::Dirt);
    let water_before = w.total_water_units();
    w.hydro_carve(cx + 15, cy, 35, true);
    let carved = w.get(cx + 15, cy).expect("in bounds");
    assert_eq!(carved.material, Material::Trench);
    assert!(carved.water_volume >= 35);
    assert_eq!(w.total_water_units(), water_before + 35);

    // Overgrow converts dry Dirt/TilledSoil and advances Crop stages
    w.set_material(cx + 17, cy, Material::TilledSoil);
    w.set_material(cx + 18, cy, Material::Crop(1));
    w.overgrow(cx + 17, cy, 2, 2);
    assert!(matches!(
        w.get(cx + 17, cy).expect("in bounds").material,
        Material::Crop(_)
    ));
    assert_eq!(
        w.get(cx + 18, cy).expect("in bounds").material,
        Material::Crop(3)
    );

    // Gravity pull accelerates debris inward
    w.debris.push(Debris {
        pos: Vec2::new(80.0, 80.0),
        vel: Vec2::ZERO,
        height: 8.0,
        v_height: 10.0,
        material: Material::Dirt,
        color: Material::Dirt.color(),
    });
    w.pull_particles_toward(Vec2::new(110.0, 80.0), 64.0, 180.0);
    assert!(w.debris[0].vel.x > 0.0);
}

#[test]
fn fire_brush_ignites_gunpowder_relights_campfire_and_boils_water() {
    let mut w = World::new(Preset::DroughtValley, 26);
    let (cx, cy) = w.village_center();
    for dy in -4..=4 {
        for dx in -4..=4 {
            w.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    // Single Gunpowder tile + paint_fire triggers immediate queued explosion
    w.set_material(cx + 2, cy, Material::Gunpowder);
    w.paint_fire(cx + 2, cy, 520.0);
    assert_eq!(w.pending_explosions.len(), 1);
    assert_eq!(
        w.get(cx + 2, cy).expect("in bounds").material,
        Material::Ash
    );

    // Unlit Campfire + paint_fire relights Campfire without destroying it
    w.set_material(cx - 2, cy, Material::Campfire);
    if let Some(cf) = w.get_mut(cx - 2, cy) {
        cf.set_lit_campfire(false);
    }
    w.paint_fire(cx - 2, cy, 520.0);
    let cf = w.get(cx - 2, cy).expect("in bounds");
    assert_eq!(cf.material, Material::Campfire);
    assert!(cf.is_lit_campfire());

    // ShallowWater + paint_fire boils water into Steam and conserves total water units
    w.set_material(cx, cy + 2, Material::ShallowWater);
    let water_before = w.total_water_units();
    w.paint_fire(cx, cy + 2, 520.0);
    assert_eq!(w.total_water_units(), water_before);
    assert!(w.gas.iter().any(|g| g.kind == GasKind::Steam));
}

#[test]
fn magma_cools_to_stone_when_splashed_with_water() {
    let mut w = World::new(Preset::DroughtValley, 27);
    let (cx, cy) = w.village_center();
    for dy in -3..=3 {
        for dx in -3..=3 {
            w.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    w.set_material(cx, cy, Material::Magma);
    let water_before = w.total_water_units();
    w.hydro_carve(cx, cy, 20, false);
    w.hydro_carve(cx, cy, 20, false);
    assert_eq!(w.get(cx, cy).expect("in bounds").material, Material::Stone);
    assert_eq!(w.total_water_units(), water_before + 40);
    assert!(w.gas.iter().any(|g| g.kind == GasKind::Steam));
}
