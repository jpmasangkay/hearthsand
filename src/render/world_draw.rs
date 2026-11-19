use macroquad::prelude::*;

use crate::ui::BrushTool;
use crate::world::{GasKind, Weather, World, GRID_H, GRID_W, TILE_PX};

use super::terrain::MICRO_WORLD_PX;

/// UI base background color (#1e1e2e), also used for world letterbox margins.
pub const UI_BASE_BG: Color = Color::from_rgba(30, 30, 46, 255);

pub const WORLD_W_PX: f32 = GRID_W as f32 * TILE_PX;
pub const WORLD_H_PX: f32 = GRID_H as f32 * TILE_PX;

pub fn make_camera(center: Vec2, zoom: f32, sw: f32, sh: f32) -> Camera2D {
    Camera2D {
        target: center,
        zoom: vec2(2.0 * zoom / sw, 2.0 * zoom / sh),
        ..Default::default()
    }
}

pub const ZOOM_SNAP_STEP: f32 = 0.25;
pub const MIN_SNAPPED_ZOOM: f32 = 0.25;
pub const MAX_ZOOM: f32 = 6.0;

pub fn fit_world_zoom(sw: f32, sh: f32) -> f32 {
    let zx = sw.max(1.0) / WORLD_W_PX;
    let zy = sh.max(1.0) / WORLD_H_PX;
    zx.min(zy).clamp(0.15, MAX_ZOOM)
}

/// Largest multiple of `0.25` that fits the full world inside `(sw, sh)` (at least `0.25`).
/// Because `MICRO_WORLD_PX == 4.0`, every `0.25` zoom multiple maps 1 micro-pixel to an exact
/// integer number of logical pixels (`4 * zoom`), avoiding uneven pixel columns/rows.
pub fn fit_world_zoom_snapped(sw: f32, sh: f32) -> f32 {
    let raw = fit_world_zoom(sw, sh);
    let steps = (raw / ZOOM_SNAP_STEP + 1e-4).floor().max(1.0);
    (steps * ZOOM_SNAP_STEP).clamp(MIN_SNAPPED_ZOOM, MAX_ZOOM)
}

pub fn snap_zoom_nearest(zoom: f32, min_zoom: f32) -> f32 {
    let min_snapped =
        ((min_zoom / ZOOM_SNAP_STEP).ceil() * ZOOM_SNAP_STEP).clamp(MIN_SNAPPED_ZOOM, MAX_ZOOM);
    let snapped = (zoom / ZOOM_SNAP_STEP).round() * ZOOM_SNAP_STEP;
    snapped.clamp(min_snapped, MAX_ZOOM)
}

pub fn snap_zoom_step(zoom: f32, delta_steps: i32, min_zoom: f32) -> f32 {
    let min_steps = (min_zoom / ZOOM_SNAP_STEP).ceil().max(1.0) as i32;
    let max_steps = (MAX_ZOOM / ZOOM_SNAP_STEP).round() as i32;
    let cur_steps = (zoom / ZOOM_SNAP_STEP).round() as i32;
    let next_steps = (cur_steps + delta_steps).clamp(min_steps, max_steps);
    next_steps as f32 * ZOOM_SNAP_STEP
}

pub fn clamp_camera_center(center: Vec2, zoom: f32, sw: f32, sh: f32) -> Vec2 {
    let z = zoom.max(0.05);
    let view_w = sw / z;
    let view_h = sh / z;

    let x = if view_w >= WORLD_W_PX {
        WORLD_W_PX * 0.5
    } else {
        let half = view_w * 0.5;
        center.x.clamp(half, WORLD_W_PX - half)
    };
    let y = if view_h >= WORLD_H_PX {
        WORLD_H_PX * 0.5
    } else {
        let half = view_h * 0.5;
        center.y.clamp(half, WORLD_H_PX - half)
    };

    vec2(x, y)
}

