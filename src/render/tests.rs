use macroquad::prelude::vec2;

use super::lighting::{
    ambient_light_color, collect_scene_glows_and_fireflies,
    collect_scene_glows_and_fireflies_with_cap, lightmap_dimensions, LIGHTMAP_DIVISOR,
    MAX_GLOW_LIGHTS,
};
use super::terrain::{
    lit_terrain_luminance, palette_shades, rasterize_terrain_into, render_terrain, rgb_luminance,
    TerrainRasterizer, MICRO_PX, MIN_LIT_TERRAIN_LUMA, MIN_TERRAIN_UNLIT_LUMA, TERRAIN_IMG_H,
    TERRAIN_IMG_W, TERRAIN_PALETTES,
};
use super::world_draw::{WORLD_H_PX, WORLD_W_PX};
use crate::agents::Agents;
use crate::world::{Material, Preset, Weather, World, GRID_H, GRID_W};

#[test]
fn render_terrain_is_deterministic_sized_and_warm_lava() {
    let mut world = World::new(Preset::VolcanicCrater, 42);
    let lava_x = 80_usize;
    let lava_y = 40_usize;
    world.set_material(lava_x as i32, lava_y as i32, Material::Lava);

    let img_a = render_terrain(&world, 1.25);
    let img_b = render_terrain(&world, 1.25);

    assert_eq!(img_a.width(), TERRAIN_IMG_W);
    assert_eq!(img_a.height(), TERRAIN_IMG_H);
    assert_eq!(img_a.width(), 640);
    assert_eq!(img_a.height(), 400);
    assert_eq!(img_a.bytes.len(), 640 * 400 * 4);
    assert_eq!(img_a.bytes, img_b.bytes);

    for my in 0..MICRO_PX {
        for mx in 0..MICRO_PX {
            let px = lava_x * MICRO_PX + mx;
            let py = lava_y * MICRO_PX + my;
            let off = (py * TERRAIN_IMG_W + px) * 4;
            let r = img_a.bytes[off];
            let g = img_a.bytes[off + 1];
            let b = img_a.bytes[off + 2];
            let a = img_a.bytes[off + 3];
            assert_eq!(a, 255);
            assert!(
                r >= 180 && r > g && r as u16 > (b as u16) * 2,
                "Lava micro-pixel ({px},{py}) expected warm RGB, got ({r},{g},{b})"
            );
        }
    }

    let img_later = render_terrain(&world, 2.75);
    assert_ne!(img_a.bytes, img_later.bytes);
}

#[test]
fn dirty_chunk_rasterizer_matches_full_render_and_benchmarks() {
    let mut world = World::new(Preset::LushRiverIsland, 42);
    let mut rast = TerrainRasterizer::new(&world, 0.0);
    assert_eq!(rast.img.bytes, render_terrain(&world, 0.0).bytes);

    // Edit across a chunk border (x = 15/16) + trigger fire/water/explosion
    world.set_material(15, 50, Material::Stone);
    world.set_material(16, 50, Material::Dirt);
    world.set_material(80, 50, Material::Fire);
    world.explode(76, 52, 3, 65.0);
    for _ in 0..10 {
        world.step();
    }

    let t = 1.45;
    assert!(rast.update(&world, t));
    assert_eq!(rast.img.bytes, render_terrain(&world, t).bytes);

    // Benchmark 120 simulated 60 FPS frames (2 seconds): full vs dirty-chunk + 10 Hz animated
    let mut full_img = render_terrain(&world, 0.0);
    let t_full = std::time::Instant::now();
    for i in 0..120_u32 {
        rasterize_terrain_into(&world, i as f32 / 60.0, &mut full_img);
    }
    let full_ms = t_full.elapsed().as_secs_f64() * 1000.0 / 120.0;

    let mut dirty_rast = TerrainRasterizer::new(&world, 0.0);
    let t_dirty = std::time::Instant::now();
    let mut uploads = 0_u32;
    for i in 0..120_u32 {
        if dirty_rast.update(&world, i as f32 / 60.0) {
            uploads += 1;
        }
    }
    let dirty_ms = t_dirty.elapsed().as_secs_f64() * 1000.0 / 120.0;
    assert_eq!(
        dirty_rast.img.bytes,
        render_terrain(&world, 119.0 / 60.0).bytes
    );

    eprintln!(
        "[terrain bench 120f@60Hz] full={full_ms:.3} ms/frame | dirty+10Hz={dirty_ms:.3} ms/frame (uploads={uploads}/120)"
    );
}

