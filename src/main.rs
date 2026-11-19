mod agents;
mod render;
mod ui;
mod world;

#[cfg(test)]
mod soak_tests;

use macroquad::prelude::*;

use agents::{Agents, CritterKind, DrawSceneOpts, MageInput, ProjectileKind, WorldCmd};
use render::{
    ambient_light_color, clamp_camera_center, collect_scene_glows_and_fireflies, draw_aim_reticle,
    draw_brush_preview, draw_firefly_dots, draw_gas_particles, draw_weather_overlay,
    draw_world_letterbox, fit_world_zoom, fit_world_zoom_snapped, make_camera, night_rim_strength,
    snap_zoom_nearest, snap_zoom_step, visible_tile_bounds, LightingPipeline, TerrainRasterizer,
    UI_BASE_BG, WORLD_H_PX, WORLD_W_PX,
};
use ui::{
    apply_world_cmds, compute_hud_layout, draw_cursor_tooltip, draw_hud, measure_ui_width,
    paint_circle_cmds, BrushTool, SpeedMode, UiAction, UiState,
};
use world::{Preset, World, SIM_DT, TILE_PX};

const SHAKE_DECAY: f32 = 2.4;
const MAX_SHAKE_PX: f32 = 12.0;

fn window_conf() -> Conf {
    Conf {
        window_title: "Hearthsand".to_owned(),
        window_width: 1280,
        window_height: 800,
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

fn village_center_px(world: &World) -> Vec2 {
    let (cx, cy) = world.village_center();
    vec2((cx as f32 + 0.5) * TILE_PX, (cy as f32 + 0.5) * TILE_PX)
}

fn reset_world_and_agents(
    preset: Preset,
    seed: u64,
    keep_mage_active: bool,
) -> (World, Agents, f32, f32) {
    let world = World::new(preset, seed);
    let mut agents = Agents::new(seed);
    agents.populate_for_world(&world);
    if keep_mage_active {
        agents.toggle_mage_mode();
    }
    (world, agents, 0.0, 0.0)
}

#[macroquad::main(window_conf)]
async fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("HEARTHSAND_BENCH_NIGHT").is_some() {
        run_night_fit_zoom_bench().await;
        return;
    }

    let mut preset = Preset::LushRiverIsland;
    let mut seed: u64 = 42;
    let (mut world, mut agents, mut sim_accum, mut shake_trauma) =
        reset_world_and_agents(preset, seed, false);

    let mut terrain_raster = TerrainRasterizer::new(&world, 0.0);
    let terrain_tex = Texture2D::from_image(&terrain_raster.img);
    terrain_tex.set_filter(FilterMode::Nearest);

    let mut lighting = LightingPipeline::new(screen_width().max(1.0), screen_height().max(1.0));

    let mut cam_center = village_center_px(&world);
    let mut cam_zoom: f32 = 2.0;
    let mut drag_anchor: Option<Vec2> = None;
    let mut zoom_wheel_accum: f32 = 0.0;

    let mut speed = SpeedMode::Normal;
    let mut anim_time: f32 = 0.0;
    let mut ui = UiState::default();

    loop {
        let frame_dt = get_frame_time().min(0.1);
        anim_time += frame_dt;

        let sw = screen_width().max(1.0);
        let sh = screen_height().max(1.0);
        lighting.ensure_size(sw, sh);

        let mouse = Vec2::from(mouse_position());
        let hud_snap =
            compute_hud_layout(sw, sh, get_fps(), &world, &agents, &ui, &measure_ui_width);
        let over_ui = hud_snap.contains_point(mouse) || ui.controls_open;

        // Keyboard shortcuts
        if is_key_pressed(KeyCode::Escape) {
            if ui.controls_open {
                ui.controls_open = false;
            } else if ui.menu_open {
                ui.menu_open = false;
            } else if ui.wand_open {
                ui.wand_open = false;
            }
        }
        if is_key_pressed(KeyCode::F3) {
            ui.stats_open = !ui.stats_open;
        }
        let shift_down = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        if (is_key_pressed(KeyCode::Slash) && shift_down) || is_key_pressed(KeyCode::F1) {
            ui.controls_open = !ui.controls_open;
        }
        if is_key_pressed(KeyCode::LeftBracket) {
            ui.brush_radius = (ui.brush_radius - 1).clamp(1, 8);
        }
        if is_key_pressed(KeyCode::RightBracket) {
            ui.brush_radius = (ui.brush_radius + 1).clamp(1, 8);
        }
        if is_key_pressed(KeyCode::Tab) {
            let active = agents.toggle_mage_mode();
            if active {
                cam_center = agents.mage.pos;
            }
        }
        if is_key_pressed(KeyCode::Home) {
            cam_center = village_center_px(&world);
        }
        if is_key_pressed(KeyCode::F) {
            cam_zoom = if ui.snap_zoom {
                fit_world_zoom_snapped(sw, sh)
            } else {
                fit_world_zoom(sw, sh)
            };
            cam_center = vec2(WORLD_W_PX * 0.5, WORLD_H_PX * 0.5);
        }
        if is_key_pressed(KeyCode::B) {
            ui.wand_open = !ui.wand_open;
        }
        if is_key_pressed(KeyCode::R) {
            seed = seed.wrapping_add(1);
            let keep_mage = agents.mage_active;
            (world, agents, sim_accum, shake_trauma) =
                reset_world_and_agents(preset, seed, keep_mage);
            terrain_raster.invalidate_all();
            cam_center = if keep_mage {
                agents.mage.pos
            } else {
                village_center_px(&world)
            };
        }
        if is_key_pressed(KeyCode::P) {
            preset = preset.next();
            let keep_mage = agents.mage_active;
            (world, agents, sim_accum, shake_trauma) =
                reset_world_and_agents(preset, seed, keep_mage);
            terrain_raster.invalidate_all();
            cam_center = if keep_mage {
                agents.mage.pos
            } else {
                village_center_px(&world)
            };
        }
        if is_key_pressed(KeyCode::V) {
            world.cycle_weather();
        }
        if is_key_pressed(KeyCode::T) {
            world.advance_time_phase();
        }
        if !agents.mage_active && is_key_pressed(KeyCode::Space) {
            if speed == SpeedMode::Pause {
                speed = ui.last_run_speed;
            } else {
                ui.last_run_speed = speed;
                speed = SpeedMode::Pause;
            }
        }
        if is_key_pressed(KeyCode::Key1) {
            speed = SpeedMode::Pause;
        }
        if is_key_pressed(KeyCode::Key2) {
            speed = SpeedMode::Normal;
            ui.last_run_speed = SpeedMode::Normal;
        }
        if is_key_pressed(KeyCode::Key3) {
            speed = SpeedMode::Fast;
            ui.last_run_speed = SpeedMode::Fast;
        }
        if is_key_pressed(KeyCode::Key4) {
            speed = SpeedMode::Turbo;
            ui.last_run_speed = SpeedMode::Turbo;
        }

        // Gather directional input (WASD / Arrows)
        let mut move_axis = Vec2::ZERO;
        if is_key_down(KeyCode::W) || is_key_down(KeyCode::Up) {
            move_axis.y -= 1.0;
        }
        if is_key_down(KeyCode::S) || is_key_down(KeyCode::Down) {
            move_axis.y += 1.0;
        }
        if is_key_down(KeyCode::A) || is_key_down(KeyCode::Left) {
            move_axis.x -= 1.0;
        }
        if is_key_down(KeyCode::D) || is_key_down(KeyCode::Right) {
            move_axis.x += 1.0;
        }

        // Camera control: God Cursor pans; Mage Mode follows Mage smoothly
        if !agents.mage_active {
            let pan_speed = 360.0 / cam_zoom.max(0.5);
            if move_axis != Vec2::ZERO {
                cam_center += move_axis.normalize() * pan_speed * frame_dt;
            }
            if !over_ui && is_mouse_button_pressed(MouseButton::Right) {
                drag_anchor = Some(mouse);
            }
            if is_mouse_button_down(MouseButton::Right) {
                if let Some(prev) = drag_anchor {
                    let delta = mouse - prev;
                    cam_center -= delta / cam_zoom;
                    drag_anchor = Some(mouse);
                }
            } else {
                drag_anchor = None;
            }
        } else {
            drag_anchor = None;
        }

        // Wheel: Shift+Wheel resizes brush; Wheel zooms toward cursor (0.25x snap when enabled)
        let min_zoom = if ui.snap_zoom {
            fit_world_zoom_snapped(sw, sh)
        } else {
            (fit_world_zoom(sw, sh) * 0.85).clamp(0.15, 0.5)
        };
        let (_, wheel_y) = mouse_wheel();
        if wheel_y.abs() > 0.01 {
            if shift_down && !agents.mage_active {
                let delta = if wheel_y > 0.0 { 1 } else { -1 };
                ui.brush_radius = (ui.brush_radius + delta).clamp(1, 8);
            } else if !over_ui {
                let cam_before = make_camera(cam_center, cam_zoom, sw, sh);
                let world_before = cam_before.screen_to_world(mouse);
                if ui.snap_zoom {
                    zoom_wheel_accum += wheel_y;
                    let steps = if zoom_wheel_accum.abs() >= 0.5 {
                        let s = zoom_wheel_accum.signum() as i32;
                        zoom_wheel_accum = 0.0;
                        s
                    } else {
                        0
                    };
                    if steps != 0 {
                        cam_zoom = snap_zoom_step(cam_zoom, steps, min_zoom);
                    }
                } else {
                    zoom_wheel_accum = 0.0;
                    let factor = if wheel_y > 0.0 { 1.12 } else { 1.0 / 1.12 };
                    cam_zoom = (cam_zoom * factor).clamp(min_zoom, 6.0);
                }
                if !agents.mage_active {
                    let cam_after = make_camera(cam_center, cam_zoom, sw, sh);
                    let world_after = cam_after.screen_to_world(mouse);
                    cam_center += world_before - world_after;
                }
            }
        } else {
            zoom_wheel_accum *= 0.8;
        }

        cam_center = clamp_camera_center(cam_center, cam_zoom, sw, sh);

        let camera = make_camera(cam_center, cam_zoom, sw, sh);
        let mouse_world = camera.screen_to_world(mouse);
        let hover_tx = (mouse_world.x / TILE_PX).floor() as i32;
        let hover_ty = (mouse_world.y / TILE_PX).floor() as i32;

        // World actions: Mage controls/casting OR God Cursor painting/spawning
        let active_tool = ui.selected_tool;
        if agents.mage_active {
            let mage_cmds = agents.drive_mage(
                &world,
                MageInput {
                    move_axis,
                    aim_world: mouse_world,
                    dash_pressed: is_key_pressed(KeyCode::Space),
                    cast_down: !over_ui && is_mouse_button_down(MouseButton::Left),
                },
                frame_dt,
            );
            apply_world_cmds(&mut world, mage_cmds);
            cam_center = cam_center.lerp(agents.mage.pos, (frame_dt * 12.0).min(1.0));
            cam_center = clamp_camera_center(cam_center, cam_zoom, sw, sh);
        } else if !over_ui && World::in_bounds(hover_tx, hover_ty) {
            let should_apply = if active_tool.is_single_point() {
                is_mouse_button_pressed(MouseButton::Left)
            } else {
                is_mouse_button_down(MouseButton::Left)
            };

            if should_apply {
                match active_tool {
                    BrushTool::Element(mat) => {
                        let cmds = paint_circle_cmds(hover_tx, hover_ty, ui.brush_radius, mat);
                        apply_world_cmds(&mut world, cmds);
                    }
                    BrushTool::AddWater => {
                        let mut cmds = Vec::new();
                        let r = ui.brush_radius - 1;
                        let r_sq = r * r;
                        for dy in -r..=r {
                            for dx in -r..=r {
                                if dx * dx + dy * dy <= r_sq {
                                    cmds.push(WorldCmd::AddWater {
                                        x: hover_tx + dx,
                                        y: hover_ty + dy,
                                        amount: 30,
                                    });
                                }
                            }
                        }
                        apply_world_cmds(&mut world, cmds);
                    }
                    BrushTool::SpawnVillager => {
                        agents.spawn_villager(mouse_world);
                    }
                    BrushTool::SpawnRabbit => {
                        agents.spawn_critter(CritterKind::Rabbit, mouse_world);
                    }
                    BrushTool::SpawnWolf => {
                        agents.spawn_critter(CritterKind::Wolf, mouse_world);
                    }
                    BrushTool::ThrowBomb => {
                        let origin = cam_center;
                        let dir = (mouse_world - origin).normalize_or_zero();
                        let dist = mouse_world.distance(origin).max(16.0);
                        let speed_px = 360.0;
                        let ttl = (dist / speed_px).clamp(0.12, 1.5);
                        let vel = if dir == Vec2::ZERO {
                            Vec2::new(0.0, 1.0)
                        } else {
                            dir * speed_px
                        };
                        agents.spawn_projectile(
                            ProjectileKind::Bomb,
                            origin,
                            vel,
                            ttl,
                            ui.brush_radius + 2,
                            75.0,
                        );
                    }
                }
            }
        }

        // Fixed 60 Hz sim loop with accumulator (max 5 steps per frame)
        let mult = speed.multiplier();
        if mult > 0 {
            sim_accum += frame_dt * mult as f32;
            let mut steps = 0;
            while sim_accum >= SIM_DT && steps < 5 {
                world.step();
                let cmds = agents.update(&world, SIM_DT);
                apply_world_cmds(&mut world, cmds);
                sim_accum -= SIM_DT;
                steps += 1;
            }
            if steps == 5 && sim_accum > SIM_DT {
                sim_accum = SIM_DT;
            }
        }

        // Decaying camera shake from explosions & gunpowder chains
        shake_trauma = (shake_trauma + world.take_shake_impulse()).clamp(0.0, 1.0);
        shake_trauma = (shake_trauma - frame_dt * SHAKE_DECAY).max(0.0);
        let shake_amp = shake_trauma * shake_trauma * (MAX_SHAKE_PX / cam_zoom.sqrt());
        let shake_offset = if shake_amp > 0.05 {
            vec2(
                (anim_time * 57.0).sin() * shake_amp,
                (anim_time * 71.0).cos() * shake_amp,
            )
        } else {
            Vec2::ZERO
        };
        let render_center = cam_center + shake_offset;

        let (view_min, view_max) = visible_tile_bounds(render_center, cam_zoom, sw, sh);
        let anim_tick = (anim_time * 6.0) as usize;

        // Dirty-chunk + 10 Hz animated-cell rasterization into single nearest-filtered texture
        if terrain_raster.update(&world, anim_time) {
            terrain_tex.update(&terrain_raster.img);
        }

        let ambient = ambient_light_color(world.time_of_day, world.weather);
        let night_rim = night_rim_strength(ambient);
        let darkness = (1.0 - (ambient.r + ambient.g + ambient.b) / 3.0).clamp(0.0, 1.0);

        let (firefly_dots, glows) = collect_scene_glows_and_fireflies(
            &world,
            &agents,
            view_min,
            view_max,
            render_center,
            anim_time,
        );

        // Pass 1: Render scene into offscreen scene_rt
        let mut scene_cam = make_camera(render_center, cam_zoom, sw, sh);
        scene_cam.render_target = Some(lighting.scene_rt.clone());
        set_camera(&scene_cam);
        clear_background(UI_BASE_BG);

        draw_texture_ex(
            &terrain_tex,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(WORLD_W_PX, WORLD_H_PX)),
                ..Default::default()
            },
        );
        agents.draw_sorted(
            &world,
            DrawSceneOpts {
                view_min,
                view_max,
                anim_tick,
                cam_zoom,
                night_rim,
            },
        );
        draw_gas_particles(&world, view_min, view_max);
        draw_firefly_dots(&firefly_dots);

        if !over_ui && World::in_bounds(hover_tx, hover_ty) {
            if agents.mage_active {
                draw_aim_reticle(mouse_world, agents.mage.wand.payload.color());
            } else {
                draw_brush_preview(hover_tx, hover_ty, ui.brush_radius, active_tool);
            }
        }

        // Pass 2: Build dynamic lightmap in lightmap_rt (ambient time-of-day + additive radial glows)
        let mut light_cam = make_camera(render_center, cam_zoom, sw, sh);
        light_cam.render_target = Some(lighting.lightmap_rt.clone());
        set_camera(&light_cam);
        clear_background(ambient);

        if let Some(glow_mat) = &lighting.radial_glow_mat {
            gl_use_material(glow_mat);
            for g in &glows {
                draw_rectangle(
                    g.pos.x - g.radius,
                    g.pos.y - g.radius,
                    g.radius * 2.0,
                    g.radius * 2.0,
                    g.color,
                );
            }
            gl_use_default_material();
        }

        // Pass 3: Composite scene * lightmap (multiply), then additive radial glow pass
        set_default_camera();
        clear_background(UI_BASE_BG);
        draw_texture_ex(
            &lighting.scene_rt.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(sw, sh)),
                ..Default::default()
            },
        );

        if let Some(mul_mat) = &lighting.multiply_mat {
            gl_use_material(mul_mat);
            draw_texture_ex(
                &lighting.lightmap_rt.texture,
                0.0,
                0.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(sw, sh)),
                    ..Default::default()
                },
            );
            gl_use_default_material();
        }

        let screen_world_cam = make_camera(render_center, cam_zoom, sw, sh);
        if let Some(glow_mat) = &lighting.radial_glow_mat {
            let bloom_scale = 0.45 + 0.55 * darkness;
            set_camera(&screen_world_cam);
            gl_use_material(glow_mat);
            for g in &glows {
                let bloom_r = g.radius * 0.72;
                let bloom_col = Color::new(
                    g.color.r,
                    g.color.g,
                    g.color.b,
                    (g.color.a * g.bloom_alpha * bloom_scale).clamp(0.0, 1.0),
                );
                draw_rectangle(
                    g.pos.x - bloom_r,
                    g.pos.y - bloom_r,
                    bloom_r * 2.0,
                    bloom_r * 2.0,
                    bloom_col,
                );
            }
            gl_use_default_material();
        }

        // Mask any area outside the world rect with UI base color (#1e1e2e)
        set_default_camera();
        draw_world_letterbox(&screen_world_cam, sw, sh);

        // Unlit screen-space weather & HUD
        draw_weather_overlay(&world, sw, sh, 0.0, 0.0, anim_time);

        let actions = draw_hud(&hud_snap, &mut speed, &world, &mut agents, &mut ui);
        for action in actions {
            match action {
                UiAction::SelectPreset(chosen) => {
                    preset = chosen;
                    let keep_mage = agents.mage_active;
                    (world, agents, sim_accum, shake_trauma) =
                        reset_world_and_agents(preset, seed, keep_mage);
                    terrain_raster.invalidate_all();
                    cam_center = if keep_mage {
                        agents.mage.pos
                    } else {
                        village_center_px(&world)
                    };
                }
                UiAction::CycleWeather => {
                    world.cycle_weather();
                }
                UiAction::AdvanceTime => {
                    world.advance_time_phase();
                }
                UiAction::Reset => {
                    seed = seed.wrapping_add(1);
                    let keep_mage = agents.mage_active;
                    (world, agents, sim_accum, shake_trauma) =
                        reset_world_and_agents(preset, seed, keep_mage);
                    terrain_raster.invalidate_all();
                    cam_center = if keep_mage {
                        agents.mage.pos
                    } else {
                        village_center_px(&world)
                    };
                }
                UiAction::ToggleMode => {
                    if agents.toggle_mage_mode() {
                        cam_center = agents.mage.pos;
                    }
                }
                UiAction::ToggleWandBuilder => {
                    ui.wand_open = !ui.wand_open;
                }
                UiAction::ToggleSnapZoom => {
                    ui.snap_zoom = !ui.snap_zoom;
                    if ui.snap_zoom {
                        let min_z = fit_world_zoom_snapped(sw, sh);
                        cam_zoom = snap_zoom_nearest(cam_zoom, min_z);
                    }
                }
                UiAction::SelectSpell(payload) => {
                    agents.mage.wand.payload = payload;
                    if !agents.mage_active {
                        agents.toggle_mage_mode();
                        cam_center = agents.mage.pos;
                    }
                }
            }
        }

        if !over_ui && !ui.controls_open && World::in_bounds(hover_tx, hover_ty) {
            draw_cursor_tooltip(
                mouse,
                vec2(sw, sh),
                &world,
                &agents,
                mouse_world,
                (hover_tx, hover_ty),
            );
        }

        next_frame().await;
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_night_fit_zoom_bench() {
    use render::{collect_scene_glows_and_fireflies_with_cap, LIGHTMAP_DIVISOR, MAX_GLOW_LIGHTS};
    use std::time::Instant;

    let presets = [
        Preset::LushRiverIsland,
        Preset::VolcanicCrater,
        Preset::DroughtValley,
    ];
    let configs = [
        ("BEFORE (1x lightmap, uncapped glows)", 1_u32, usize::MAX),
        (
            "AFTER  (1/4 lightmap, cap 64 glows)",
            LIGHTMAP_DIVISOR,
            MAX_GLOW_LIGHTS,
        ),
    ];

    // Warm up window/GL context for 10 frames
    for _ in 0..10 {
        clear_background(UI_BASE_BG);
        next_frame().await;
    }

    for &preset in &presets {
        eprintln!("=== Night Fit-Zoom Benchmark: {} ===", preset.name());
        for &(label, div, glow_cap) in &configs {
            let (mut world, mut agents, _, _) = reset_world_and_agents(preset, 42, false);
            world.time_of_day = 0.90; // Night
            for _ in 0..120 {
                world.step();
                let cmds = agents.update(&world, SIM_DT);
                apply_world_cmds(&mut world, cmds);
            }

            let sw = screen_width().max(1.0);
            let sh = screen_height().max(1.0);
            let mut terrain_raster = TerrainRasterizer::new(&world, 0.0);
            let terrain_tex = Texture2D::from_image(&terrain_raster.img);
            terrain_tex.set_filter(FilterMode::Nearest);
            let mut lighting = LightingPipeline::new_with_divisor(sw, sh, div);

            let cam_zoom = fit_world_zoom_snapped(sw, sh);
            let cam_center =
                clamp_camera_center(vec2(WORLD_W_PX * 0.5, WORLD_H_PX * 0.5), cam_zoom, sw, sh);
            let mut speed = SpeedMode::Normal;
            let mut ui = UiState::default();

            let frames = 90_u32;
            let mut t_sim_raster_ms = 0.0_f64;
            let mut t_pass1_ms = 0.0_f64;
            let mut t_pass2_ms = 0.0_f64;
            let mut t_pass3_ms = 0.0_f64;
            let mut t_hud_ms = 0.0_f64;
            let mut glow_count_last = 0_usize;

            for f in 0..frames {
                let anim_time = f as f32 / 60.0;
                let sw = screen_width().max(1.0);
                let sh = screen_height().max(1.0);
                lighting.ensure_size(sw, sh);

                let t0 = Instant::now();
                world.step();
                world.time_of_day = 0.90;
                let cmds = agents.update(&world, SIM_DT);
                apply_world_cmds(&mut world, cmds);
                if terrain_raster.update(&world, anim_time) {
                    terrain_tex.update(&terrain_raster.img);
                }
                t_sim_raster_ms += t0.elapsed().as_secs_f64() * 1000.0;

                let (view_min, view_max) = visible_tile_bounds(cam_center, cam_zoom, sw, sh);
                let anim_tick = (anim_time * 6.0) as usize;
                let ambient = ambient_light_color(world.time_of_day, world.weather);
                let night_rim = night_rim_strength(ambient);
                let darkness = (1.0 - (ambient.r + ambient.g + ambient.b) / 3.0).clamp(0.0, 1.0);

                // Pass 1: Scene RT
                let t1 = Instant::now();
                let (firefly_dots, glows) = collect_scene_glows_and_fireflies_with_cap(
                    &world, &agents, view_min, view_max, cam_center, anim_time, glow_cap,
                );
                glow_count_last = glows.len();
                let mut scene_cam = make_camera(cam_center, cam_zoom, sw, sh);
                scene_cam.render_target = Some(lighting.scene_rt.clone());
                set_camera(&scene_cam);
                clear_background(UI_BASE_BG);
                draw_texture_ex(
                    &terrain_tex,
                    0.0,
                    0.0,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(WORLD_W_PX, WORLD_H_PX)),
                        ..Default::default()
                    },
                );
                agents.draw_sorted(
                    &world,
                    DrawSceneOpts {
                        view_min,
                        view_max,
                        anim_tick,
                        cam_zoom,
                        night_rim,
                    },
                );
                draw_gas_particles(&world, view_min, view_max);
                draw_firefly_dots(&firefly_dots);
                t_pass1_ms += t1.elapsed().as_secs_f64() * 1000.0;

                // Pass 2: Lightmap RT
                let t2 = Instant::now();
                let mut light_cam = make_camera(cam_center, cam_zoom, sw, sh);
                light_cam.render_target = Some(lighting.lightmap_rt.clone());
                set_camera(&light_cam);
                clear_background(ambient);
                if let Some(glow_mat) = &lighting.radial_glow_mat {
                    gl_use_material(glow_mat);
                    for g in &glows {
                        draw_rectangle(
                            g.pos.x - g.radius,
                            g.pos.y - g.radius,
                            g.radius * 2.0,
                            g.radius * 2.0,
                            g.color,
                        );
                    }
                    gl_use_default_material();
                }
                t_pass2_ms += t2.elapsed().as_secs_f64() * 1000.0;

                // Pass 3: Multiply composite + additive bloom + letterbox
                let t3 = Instant::now();
                set_default_camera();
                clear_background(UI_BASE_BG);
                draw_texture_ex(
                    &lighting.scene_rt.texture,
                    0.0,
                    0.0,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(sw, sh)),
                        ..Default::default()
                    },
                );
                if let Some(mul_mat) = &lighting.multiply_mat {
                    gl_use_material(mul_mat);
                    draw_texture_ex(
                        &lighting.lightmap_rt.texture,
                        0.0,
                        0.0,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(sw, sh)),
                            ..Default::default()
                        },
                    );
                    gl_use_default_material();
                }
                let screen_world_cam = make_camera(cam_center, cam_zoom, sw, sh);
                if let Some(glow_mat) = &lighting.radial_glow_mat {
                    let bloom_scale = 0.45 + 0.55 * darkness;
                    set_camera(&screen_world_cam);
                    gl_use_material(glow_mat);
                    for g in &glows {
                        let bloom_r = g.radius * 0.72;
                        let bloom_col = Color::new(
                            g.color.r,
                            g.color.g,
                            g.color.b,
                            (g.color.a * g.bloom_alpha * bloom_scale).clamp(0.0, 1.0),
                        );
                        draw_rectangle(
                            g.pos.x - bloom_r,
                            g.pos.y - bloom_r,
                            bloom_r * 2.0,
                            bloom_r * 2.0,
                            bloom_col,
                        );
                    }
                    gl_use_default_material();
                }
                set_default_camera();
                draw_world_letterbox(&screen_world_cam, sw, sh);
                t_pass3_ms += t3.elapsed().as_secs_f64() * 1000.0;

                // Pass 4: HUD + weather
                let t4 = Instant::now();
                draw_weather_overlay(&world, sw, sh, 0.0, 0.0, anim_time);
                let hud_snap =
                    compute_hud_layout(sw, sh, get_fps(), &world, &agents, &ui, &measure_ui_width);
                let _ = draw_hud(&hud_snap, &mut speed, &world, &mut agents, &mut ui);
                t_hud_ms += t4.elapsed().as_secs_f64() * 1000.0;

                next_frame().await;
            }

            let nf = frames as f64;
            let total_cpu_gl_ms =
                (t_sim_raster_ms + t_pass1_ms + t_pass2_ms + t_pass3_ms + t_hud_ms) / nf;
            eprintln!(
                "  {label}: glows={glow_count_last:3} | sim+raster={:.3}ms | pass1_scene={:.3}ms | pass2_lightmap={:.3}ms | pass3_composite+bloom={:.3}ms | hud={:.3}ms | render_passes_sum={:.3}ms | total={:.3}ms",
                t_sim_raster_ms / nf,
                t_pass1_ms / nf,
                t_pass2_ms / nf,
                t_pass3_ms / nf,
                t_hud_ms / nf,
                (t_pass1_ms + t_pass2_ms + t_pass3_ms) / nf,
                total_cpu_gl_ms,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Weather;

    #[test]
    fn day_night_ambient_lighting_curves() {
        let day = ambient_light_color(0.45, Weather::Clear);
        let night = ambient_light_color(0.90, Weather::Clear);
        let night_rain = ambient_light_color(0.90, Weather::Rainstorm);
        let dawn = ambient_light_color(0.24, Weather::Clear);
        let dusk = ambient_light_color(0.68, Weather::Clear);

        assert!(day.r > night.r && day.g > night.g);
        assert!(night.b > night.g && night.g > night.r);
        let night_luma = 0.2126 * night.r + 0.7152 * night.g + 0.0722 * night.b;
        let night_rain_luma = 0.2126 * night_rain.r + 0.7152 * night_rain.g + 0.0722 * night_rain.b;
        assert!(
            (0.30..=0.40).contains(&night_luma),
            "night luma {night_luma} should stay readable around 0.30-0.35"
        );
        assert!(night_rain_luma >= 0.30);
        assert!(night_rim_strength(night) > 0.7);
        assert!(night_rim_strength(day) < 0.01);
        assert!(dawn.r > night.r && dawn.r < day.r);
        assert!(dusk.r > dusk.b);

        let rain = ambient_light_color(0.45, Weather::Rainstorm);
        assert!(rain.r < day.r);
    }
}