pub fn draw_world_letterbox(camera: &Camera2D, sw: f32, sh: f32) {
    let tl = camera.world_to_screen(Vec2::ZERO);
    let br = camera.world_to_screen(vec2(WORLD_W_PX, WORLD_H_PX));
    let left = tl.x.min(br.x);
    let right = tl.x.max(br.x);
    let top = tl.y.min(br.y);
    let bottom = tl.y.max(br.y);

    if left > 0.0 {
        draw_rectangle(0.0, 0.0, left.ceil(), sh, UI_BASE_BG);
    }
    if right < sw {
        draw_rectangle(
            right.floor(),
            0.0,
            (sw - right).ceil() + 1.0,
            sh,
            UI_BASE_BG,
        );
    }
    if top > 0.0 {
        draw_rectangle(0.0, 0.0, sw, top.ceil(), UI_BASE_BG);
    }
    if bottom < sh {
        draw_rectangle(
            0.0,
            bottom.floor(),
            sw,
            (sh - bottom).ceil() + 1.0,
            UI_BASE_BG,
        );
    }
}

pub fn visible_tile_bounds(
    center: Vec2,
    zoom: f32,
    sw: f32,
    sh: f32,
) -> ((usize, usize), (usize, usize)) {
    let half_w = sw / (2.0 * zoom);
    let half_h = sh / (2.0 * zoom);
    let min_x = ((center.x - half_w) / TILE_PX).floor().max(0.0) as usize;
    let min_y = ((center.y - half_h) / TILE_PX).floor().max(0.0) as usize;
    let max_x = (((center.x + half_w) / TILE_PX).ceil() as usize + 1).min(GRID_W);
    let max_y = (((center.y + half_h) / TILE_PX).ceil() as usize + 1).min(GRID_H);
    ((min_x, min_y), (max_x, max_y))
}

pub fn draw_brush_preview(cx: i32, cy: i32, radius: i32, tool: BrushTool) {
    if tool.is_single_point() {
        draw_rectangle_lines(
            cx as f32 * TILE_PX,
            cy as f32 * TILE_PX,
            TILE_PX,
            TILE_PX,
            1.5,
            Color::from_rgba(255, 224, 96, 220),
        );
        return;
    }

    let r = radius - 1;
    let r_sq = r * r;
    let color = Color::from_rgba(255, 240, 140, 175);
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r_sq {
                let x = cx + dx;
                let y = cy + dy;
                if World::in_bounds(x, y) {
                    draw_rectangle_lines(
                        x as f32 * TILE_PX,
                        y as f32 * TILE_PX,
                        TILE_PX,
                        TILE_PX,
                        1.0,
                        color,
                    );
                }
            }
        }
    }
}

pub fn draw_gas_particles(world: &World, view_min: (usize, usize), view_max: (usize, usize)) {
    let min_x = view_min.0 as f32 * TILE_PX - 24.0;
    let max_x = view_max.0 as f32 * TILE_PX + 24.0;
    let min_y = view_min.1 as f32 * TILE_PX - 24.0;
    let max_y = view_max.1 as f32 * TILE_PX + 48.0;
    let m = MICRO_WORLD_PX;

    for g in &world.gas {
        if g.pos.x < min_x || g.pos.x > max_x || g.pos.y < min_y || g.pos.y > max_y {
            continue;
        }
        let fade = (g.ttl / 1.5).clamp(0.18, 1.0);
        let alpha = (fade * 170.0) as u8;
        let gx = (g.pos.x / m).floor() * m;
        let gy = ((g.pos.y - g.height) / m).floor() * m;
        let (core, halo) = match g.kind {
            GasKind::Smoke => (
                Color::from_rgba(148, 150, 160, alpha),
                Color::from_rgba(116, 118, 128, alpha / 2),
            ),
            GasKind::Steam => (
                Color::from_rgba(232, 242, 252, alpha),
                Color::from_rgba(202, 218, 236, alpha / 2),
            ),
            GasKind::AcidGas => (
                Color::from_rgba(132, 244, 72, alpha),
                Color::from_rgba(88, 198, 44, alpha / 2),
            ),
        };
        draw_rectangle(gx - m, gy, m * 3.0, m, halo);
        draw_rectangle(gx, gy - m, m * 2.0, m * 2.0, core);
    }
}

