mod lighting;
mod terrain;
mod world_draw;

#[cfg(test)]
mod tests;

pub use lighting::{
    ambient_light_color, collect_scene_glows_and_fireflies, draw_firefly_dots, night_rim_strength,
    LightingPipeline,
};
#[cfg(not(target_arch = "wasm32"))]
pub use lighting::{collect_scene_glows_and_fireflies_with_cap, LIGHTMAP_DIVISOR, MAX_GLOW_LIGHTS};
pub use terrain::{TerrainRasterizer, MICRO_WORLD_PX};
pub use world_draw::{
    clamp_camera_center, draw_aim_reticle, draw_brush_preview, draw_gas_particles,
    draw_weather_overlay, draw_world_letterbox, fit_world_zoom, fit_world_zoom_snapped,
    make_camera, snap_zoom_nearest, snap_zoom_step, visible_tile_bounds, UI_BASE_BG, WORLD_H_PX,
    WORLD_W_PX,
};
