use super::utility_ai::{
    is_job_target_still_valid, pick_best_job, score_eat, score_extinguish, score_warm,
};
use super::*;
use crate::world::{Material, Preset, SIM_DT};

fn apply_cmds(world: &mut World, cmds: Vec<WorldCmd>) {
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

#[test]
fn utility_ai_hysteresis_and_flee_override() {
    let mut world = World::new(Preset::DroughtValley, 101);
    let (cx, cy) = world.village_center();
    for dy in -8..=8 {
        for dx in -8..=8 {
            if dx != 0 || dy != 0 {
                world.set_material(cx + dx, cy + dy, Material::Dirt);
            }
        }
    }
    world.set_material(cx + 4, cy, Material::Chest);
    world.set_material(cx - 4, cy, Material::Tree);
    let scan = VillageScan::scan(&world);

    let mut agents = Agents::new(101);
    let vid = agents.spawn_villager(tile_center(cx + 1, cy + 1));
    assert_eq!(vid, 1);

    // Current job LumberBuild at raw 0.50 gets 1.20x => 0.60, beating FarmIrrigate at 0.54
    {
        let v = &mut agents.villagers[0];
        v.job = JobKind::LumberBuild;
        v.hunger = 0.85;
        v.warmth = 0.90;
    }
    let colony = ColonyState {
        wood_stored: 4,
        food_stored: 10,
        villager_count: 1,
        total_crops_harvested: 0,
        sim_time: 0.0,
    };
    let chosen = pick_best_job(&world, &scan, &agents.villagers[0], &[], colony, &[]);
    assert_eq!(chosen, JobKind::LumberBuild);

    // Place Fire within 3 tiles: FleeHazard must override immediately
    world.set_material(cx + 2, cy + 1, Material::Fire);
    let scan_fire = VillageScan::scan(&world);
    let chosen_flee = pick_best_job(&world, &scan_fire, &agents.villagers[0], &[], colony, &[]);
    assert_eq!(chosen_flee, JobKind::FleeHazard);
}

#[test]
fn astar_invalidates_on_terrain_change_and_caps_at_eight() {
    let mut world = World::new(Preset::DroughtValley, 102);
    let (cx, cy) = world.village_center();
    for dy in -10..=10 {
        for dx in -10..=10 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    world.set_material(cx + 6, cy, Material::Chest);
    world.set_material(cx - 6, cy, Material::Tree);

    let mut agents = Agents::new(102);
    for i in 0..20 {
        let y = cy - 4 + (i % 8);
        let x = cx - 4 + (i / 8);
        agents.spawn_villager(tile_center(x, y));
        let v = &mut agents.villagers[i as usize];
        v.job = JobKind::Idle;
        v.target_tile = Some((cx + 5, cy));
        v.target_adjacent = false;
        v.needs_path = true;
        v.path_terrain_tick = world.last_terrain_change_tick;
    }

    // First frame caps A* queue strictly at MAX_ASTAR_PER_FRAME (8)
    let _ = agents.update(&world, SIM_DT);
    assert_eq!(agents.last_frame_astar_count, MAX_ASTAR_PER_FRAME);

    // Trigger terrain change and verify all existing paths invalidate cleanly
    world.step();
    world.set_material(cx + 2, cy, Material::Stone);
    for v in &mut agents.villagers {
        v.target_tile = Some((cx + 5, cy));
        v.path.push((cx + 2, cy));
    }
    let _ = agents.update(&world, SIM_DT);
    assert!(agents.last_frame_astar_count <= MAX_ASTAR_PER_FRAME);

    // Any villager that was not yet recomputed had its stale path cleared on terrain change
    let stale_remaining = agents
        .villagers
        .iter()
        .filter(|v| v.path.contains(&(cx + 2, cy)))
        .count();
    assert_eq!(stale_remaining, 0);
}

#[test]
fn villager_extinguish_fire_with_bucket() {
    let mut world = World::new(Preset::DroughtValley, 103);
    let (cx, cy) = world.village_center();
    for dy in -6..=6 {
        for dx in -6..=6 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    world.set_material(cx - 2, cy, Material::ShallowWater);
    world.set_material(cx + 3, cy, Material::Fire);
    if let Some(fire) = world.get_mut(cx + 3, cy) {
        fire.burn_timer = 15.0;
    }

    let mut agents = Agents::new(103);
    agents.spawn_villager(tile_center(cx, cy));

    let mut extinguished = false;
    for _ in 0..420 {
        world.step();
        let cmds = agents.update(&world, SIM_DT);
        apply_cmds(&mut world, cmds);
        if world.get(cx + 3, cy).expect("in bounds").material != Material::Fire {
            extinguished = true;
            break;
        }
    }
    assert!(extinguished);
}

#[test]
fn villager_digs_trench_to_dry_tilled_soil() {
    let mut world = World::new(Preset::DroughtValley, 104);
    let (cx, cy) = world.village_center();
    for dy in -6..=6 {
        for dx in -10..=10 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    world.set_material(cx - 5, cy, Material::DeepWater);
    world.set_material(cx - 6, cy, Material::DeepWater);
    world.set_material(cx + 1, cy, Material::TilledSoil);

    let mut agents = Agents::new(104);
    agents.spawn_villager(tile_center(cx - 2, cy + 1));

    let mut dug_trench = false;
    for _ in 0..600 {
        world.step();
        let cmds = agents.update(&world, SIM_DT);
        apply_cmds(&mut world, cmds);
        if (-4..=0)
            .any(|dx| world.get(cx + dx, cy).expect("in bounds").material == Material::Trench)
        {
            dug_trench = true;
            if world.get(cx + 1, cy).expect("in bounds").material == Material::WetSoil {
                break;
            }
        }
    }
    assert!(dug_trench);
}

#[test]
fn villager_harvests_and_hauls_to_chest() {
    let mut world = World::new(Preset::DroughtValley, 105);
    let (cx, cy) = world.village_center();
    for dy in -5..=5 {
        for dx in -5..=5 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    world.set_material(cx + 2, cy, Material::Chest);
    world.set_material(cx - 1, cy, Material::Crop(4));

    let mut agents = Agents::new(105);
    let vid = agents.spawn_villager(tile_center(cx, cy));
    agents.villagers[0].hunger = 0.95;
    let _ = vid;

    for _ in 0..480 {
        world.step();
        let cmds = agents.update(&world, SIM_DT);
        apply_cmds(&mut world, cmds);
        if agents.total_crops_harvested >= 1 {
            break;
        }
    }
    assert!(agents.total_crops_harvested >= 1);
    assert_ne!(
        world.get(cx - 1, cy).expect("in bounds").material,
        Material::Crop(4)
    );
}

#[test]
fn rabbit_nibbles_crops_and_wolf_night_behavior() {
    let mut world = World::new(Preset::DroughtValley, 106);
    let (cx, cy) = world.village_center();
    for dy in -8..=8 {
        for dx in -8..=8 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    world.set_material(cx, cy, Material::Campfire);
    world.set_material(cx + 5, cy + 5, Material::Crop(3));

    let mut agents = Agents::new(106);
    agents.spawn_critter(CritterKind::Rabbit, tile_center(cx + 3, cy + 5));
    agents.spawn_critter(CritterKind::Wolf, tile_center(cx + 2, cy));

    // Night wolf near lit Campfire flees outward past campfire radius; lightning stuns wolf
    world.time_of_day = 0.85;
    let start_dist = agents.critters[1].pos.distance(tile_center(cx, cy));
    for _ in 0..120 {
        world.step();
        let cmds = agents.update(&world, SIM_DT);
        apply_cmds(&mut world, cmds);
    }
    let end_dist = agents.critters[1].pos.distance(tile_center(cx, cy));
    assert!(end_dist > start_dist);
    assert_ne!(
        world.get(cx + 5, cy + 5).expect("in bounds").material,
        Material::Crop(3)
    );

    let wolf_pos = agents.critters[1].pos;
    agents.stun_wolves_near(wolf_pos, 32.0, 2.0);
    assert!(agents.critters[1].stun_timer > 1.5);
    let frozen_pos = agents.critters[1].pos;
    let _ = agents.update(&world, SIM_DT);
    assert_eq!(agents.critters[1].pos, frozen_pos);
}

#[test]
fn mage_dash_ghosts_wand_slots_and_all_rune_payload_combos() {
    let mut world = World::new(Preset::DroughtValley, 107);
    let (cx, cy) = world.village_center();
    for dy in -12..=12 {
        for dx in -12..=12 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }

    let mut agents = Agents::new(107);
    agents.populate_for_world(&world);
    assert!(!agents.mage_active);
    assert!(agents.toggle_mage_mode());
    assert!(agents.mage_pos.is_some());

    // Dash spawns ghost afterimages and moves Mage forward
    let start_pos = agents.mage.pos;
    let _ = agents.drive_mage(
        &world,
        MageInput {
            move_axis: Vec2::new(1.0, 0.0),
            aim_world: start_pos + Vec2::new(48.0, 0.0),
            dash_pressed: true,
            cast_down: false,
        },
        0.05,
    );
    assert!(!agents.mage_ghosts.is_empty());
    assert!(agents.mage.pos.x > start_pos.x);

    // Slot runes and clear with right-click equivalent
    agents.mage.wand.slot_rune(0, Some(Rune::MultiCast));
    assert_eq!(agents.mage.wand.runes[0], Some(Rune::MultiCast));
    agents.mage.wand.slot_rune(0, None);
    assert_eq!(agents.mage.wand.runes[0], None);

    // Exercise every 6x5 Payload x Rune combination
    for &payload in &Payload::ALL {
        for &rune in &Rune::ALL {
            let mut w = World::new(Preset::DroughtValley, 108);
            for dy in -12..=12 {
                for dx in -12..=12 {
                    w.set_material(cx + dx, cy + dy, Material::Dirt);
                }
            }
            w.set_material(cx + 4, cy, Material::Gunpowder);
            w.set_material(cx + 3, cy + 1, Material::TilledSoil);

            let mut ag = Agents::new(108);
            ag.mage.pos = tile_center(cx, cy);
            ag.mage_active = true;
            ag.mage_pos = Some(ag.mage.pos);
            let wid = ag.spawn_critter(CritterKind::Wolf, tile_center(cx + 3, cy));
            let _ = wid;

            ag.mage.wand.payload = payload;
            ag.mage.wand.runes = [Some(rune), None, None];
            ag.mage.wand.mana = 100.0;
            ag.mage.wand.cooldown = 0.0;

            let cast_cmds = ag.cast_wand(&w, tile_center(cx + 3, cy));
            apply_cmds(&mut w, cast_cmds);
            assert!(ag.mage.wand.mana < 100.0);

            for _ in 0..95 {
                w.step();
                let cmds = ag.update(&w, SIM_DT);
                apply_cmds(&mut w, cmds);
            }

            if payload == Payload::ChainLightning || rune == Rune::ElectricCharge {
                let wolf = ag
                    .critters
                    .iter()
                    .find(|c| c.kind == CritterKind::Wolf)
                    .expect("wolf present");
                assert!(wolf.stun_timer > 0.0 || !ag.lightning_arcs.is_empty());
            }
        }
    }

    // Direct TerraWall perpendicular stone wall check
    world.set_material(cx + 4, cy, Material::Dirt);
    agents.mage.wand.payload = Payload::TerraWall;
    agents.mage.wand.runes = [None; 3];
    agents.mage.wand.mana = 100.0;
    agents.mage.wand.cooldown = 0.0;
    agents.mage.pos = tile_center(cx, cy);
    let wall_cmds = agents.cast_wand(&world, tile_center(cx + 4, cy));
    apply_cmds(&mut world, wall_cmds);
    for _ in 0..45 {
        world.step();
        let cmds = agents.update(&world, SIM_DT);
        apply_cmds(&mut world, cmds);
    }
    let stone_wall_tiles = (-3..=3)
        .filter(|dy| world.get(cx + 4, cy + dy).expect("in bounds").material == Material::Stone)
        .count();
    assert!(stone_wall_tiles >= 3);
}

#[test]
fn mage_dash_cannot_tunnel_through_thin_wall_and_terrawall_spares_mage() {
    let mut world = World::new(Preset::DroughtValley, 109);
    let (cx, cy) = world.village_center();
    for dy in -8..=8 {
        for dx in -8..=8 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    // Build a 1-tile thick Stone wall at cx + 1
    for dy in -5..=5 {
        world.set_material(cx + 1, cy + dy, Material::Stone);
    }

    let mut agents = Agents::new(109);
    agents.mage.pos = tile_center(cx, cy);
    agents.toggle_mage_mode();

    // Dash right with worst-case lag frame dt = 0.10 (27px > 16px tile width)
    let _ = agents.drive_mage(
        &world,
        MageInput {
            move_axis: Vec2::new(1.0, 0.0),
            aim_world: tile_center(cx + 4, cy),
            dash_pressed: true,
            cast_down: false,
        },
        0.10,
    );
    let wall_left_edge = (cx + 1) as f32 * TILE_PX;
    assert!(agents.mage.pos.x < wall_left_edge);

    // TerraWall cast directly onto the Mage's tile must never place Stone over the Mage
    agents.mage.pos = tile_center(cx - 2, cy);
    let mut cmds = Vec::new();
    agents.apply_spell_impact(
        &world,
        Payload::TerraWall,
        SpellImpact {
            pos: tile_center(cx - 2, cy),
            vel: Vec2::new(100.0, 0.0),
            mods: RuneMods {
                homing: 0,
                explosive: 0,
                electric: 0,
            },
        },
        &mut cmds,
    );
    apply_cmds(&mut world, cmds);
    assert_ne!(
        world.get(cx - 2, cy).expect("in bounds").material,
        Material::Stone
    );
    assert_eq!(
        world.get(cx - 2, cy + 1).expect("in bounds").material,
        Material::Stone
    );
}

#[test]
fn villager_bucket_scoop_exact_and_death_conserves_water_and_drops() {
    let mut world = World::new(Preset::DroughtValley, 110);
    let (cx, cy) = world.village_center();
    for dy in -5..=5 {
        for dx in -5..=5 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    world.set_material(cx + 1, cy, Material::ShallowWater);
    if let Some(cell) = world.get_mut(cx + 1, cy) {
        cell.water_volume = 10;
    }
    let water_before = world.total_water_units();

    let mut agents = Agents::new(110);
    agents.spawn_villager(tile_center(cx, cy));
    {
        let v = &mut agents.villagers[0];
        v.job = JobKind::Extinguish;
        v.bucket_water = 0;
        v.target_tile = Some((cx + 1, cy));
        v.target_adjacent = true;
        v.decision_timer = 5.0;
    }
    let cmds = agents.update(&world, SIM_DT);
    apply_cmds(&mut world, cmds);

    // Scooped 9 units (leaving 1) so bucket_water matches exact removed amount
    assert_eq!(agents.villagers[0].bucket_water, 9);
    assert_eq!(world.total_water_units() + 9, water_before);

    // Give villager Wood and trigger lethal damage; death spills water & drops Wood
    agents.villagers[0].carrying = Some(ItemKind::Wood);
    agents.villagers[0].hp = 0.0;
    let death_cmds = agents.update(&world, SIM_DT);
    apply_cmds(&mut world, death_cmds);

    assert!(agents.villagers.is_empty());
    assert_eq!(world.total_water_units(), water_before);
    assert!(agents.drops.iter().any(|d| d.kind == ItemKind::Wood));
}

#[test]
fn orbit_homing_combo_tracks_target_and_warm_target_revalidates() {
    let mut world = World::new(Preset::DroughtValley, 111);
    let (cx, cy) = world.village_center();
    for dy in -8..=8 {
        for dx in -8..=8 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    world.set_material(cx, cy, Material::Campfire);

    // Warm job target becomes invalid when its Campfire is extinguished
    let mut agents = Agents::new(111);
    agents.spawn_villager(tile_center(cx + 1, cy));
    agents.villagers[0].job = JobKind::Warm;
    agents.villagers[0].target_tile = Some((cx + 1, cy));
    let scan_lit = VillageScan::scan(&world);
    assert!(is_job_target_still_valid(
        &world,
        &scan_lit,
        &agents.villagers[0],
        &[]
    ));
    if let Some(cf) = world.get_mut(cx, cy) {
        cf.set_lit_campfire(false);
    }
    let scan_unlit = VillageScan::scan(&world);
    assert!(!is_job_target_still_valid(
        &world,
        &scan_unlit,
        &agents.villagers[0],
        &[]
    ));

    // Orbit + Homing launches a homing bolt on shard expiry when a Wolf is nearby
    agents.mage.pos = tile_center(cx, cy);
    agents.toggle_mage_mode();
    agents.spawn_critter(CritterKind::Wolf, tile_center(cx + 4, cy));
    agents.mage.wand.payload = Payload::Fireball;
    agents.mage.wand.runes = [Some(Rune::Orbit), Some(Rune::Homing), None];
    agents.mage.wand.mana = 100.0;
    agents.mage.wand.cooldown = 0.0;
    let _ = agents.cast_wand(&world, tile_center(cx + 4, cy));
    assert_eq!(agents.orbit_shards.len(), 1);
    agents.orbit_shards[0].ttl = 0.01;
    let _ = agents.update(&world, 0.02);
    assert!(!agents.spells.is_empty());
}

#[test]
fn extinguish_outscores_warm_at_night_within_18_tiles() {
    let mut world = World::new(Preset::DroughtValley, 112);
    let (cx, cy) = world.village_center();
    for dy in -20..=20 {
        for dx in -20..=20 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    world.time_of_day = 0.90;
    assert!(world.is_night());

    world.set_material(cx, cy, Material::Campfire);
    world.set_material(cx - 2, cy, Material::ShallowWater);
    world.set_material(cx + EXTINGUISH_RADIUS, cy, Material::Fire);
    let scan = VillageScan::scan(&world);
    assert!(scan.village_fires.contains(&(cx + EXTINGUISH_RADIUS, cy)));

    let mut agents = Agents::new(112);
    agents.spawn_villager(tile_center(cx + 1, cy));
    let colony = ColonyState {
        wood_stored: 4,
        food_stored: 10,
        villager_count: 1,
        total_crops_harvested: 0,
        sim_time: 0.0,
    };

    for &warmth in &[0.95_f32, 0.40_f32] {
        for &cur_job in &[JobKind::Idle, JobKind::Warm] {
            let v = &mut agents.villagers[0];
            v.job = cur_job;
            v.hunger = 0.90;
            v.warmth = warmth;
            v.bucket_water = 0;

            let ext = score_extinguish(&scan, v, colony.sim_time);
            let warm = score_warm(&world, &scan, v);
            assert!(
                ext > warm,
                "Extinguish ({ext}) must outscore Warm ({warm}) at night for fire within 18 tiles (warmth={warmth})"
            );
            let chosen = pick_best_job(&world, &scan, v, &[], colony, &[]);
            assert_eq!(
                chosen,
                JobKind::Extinguish,
                "pick_best_job should choose Extinguish over Warm at night (cur_job={cur_job:?}, warmth={warmth})"
            );
        }
    }
}

#[test]
fn villager_skips_stone_walled_ripe_crop_and_does_other_work() {
    let mut world = World::new(Preset::DroughtValley, 113);
    let (cx, cy) = world.village_center();
    for dy in -12..=12 {
        for dx in -12..=12 {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    // Place a ripe crop walled completely off by Stone
    let crop_x = cx + 4;
    let crop_y = cy;
    world.set_material(crop_x, crop_y, Material::Crop(4));
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx != 0 || dy != 0 {
                world.set_material(crop_x + dx, crop_y + dy, Material::Stone);
            }
        }
    }
    // Place a reachable Chest and Tree on the open side
    world.set_material(cx - 2, cy, Material::Chest);
    world.set_material(cx - 4, cy, Material::Tree);

    let mut agents = Agents::new(113);
    agents.wood_stored = 0;
    agents.food_stored = 10;
    agents.spawn_villager(tile_center(cx, cy));

    let mut did_other_work = false;
    for _ in 0..480 {
        world.step();
        let cmds = agents.update(&world, SIM_DT);
        apply_cmds(&mut world, cmds);
        if agents.villagers[0].job != JobKind::HarvestHaul
            && agents.villagers[0].job != JobKind::Idle
        {
            did_other_work = true;
        }
        // Tree chopped or soil tilled proves villager moved on to productive work
        if world
            .get(cx - 4, cy)
            .is_some_and(|c| c.material != Material::Tree)
            || agents.wood_stored > 0
        {
            did_other_work = true;
            break;
        }
    }

    assert!(
        did_other_work,
        "villager should skip Stone-walled ripe crop and move on to other work (job={:?})",
        agents.villagers[0].job
    );
    // Walled-off crop must still be untouched
    assert_eq!(
        world.get(crop_x, crop_y).expect("in bounds").material,
        Material::Crop(4)
    );
}

#[test]
fn eat_scores_zero_when_food_unreachable_and_astar_failure_blacklists() {
    let mut world = World::new(Preset::DroughtValley, 114);
    let (cx, cy) = world.village_center();
    for dy in -VILLAGE_SCAN_RADIUS..=VILLAGE_SCAN_RADIUS {
        for dx in -VILLAGE_SCAN_RADIUS..=VILLAGE_SCAN_RADIUS {
            world.set_material(cx + dx, cy + dy, Material::Dirt);
        }
    }
    // Wall off a ripe BerryBush with Stone and leave zero stored food
    world.set_material(cx + 4, cy, Material::BerryBush);
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx != 0 || dy != 0 {
                world.set_material(cx + 4 + dx, cy + dy, Material::Stone);
            }
        }
    }

    let scan = VillageScan::scan(&world);
    assert!(
        !scan.ripe_bushes.contains(&(cx + 4, cy)),
        "VillageScan should drop unreachable BerryBush"
    );

    let mut agents = Agents::new(114);
    agents.spawn_villager(tile_center(cx, cy));
    agents.villagers[0].hunger = 0.05;
    let colony = ColonyState {
        wood_stored: 0,
        food_stored: 0,
        villager_count: 1,
        total_crops_harvested: 0,
        sim_time: 0.0,
    };
    assert_eq!(
        score_eat(&scan, &agents.villagers[0], colony),
        0.0,
        "Eat must score 0 when no reachable food exists"
    );

    // Direct A* failure against a walled-off target must blacklist (job, target) for 30s
    {
        let v = &mut agents.villagers[0];
        v.job = JobKind::LumberBuild;
        v.target_tile = Some((cx + 4, cy));
        v.target_adjacent = true;
        v.needs_path = true;
        v.decision_timer = 5.0;
    }
    let _ = agents.update(&world, SIM_DT);
    assert!(agents.villagers[0].is_blacklisted(JobKind::LumberBuild, (cx + 4, cy), 10.0));
    assert!(!agents.villagers[0].is_blacklisted(JobKind::LumberBuild, (cx + 4, cy), 31.0));
}
