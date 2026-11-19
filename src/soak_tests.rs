use std::collections::HashMap;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::agents::{Agents, JobKind};
use crate::ui::apply_world_cmds;
use crate::world::{Material, Preset, World, GRID_H, GRID_W, SIM_DT, TILE_PX};

const SOAK_MINUTES: usize = 20;
const SOAK_TICKS: usize = SOAK_MINUTES * 60 * 60; // 72,000 ticks at 60 Hz
const MINUTE_TICKS: usize = 60 * 60; // 3,600 ticks per simulated minute
const MAX_STALL_REASSIGNMENTS: usize = 5;
const SOAK_SEED: u64 = 42;
const CHAOS_SEED: u64 = 99;

struct JobTracker {
    job: JobKind,
    unchanged_secs: f32,
    active_secs: u32,
    start_tod: f32,
    last_pos: macroquad::prelude::Vec2,
    idle_pos_secs: f32,
    idle_start_tod: f32,
    last_harvests: u32,
    last_wood: u32,
    last_food: u32,
}

#[derive(Clone, Debug)]
struct PeakDetail {
    secs: f32,
    job: JobKind,
    start_tod: f32,
    end_tod: f32,
    active_secs: u32,
}

impl Default for PeakDetail {
    fn default() -> Self {
        Self {
            secs: 0.0,
            job: JobKind::Idle,
            start_tod: 0.0,
            end_tod: 0.0,
            active_secs: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct ReassignPeak {
    count: usize,
    villager_id: usize,
    job: JobKind,
    target: (i32, i32),
    disp_tiles: f32,
}

impl Default for ReassignPeak {
    fn default() -> Self {
        Self {
            count: 0,
            villager_id: 0,
            job: JobKind::Idle,
            target: (0, 0),
            disp_tiles: 0.0,
        }
    }
}

struct SoakReport {
    villagers: usize,
    water: u64,
    wood: u32,
    food: u32,
    harvests: u32,
    same_job: PeakDetail,
    stuck: PeakDetail,
    reassign: ReassignPeak,
}

fn total_water(w: &World, a: &Agents) -> u64 {
    let buckets: u64 = a.villagers.iter().map(|v| v.bucket_water as u64).sum();
    w.total_water_units() + buckets
}

fn simulate_preset_soak(preset: Preset, seed: u64) -> SoakReport {
    let mut world = World::new(preset, seed);
    let mut agents = Agents::new(seed);
    agents.populate_for_world(&world);

    let initial_water = total_water(&world, &agents);
    let mut trackers: HashMap<usize, JobTracker> = HashMap::new();
    let mut same_job = PeakDetail::default();
    let mut stuck = PeakDetail::default();
    let mut reassign = ReassignPeak::default();

    for tick in 0..SOAK_TICKS {
        world.step();
        let cmds = agents.update(&world, SIM_DT);
        apply_world_cmds(&mut world, cmds);

        // Check water conservation every 60 ticks (1s) and on final tick
        if tick % 60 == 59 || tick + 1 == SOAK_TICKS {
            let cur_water = total_water(&world, &agents);
            assert_eq!(
                cur_water,
                initial_water + world.rain_water_added,
                "{} (seed {}): water not conserved at tick {}: got {}, expected {}",
                preset.name(),
                seed,
                tick,
                cur_water,
                initial_water + world.rain_water_added
            );
        }

        // Sample villager job progress & B9 reassignment stall detector once per second (60 ticks)
        if tick % 60 == 59 {
            let sim_time = agents.sim_time();
            for v in &agents.villagers {
                let (cnt, rjob, rtarget, disp_px) = v.worst_stall_reassignments(sim_time);
                if cnt > MAX_STALL_REASSIGNMENTS && reassign.count <= MAX_STALL_REASSIGNMENTS {
                    let mat = world.get(rtarget.0, rtarget.1).map(|c| c.material);
                    let vtx = (v.pos.x / TILE_PX).floor() as i32;
                    let vty = (v.pos.y / TILE_PX).floor() as i32;
                    eprintln!(
                        "[DIAG {} s{} t={:.1}s tod={:.2} night={}] v{} pos=({:.1},{:.1}) tile=({},{}) job={:?} rjob={:?} rtarget={:?} mat={:?} carry={:?} hunger={:.2} food={} cnt={} disp={:.2}t",
                        preset.name(),
                        seed,
                        sim_time,
                        world.time_of_day,
                        world.is_night(),
                        v.id,
                        v.pos.x,
                        v.pos.y,
                        vtx,
                        vty,
                        v.job,
                        rjob,
                        rtarget,
                        mat,
                        v.carrying,
                        v.hunger,
                        agents.food_stored,
                        cnt,
                        disp_px / TILE_PX,
                    );
                }
                if cnt > reassign.count {
                    reassign = ReassignPeak {
                        count: cnt,
                        villager_id: v.id,
                        job: rjob,
                        target: rtarget,
                        disp_tiles: disp_px / TILE_PX,
                    };
                }

                let entry = trackers.entry(v.id).or_insert_with(|| JobTracker {
                    job: v.job,
                    unchanged_secs: 0.0,
                    active_secs: 0,
                    start_tod: world.time_of_day,
                    last_pos: v.pos,
                    idle_pos_secs: 0.0,
                    idle_start_tod: world.time_of_day,
                    last_harvests: agents.total_crops_harvested,
                    last_wood: agents.wood_stored,
                    last_food: agents.food_stored,
                });
                if entry.job == v.job {
                    entry.unchanged_secs += 1.0;
                    let moved = v.pos.distance(entry.last_pos) > 4.0;
                    let acted = v.work_timer > 0.0
                        || v.action_tween > 0.0
                        || agents.total_crops_harvested != entry.last_harvests
                        || agents.wood_stored != entry.last_wood
                        || agents.food_stored != entry.last_food;
                    if moved || acted {
                        entry.active_secs += 1;
                        entry.idle_pos_secs = 0.0;
                        entry.idle_start_tod = world.time_of_day;
                    } else {
                        entry.idle_pos_secs += 1.0;
                    }
                    entry.last_pos = v.pos;
                    entry.last_harvests = agents.total_crops_harvested;
                    entry.last_wood = agents.wood_stored;
                    entry.last_food = agents.food_stored;
                } else {
                    entry.job = v.job;
                    entry.unchanged_secs = 0.0;
                    entry.active_secs = 0;
                    entry.start_tod = world.time_of_day;
                    entry.idle_pos_secs = 0.0;
                    entry.idle_start_tod = world.time_of_day;
                    entry.last_pos = v.pos;
                    entry.last_harvests = agents.total_crops_harvested;
                    entry.last_wood = agents.wood_stored;
                    entry.last_food = agents.food_stored;
                }

                if entry.unchanged_secs > same_job.secs {
                    same_job = PeakDetail {
                        secs: entry.unchanged_secs,
                        job: entry.job,
                        start_tod: entry.start_tod,
                        end_tod: world.time_of_day,
                        active_secs: entry.active_secs,
                    };
                }
                if entry.idle_pos_secs > stuck.secs {
                    stuck = PeakDetail {
                        secs: entry.idle_pos_secs,
                        job: entry.job,
                        start_tod: entry.idle_start_tod,
                        end_tod: world.time_of_day,
                        active_secs: 0,
                    };
                }
            }
        }
    }

    let final_water = total_water(&world, &agents);
    assert_eq!(
        final_water,
        initial_water + world.rain_water_added,
        "{} (seed {}): final water mismatch",
        preset.name(),
        seed
    );
    assert!(
        !agents.villagers.is_empty(),
        "{} (seed {}): all villagers died during 20-minute soak",
        preset.name(),
        seed
    );
    assert!(
        reassign.count <= MAX_STALL_REASSIGNMENTS,
        "{} (seed {}): villager {} reassigned ({:?}, {:?}) {} times in 30s while moving only {:.2} tiles (> {})",
        preset.name(),
        seed,
        reassign.villager_id,
        reassign.job,
        reassign.target,
        reassign.count,
        reassign.disp_tiles,
        MAX_STALL_REASSIGNMENTS
    );

    SoakReport {
        villagers: agents.villagers.len(),
        water: final_water,
        wood: agents.wood_stored,
        food: agents.food_stored,
        harvests: agents.total_crops_harvested,
        same_job,
        stuck,
        reassign,
    }
}

fn run_preset_soak(preset: Preset, seed: u64) {
    let r = simulate_preset_soak(preset, seed);
    let reassign_str = if r.reassign.count >= 4 {
        format!(
            "{}/30s ({:?} @ {:?}, v{}, disp={:.2}t)",
            r.reassign.count,
            r.reassign.job,
            r.reassign.target,
            r.reassign.villager_id,
            r.reassign.disp_tiles
        )
    } else {
        format!("{}/30s ({:?})", r.reassign.count, r.reassign.job)
    };
    eprintln!(
        "[soak {}] seed={} ticks={} villagers={} water={} wood={} food={} harvests={} max_same_job={:.0}s ({:?}, active={}s, tod={:.2}->{:.2}) max_stuck={:.0}s ({:?}, tod={:.2}->{:.2}) max_reassign={}",
        preset.name(),
        seed,
        SOAK_TICKS,
        r.villagers,
        r.water,
        r.wood,
        r.food,
        r.harvests,
        r.same_job.secs,
        r.same_job.job,
        r.same_job.active_secs,
        r.same_job.start_tod,
        r.same_job.end_tod,
        r.stuck.secs,
        r.stuck.job,
        r.stuck.start_tod,
        r.stuck.end_tod,
        reassign_str,
    );
}

fn find_dry_chaos_tile(world: &World, rng: &mut StdRng, cx: i32, cy: i32) -> Option<(i32, i32)> {
    for _ in 0..80 {
        let dx: i32 = rng.gen_range(-34..=34);
        let dy: i32 = rng.gen_range(-34..=34);
        if dx.abs().max(dy.abs()) < 8 {
            continue;
        }
        let x = cx + dx;
        let y = cy + dy;
        if x < 2 || y < 2 || x >= GRID_W as i32 - 2 || y >= GRID_H as i32 - 2 {
            continue;
        }
        let Some(cell) = world.get(x, y) else {
            continue;
        };
        if cell.water_volume == 0
            && !cell.material.is_water()
            && !matches!(
                cell.material,
                Material::Campfire
                    | Material::Chest
                    | Material::Cabin
                    | Material::Lava
                    | Material::Magma
            )
        {
            return Some((x, y));
        }
    }
    None
}

fn run_chaos_soak(preset: Preset, seed: u64) {
    let mut world = World::new(preset, seed);
    let mut agents = Agents::new(seed);
    agents.populate_for_world(&world);
    let mut chaos_rng = StdRng::seed_from_u64(seed.wrapping_add(0xC4A05));

    let (cx, cy) = world.village_center();
    let initial_water = total_water(&world, &agents);
    let mut weather_cycles = 0_u32;
    let mut fires_ignited = 0_u32;
    let mut gunpowder_detonated = 0_u32;

    for tick in 0..SOAK_TICKS {
        // Every simulated minute: cycle weather and ignite random fires & gunpowder
        if tick > 0 && tick.is_multiple_of(MINUTE_TICKS) {
            world.cycle_weather();
            weather_cycles += 1;

            for _ in 0..4 {
                if let Some((fx, fy)) = find_dry_chaos_tile(&world, &mut chaos_rng, cx, cy) {
                    world.paint_fire(fx, fy, 520.0);
                    fires_ignited += 1;
                }
            }

            for _ in 0..2 {
                if let Some((gx, gy)) = find_dry_chaos_tile(&world, &mut chaos_rng, cx, cy) {
                    let mut placed = false;
                    for (ox, oy) in [(0, 0), (1, 0), (0, 1)] {
                        let tx = gx + ox;
                        let ty = gy + oy;
                        if world.get(tx, ty).is_some_and(|c| {
                            c.water_volume == 0
                                && !c.material.is_water()
                                && !matches!(
                                    c.material,
                                    Material::Campfire | Material::Chest | Material::Cabin
                                )
                        }) {
                            world.set_material(tx, ty, Material::Gunpowder);
                            placed = true;
                        }
                    }
                    if placed {
                        world.ignite(gx, gy, 400.0);
                        gunpowder_detonated += 1;
                    }
                }
            }
        }

        world.step();
        let cmds = agents.update(&world, SIM_DT);
        apply_world_cmds(&mut world, cmds);

        if tick % 60 == 59 || tick + 1 == SOAK_TICKS {
            let expected = initial_water + world.rain_water_added;
            let cur = total_water(&world, &agents);
            assert_eq!(
                cur,
                expected,
                "[chaos {}] water not conserved at tick {}: got {}, expected {} (initial={} + rain={})",
                preset.name(),
                tick,
                cur,
                expected,
                initial_water,
                world.rain_water_added
            );
        }
    }

    let expected_final = initial_water + world.rain_water_added;
    let final_water = total_water(&world, &agents);
    eprintln!(
        "[chaos {}] seed={} ticks={} villagers={} water={} (init={} + rain={}) weather_cycles={} fires={} gunpowder={} harvests={}",
        preset.name(),
        seed,
        SOAK_TICKS,
        agents.villagers.len(),
        final_water,
        initial_water,
        world.rain_water_added,
        weather_cycles,
        fires_ignited,
        gunpowder_detonated,
        agents.total_crops_harvested,
    );

    assert_eq!(
        final_water,
        expected_final,
        "[chaos {}] final water mismatch",
        preset.name()
    );
    assert!(
        !agents.villagers.is_empty(),
        "[chaos {}] all villagers died during 20-minute chaos soak",
        preset.name()
    );
}

#[test]
fn soak_20min_lush_river_island() {
    run_preset_soak(Preset::LushRiverIsland, SOAK_SEED);
}

#[test]
fn soak_20min_volcanic_crater_village() {
    run_preset_soak(Preset::VolcanicCrater, SOAK_SEED);
}

#[test]
fn soak_20min_drought_valley() {
    run_preset_soak(Preset::DroughtValley, SOAK_SEED);
}

#[test]
fn chaos_soak_lush_river_island() {
    run_chaos_soak(Preset::LushRiverIsland, CHAOS_SEED);
}

#[test]
fn chaos_soak_volcanic_crater_village() {
    run_chaos_soak(Preset::VolcanicCrater, CHAOS_SEED);
}

#[test]
fn chaos_soak_drought_valley() {
    run_chaos_soak(Preset::DroughtValley, CHAOS_SEED);
}

#[test]
#[ignore]
fn soak_20min_seeds_1_to_30_all_presets() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    let mut jobs: Vec<(usize, Preset, u64)> = Vec::with_capacity(Preset::ALL.len() * 30);
    for (p_idx, &preset) in Preset::ALL.iter().enumerate() {
        for seed in 1..=30_u64 {
            jobs.push((p_idx, preset, seed));
        }
    }
    let next_job = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, u64, SoakReport)>> = Mutex::new(Vec::with_capacity(jobs.len()));
    let worker_count = std::thread::available_parallelism()
        .map(|n| n.get().clamp(3, 12))
        .unwrap_or(6);

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let jobs_ref = &jobs;
            let next_ref = &next_job;
            let res_ref = &results;
            scope.spawn(move || loop {
                let idx = next_ref.fetch_add(1, Ordering::Relaxed);
                if idx >= jobs_ref.len() {
                    break;
                }
                let (p_idx, preset, seed) = jobs_ref[idx];
                let rep = simulate_preset_soak(preset, seed);
                res_ref
                    .lock()
                    .expect("lock results")
                    .push((p_idx, seed, rep));
            });
        }
    });

    let mut all_results = results.into_inner().expect("results inner");
    assert_eq!(all_results.len(), jobs.len());
    all_results.sort_by_key(|&(p_idx, seed, _)| (p_idx, seed));

    for (p_idx, &preset) in Preset::ALL.iter().enumerate() {
        let mut worst_same = 0.0_f32;
        let mut worst_same_seed = 0_u64;
        let mut worst_same_job = JobKind::Idle;
        let mut worst_stuck = 0.0_f32;
        let mut worst_stuck_seed = 0_u64;
        let mut worst_stuck_job = JobKind::Idle;
        let mut worst_reassign = 0_usize;
        let mut worst_reassign_seed = 0_u64;
        let mut worst_reassign_job = JobKind::Idle;
        let mut worst_reassign_target = (0_i32, 0_i32);

        let mut harvests_sorted: Vec<u32> = all_results
            .iter()
            .filter(|&&(pi, _, _)| pi == p_idx)
            .map(|(_, _, r)| r.harvests)
            .collect();
        harvests_sorted.sort_unstable();
        let median_harvests = if harvests_sorted.len() >= 16 {
            (harvests_sorted[14] as f32 + harvests_sorted[15] as f32) * 0.5
        } else {
            harvests_sorted.first().copied().unwrap_or(0) as f32
        };
        let min_required_harvests = median_harvests * 0.40;
        let min_harvests = harvests_sorted.first().copied().unwrap_or(0);

        eprintln!("=== {} (seeds 1..=30) ===", preset.name());
        for &(_, seed, ref r) in all_results.iter().filter(|&&(pi, _, _)| pi == p_idx) {
            let reassign_detail = if r.reassign.count >= 4 {
                format!(
                    "{}/30s ({:?} @ {:?}, v{}, disp={:.2}t)",
                    r.reassign.count,
                    r.reassign.job,
                    r.reassign.target,
                    r.reassign.villager_id,
                    r.reassign.disp_tiles
                )
            } else {
                format!("{}/30s", r.reassign.count)
            };
            eprintln!(
                "  seed={:2} villagers={} water={} harvests={:3} max_same_job={:3.0}s ({:?}, active={}s) max_stuck={:3.0}s ({:?}) max_reassign={}",
                seed,
                r.villagers,
                r.water,
                r.harvests,
                r.same_job.secs,
                r.same_job.job,
                r.same_job.active_secs,
                r.stuck.secs,
                r.stuck.job,
                reassign_detail,
            );
            if r.same_job.secs > worst_same {
                worst_same = r.same_job.secs;
                worst_same_seed = seed;
                worst_same_job = r.same_job.job;
            }
            if r.stuck.secs > worst_stuck {
                worst_stuck = r.stuck.secs;
                worst_stuck_seed = seed;
                worst_stuck_job = r.stuck.job;
            }
            if r.reassign.count > worst_reassign {
                worst_reassign = r.reassign.count;
                worst_reassign_seed = seed;
                worst_reassign_job = r.reassign.job;
                worst_reassign_target = r.reassign.target;
            }
        }
        eprintln!(
            "  SUMMARY {}: median_harvests={:.1} min_harvests={} (40%_floor={:.1}) | worst_same_job={:.0}s ({:?} on seed {}) | worst_stuck={:.0}s ({:?} on seed {}) | worst_reassign={}/30s ({:?} @ {:?} on seed {})",
            preset.name(),
            median_harvests,
            min_harvests,
            min_required_harvests,
            worst_same,
            worst_same_job,
            worst_same_seed,
            worst_stuck,
            worst_stuck_job,
            worst_stuck_seed,
            worst_reassign,
            worst_reassign_job,
            worst_reassign_target,
            worst_reassign_seed,
        );

        for &(_, seed, ref r) in all_results.iter().filter(|&&(pi, _, _)| pi == p_idx) {
            assert!(
                r.harvests as f32 >= min_required_harvests,
                "{} (seed {}): crop harvests {} fell below 40% of 30-seed median {:.1} (required >= {:.1})",
                preset.name(),
                seed,
                r.harvests,
                median_harvests,
                min_required_harvests
            );
        }
    }
}