#[test]
fn generator_lush_stone_ratio_river_visible_and_farm_clearance() {
    let mut min_stone_pct = 1.0_f32;
    let mut max_stone_pct = 0.0_f32;
    let mut sum_stone_pct = 0.0_f32;
    let mut min_river_tiles = usize::MAX;
    let mut min_meander_span = i32::MAX;

    for seed in 1..=30_u64 {
        let lush = World::new(Preset::LushRiverIsland, seed);
        let (lcx, lcy) = lush.village_center();
        let mut land = 0_usize;
        let mut stone = 0_usize;
        let mut river_in_view = 0_usize;
        let mut river_min_x = i32::MAX;
        let mut river_max_x = i32::MIN;

        for y in 0..GRID_H {
            let mut row_water_sum = 0_i32;
            let mut row_water_cnt = 0_i32;
            for x in 0..GRID_W {
                let mat = lush.cells[y * GRID_W + x].material;
                if !mat.is_water() {
                    land += 1;
                    if mat == Material::Stone {
                        stone += 1;
                    }
                } else if (48..=84).contains(&x) && (12..=88).contains(&y) {
                    row_water_sum += x as i32;
                    row_water_cnt += 1;
                }
                let dx = x as i32 - lcx;
                let dy = y as i32 - lcy;
                if dx * dx + dy * dy <= 12 * 12 {
                    assert_ne!(
                        mat,
                        Material::Stone,
                        "Lush seed {seed}: stone found within 12 tiles of village at ({x},{y})"
                    );
                }
                if mat.is_water() && dx.abs() <= 20 && dy.abs() <= 12 {
                    river_in_view += 1;
                }
            }
            if row_water_cnt > 0 {
                let row_cx = row_water_sum / row_water_cnt;
                river_min_x = river_min_x.min(row_cx);
                river_max_x = river_max_x.max(row_cx);
            }
        }
        let meander_span = (river_max_x - river_min_x).max(0);
        min_meander_span = min_meander_span.min(meander_span);
        assert!(
            meander_span >= 8,
            "Lush seed {seed}: river meander span too narrow ({meander_span} tiles)"
        );

        let stone_pct = stone as f32 / land.max(1) as f32;
        min_stone_pct = min_stone_pct.min(stone_pct);
        max_stone_pct = max_stone_pct.max(stone_pct);
        sum_stone_pct += stone_pct;
        min_river_tiles = min_river_tiles.min(river_in_view);
        assert!(
            (0.018..=0.075).contains(&stone_pct),
            "Lush seed {seed}: stone covers {:.2}% of land (expected ~3-5%)",
            stone_pct * 100.0
        );
        assert!(
            river_in_view >= 20,
            "Lush seed {seed}: river barely visible from start camera ({river_in_view} water tiles)"
        );

        let drought = World::new(Preset::DroughtValley, seed);
        let (dcx, dcy) = drought.village_center();
        for dy in -12..=12_i32 {
            for dx in -12..=12_i32 {
                if dx * dx + dy * dy <= 12 * 12 {
                    let mat = drought.get(dcx + dx, dcy + dy).map(|c| c.material);
                    assert_ne!(
                        mat,
                        Some(Material::Stone),
                        "Drought seed {seed}: stone within 12 tiles of village at ({},{})",
                        dcx + dx,
                        dcy + dy
                    );
                }
            }
        }
    }
    let mean_stone_pct = sum_stone_pct / 30.0;
    assert!(
        (0.03..=0.05).contains(&mean_stone_pct),
        "Lush mean stone percentage {:.2}% outside 3-5% target",
        mean_stone_pct * 100.0
    );
    eprintln!(
        "[gen lush 1..=30] stone_pct: min={:.2}% mean={:.2}% max={:.2}% | min_meander_span={}t | min_river_tiles_in_start_view={}",
        min_stone_pct * 100.0,
        mean_stone_pct * 100.0,
        max_stone_pct * 100.0,
        min_meander_span,
        min_river_tiles
    );
}