pub fn draw_aim_reticle(world_pos: Vec2, color: Color) {
    draw_circle_lines(world_pos.x, world_pos.y, 7.0, 1.5, color);
    draw_line(
        world_pos.x - 4.0,
        world_pos.y,
        world_pos.x + 4.0,
        world_pos.y,
        1.0,
        color,
    );
    draw_line(
        world_pos.x,
        world_pos.y - 4.0,
        world_pos.x,
        world_pos.y + 4.0,
        1.0,
        color,
    );
}

pub fn draw_weather_overlay(
    world: &World,
    sw: f32,
    sh: f32,
    top_bar_h: f32,
    palette_h: f32,
    anim_time: f32,
) {
    let view_h = (sh - top_bar_h - palette_h).max(1.0);
    match world.weather {
        Weather::Clear => {}
        Weather::Rainstorm => {
            draw_rectangle(
                0.0,
                top_bar_h,
                sw,
                view_h,
                Color::from_rgba(32, 64, 118, 34),
            );
            let wind_tilt = world.wind.x * 0.45;
            let drop_color = Color::from_rgba(168, 214, 255, 130);
            for i in 0..85 {
                let fi = i as f32;
                let base_x = (fi * 137.5) % sw;
                let base_y = (fi * 91.3) % view_h;
                let x = (base_x + anim_time * (140.0 + world.wind.x * 4.0)).rem_euclid(sw);
                let y = top_bar_h + (base_y + anim_time * 420.0).rem_euclid(view_h);
                draw_line(x, y, x + wind_tilt * 0.35, y + 11.0, 1.0, drop_color);
            }
        }
        Weather::Heatwave => {
            let pulse = (anim_time * 2.5).sin() * 6.0;
            let alpha = (26.0 + pulse).clamp(16.0, 38.0) as u8;
            draw_rectangle(
                0.0,
                top_bar_h,
                sw,
                view_h,
                Color::from_rgba(255, 140, 42, alpha),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_clamp_keeps_inside_world_and_centers_when_smaller() {
        // Zoomed in (view 640x400 < world 2560x1600): clamp to [320..2240] x [200..1400]
        let clamped_tl = clamp_camera_center(vec2(-100.0, -50.0), 2.0, 1280.0, 800.0);
        assert!((clamped_tl.x - 320.0).abs() < 0.01);
        assert!((clamped_tl.y - 200.0).abs() < 0.01);

        let clamped_br = clamp_camera_center(vec2(5000.0, 4000.0), 2.0, 1280.0, 800.0);
        assert!((clamped_br.x - (WORLD_W_PX - 320.0)).abs() < 0.01);
        assert!((clamped_br.y - (WORLD_H_PX - 200.0)).abs() < 0.01);

        // Zoomed out smaller than viewport (view 3200x2000 > world 2560x1600): centered
        let centered = clamp_camera_center(vec2(100.0, 100.0), 0.4, 1280.0, 800.0);
        assert!((centered.x - WORLD_W_PX * 0.5).abs() < 0.01);
        assert!((centered.y - WORLD_H_PX * 0.5).abs() < 0.01);

        // Fit-world zoom fits both dimensions cleanly
        let fit = fit_world_zoom(960.0, 600.0);
        assert!((fit - 0.375).abs() < 0.001);
        assert!((fit_world_zoom_snapped(960.0, 600.0) - 0.25).abs() < 0.001);
        assert!((fit_world_zoom_snapped(1280.0, 800.0) - 0.50).abs() < 0.001);
    }

    #[test]
    fn zoom_snaps_to_quarter_steps_within_bounds() {
        assert!((snap_zoom_step(2.0, 1, 0.25) - 2.25).abs() < 1e-4);
        assert!((snap_zoom_step(2.0, -1, 0.25) - 1.75).abs() < 1e-4);
        assert!((snap_zoom_step(0.25, -3, 0.25) - 0.25).abs() < 1e-4);
        assert!((snap_zoom_step(6.0, 2, 0.25) - 6.0).abs() < 1e-4);
        assert!((snap_zoom_nearest(1.62, 0.25) - 1.5).abs() < 1e-4);
        assert!((snap_zoom_nearest(1.63, 0.25) - 1.75).abs() < 1e-4);
    }
}