#[test]
fn wet_soil_luminance_and_min_lit_terrain_luminance_at_night_and_rain() {
    let wet_shades = palette_shades(Material::WetSoil);
    let tilled_shades = palette_shades(Material::TilledSoil);
    let dirt_shades = palette_shades(Material::Dirt);

    let wet_min = wet_shades
        .iter()
        .copied()
        .map(rgb_luminance)
        .fold(f32::INFINITY, f32::min);
    let wet_mean: f32 = wet_shades.iter().copied().map(rgb_luminance).sum::<f32>() / 4.0;
    let tilled_mean: f32 = tilled_shades
        .iter()
        .copied()
        .map(rgb_luminance)
        .sum::<f32>()
        / 4.0;
    let dirt_mean: f32 = dirt_shades.iter().copied().map(rgb_luminance).sum::<f32>() / 4.0;

    // Raised from old 0.127 minimum to >= 0.25 while keeping WetSoil < TilledSoil < Dirt
    assert!(
        wet_min >= 0.25,
        "WetSoil min unlit luminance {wet_min:.3} should be >= 0.25"
    );
    assert!(
        wet_mean < tilled_mean && tilled_mean < dirt_mean,
        "Expected WetSoil ({wet_mean:.3}) < TilledSoil ({tilled_mean:.3}) < Dirt ({dirt_mean:.3})"
    );

    let night_clear = ambient_light_color(0.90, Weather::Clear);
    let night_rain = ambient_light_color(0.90, Weather::Rainstorm);

    for (mat_idx, pal) in TERRAIN_PALETTES.iter().enumerate() {
        for (shade_idx, &rgb) in pal.iter().enumerate() {
            let unlit = rgb_luminance(rgb);
            assert!(
                unlit >= MIN_TERRAIN_UNLIT_LUMA,
                "Palette {mat_idx} shade {shade_idx} unlit luma {unlit:.3} < {MIN_TERRAIN_UNLIT_LUMA}"
            );
            let lit_night = lit_terrain_luminance(rgb, night_clear);
            let lit_rain = lit_terrain_luminance(rgb, night_rain);
            assert!(
                lit_night >= MIN_LIT_TERRAIN_LUMA - 1e-3,
                "Palette {mat_idx} shade {shade_idx} night lit luma {lit_night:.3} < {MIN_LIT_TERRAIN_LUMA}"
            );
            assert!(
                lit_rain >= MIN_LIT_TERRAIN_LUMA - 1e-3,
                "Palette {mat_idx} shade {shade_idx} night+rain lit luma {lit_rain:.3} < {MIN_LIT_TERRAIN_LUMA}"
            );
        }
    }

    // Also verify every rasterized pixel across a full world render (including rim-darkened edges)
    let world = World::new(Preset::LushRiverIsland, 42);
    let img = render_terrain(&world, 0.5);
    let mut worst_lit_rain = f32::INFINITY;
    for px in img.bytes.chunks_exact(4) {
        let rgb = [px[0], px[1], px[2]];
        let l = lit_terrain_luminance(rgb, night_rain);
        worst_lit_rain = worst_lit_rain.min(l);
    }
    assert!(
        worst_lit_rain >= MIN_LIT_TERRAIN_LUMA - 1e-3,
        "Worst rasterized pixel lit luma in night+rain {worst_lit_rain:.3} < {MIN_LIT_TERRAIN_LUMA}"
    );
}

#[test]
fn lightmap_quarter_res_and_glow_cap_nearest_64() {
    let ((sw, sh), (lw, lh)) = lightmap_dimensions(1280.0, 800.0, 2.0, LIGHTMAP_DIVISOR);
    assert_eq!((sw, sh), (2560, 1600));
    assert_eq!((lw, lh), (640, 400));

    let mut world = World::new(Preset::VolcanicCrater, 42);
    world.time_of_day = 0.90;
    let mut agents = Agents::new(42);
    agents.populate_for_world(&world);
    let center = vec2(WORLD_W_PX * 0.5, WORLD_H_PX * 0.5);

    let (_, uncapped) = collect_scene_glows_and_fireflies_with_cap(
        &world,
        &agents,
        (0, 0),
        (GRID_W, GRID_H),
        center,
        1.0,
        usize::MAX,
    );
    let (_, capped) =
        collect_scene_glows_and_fireflies(&world, &agents, (0, 0), (GRID_W, GRID_H), center, 1.0);

    assert!(
        uncapped.len() > MAX_GLOW_LIGHTS,
        "VolcanicCrater at fit zoom at night should have >64 raw glow lights, got {}",
        uncapped.len()
    );
    assert_eq!(capped.len(), MAX_GLOW_LIGHTS);

    let max_capped_d2 = capped
        .iter()
        .map(|g| g.pos.distance_squared(center))
        .fold(0.0_f32, f32::max);
    let closer_in_uncapped = uncapped
        .iter()
        .filter(|g| g.pos.distance_squared(center) < max_capped_d2 - 1e-2)
        .count();
    assert!(closer_in_uncapped < MAX_GLOW_LIGHTS);
}
