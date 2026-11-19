use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams};
use macroquad::prelude::*;

use crate::agents::{self, Agents, GlowLight};
use crate::world::{Material, Weather, World, TILE_PX};

use super::terrain::hash2;

const LIGHT_VERTEX_SHADER: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec2 uv;
varying lowp vec4 color;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}
"#;

const MULTIPLY_FRAGMENT_SHADER: &str = r#"#version 100
precision mediump float;
varying lowp vec4 color;
varying lowp vec2 uv;

uniform sampler2D Texture;

void main() {
    vec3 light = max(color.rgb * texture2D(Texture, uv).rgb, vec3(0.28, 0.31, 0.48));
    gl_FragColor = vec4(light, 1.0);
}
"#;

/// Merged lightmap multiply + additive radial bloom shader (M8.2):
/// Used with BlendState(Equation::Add, SourceAlpha, SourceColor) so hardware blending computes:
///   out.rgb = dst.rgb * light + light * (color.a * tex.a)
/// where `tex.a` holds the radial bloom intensity accumulated in `lightmap_rt` and `color.a`
/// is 1.0 when bloom is enabled (F5 OFF) or 0.0 when bloom is skipped (F5 ON).
const COMPOSITE_BLOOM_FRAGMENT_SHADER: &str = r#"#version 100
precision mediump float;
varying lowp vec4 color;
varying lowp vec2 uv;

uniform sampler2D Texture;

void main() {
    vec4 tex = texture2D(Texture, uv);
    vec3 light = max(color.rgb * tex.rgb, vec3(0.28, 0.31, 0.48));
    float bloom_a = color.a * tex.a;
    gl_FragColor = vec4(light, bloom_a);
}
"#;

const RADIAL_GLOW_FRAGMENT_SHADER: &str = r#"#version 100
precision mediump float;
varying lowp vec4 color;
varying lowp vec2 uv;

void main() {
    vec2 d = uv - vec2(0.5, 0.5);
    float dist = clamp(length(d) * 2.0, 0.0, 1.0);
    float falloff = 1.0 - dist;
    float glow = falloff * falloff * (0.45 + 0.55 * falloff);
    gl_FragColor = vec4(color.rgb, color.a * glow);
}
"#;

/// Accumulates pre-multiplied RGB light and alpha bloom weight into `lightmap_rt` in one pass.
const RADIAL_GLOW_ACCUM_FRAGMENT_SHADER: &str = r#"#version 100
precision mediump float;
varying lowp vec4 color;
varying lowp vec2 uv;

void main() {
    vec2 d = uv - vec2(0.5, 0.5);
    float dist = clamp(length(d) * 2.0, 0.0, 1.0);
    float falloff = 1.0 - dist;
    float glow = falloff * falloff * (0.45 + 0.55 * falloff);
    gl_FragColor = color * glow;
}
"#;

/// Quarter-resolution divisor for the linear-filtered dynamic lightmap render target (M7.2).
pub const LIGHTMAP_DIVISOR: u32 = 4;

/// Maximum radial glow draws per frame (nearest ~64 to camera center, M7.2).
pub const MAX_GLOW_LIGHTS: usize = 64;

#[inline]
pub fn lightmap_dimensions(sw: f32, sh: f32, dpi: f32, divisor: u32) -> ((u32, u32), (u32, u32)) {
    let div = divisor.max(1);
    let w = (sw * dpi).round().max(1.0) as u32;
    let h = (sh * dpi).round().max(1.0) as u32;
    let lw = (w / div).max(1);
    let lh = (h / div).max(1);
    ((w, h), (lw, lh))
}

pub struct LightingPipeline {
    pub scene_rt: RenderTarget,
    pub lightmap_rt: RenderTarget,
    width: u32,
    height: u32,
    lightmap_divisor: u32,
    pub multiply_mat: Option<macroquad::material::Material>,
    pub composite_bloom_mat: Option<macroquad::material::Material>,
    pub radial_glow_mat: Option<macroquad::material::Material>,
    pub radial_glow_accum_mat: Option<macroquad::material::Material>,
}

impl LightingPipeline {
    pub fn new(sw: f32, sh: f32) -> Self {
        Self::new_with_divisor(sw, sh, LIGHTMAP_DIVISOR)
    }

    pub fn new_with_divisor(sw: f32, sh: f32, divisor: u32) -> Self {
        let dpi = screen_dpi_scale();
        let ((w, h), (lw, lh)) = lightmap_dimensions(sw, sh, dpi, divisor);
        let scene_rt = render_target(w, h);
        scene_rt.texture.set_filter(FilterMode::Nearest);
        let lightmap_rt = render_target(lw, lh);
        lightmap_rt.texture.set_filter(FilterMode::Linear);

        let multiply_mat = load_material(
            ShaderSource::Glsl {
                vertex: LIGHT_VERTEX_SHADER,
                fragment: MULTIPLY_FRAGMENT_SHADER,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Zero,
                        BlendFactor::Value(BlendValue::SourceColor),
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .ok();

        let composite_bloom_mat = load_material(
            ShaderSource::Glsl {
                vertex: LIGHT_VERTEX_SHADER,
                fragment: COMPOSITE_BLOOM_FRAGMENT_SHADER,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::Value(BlendValue::SourceColor),
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .ok();

        let radial_glow_mat = load_material(
            ShaderSource::Glsl {
                vertex: LIGHT_VERTEX_SHADER,
                fragment: RADIAL_GLOW_FRAGMENT_SHADER,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::One,
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .ok();

        let radial_glow_accum_mat = load_material(
            ShaderSource::Glsl {
                vertex: LIGHT_VERTEX_SHADER,
                fragment: RADIAL_GLOW_ACCUM_FRAGMENT_SHADER,
            },
            MaterialParams {
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::One,
                    )),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .ok();

        Self {
            scene_rt,
            lightmap_rt,
            width: w,
            height: h,
            lightmap_divisor: divisor.max(1),
            multiply_mat,
            composite_bloom_mat,
            radial_glow_mat,
            radial_glow_accum_mat,
        }
    }

    pub fn ensure_size(&mut self, sw: f32, sh: f32) {
        let dpi = screen_dpi_scale();
        let ((w, h), (lw, lh)) = lightmap_dimensions(sw, sh, dpi, self.lightmap_divisor);
        if w != self.width || h != self.height {
            self.scene_rt = render_target(w, h);
            self.scene_rt.texture.set_filter(FilterMode::Nearest);
            self.lightmap_rt = render_target(lw, lh);
            self.lightmap_rt.texture.set_filter(FilterMode::Linear);
            self.width = w;
            self.height = h;
        }
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        1.0,
    )
}

pub fn ambient_light_color(time_of_day: f32, weather: Weather) -> Color {
    let t = time_of_day.rem_euclid(1.0);
    // Minimum night ambient ~0.30-0.35 perceived luminance with a cool blue tint
    let night = Color::new(0.30, 0.34, 0.52, 1.0);
    let dawn = Color::new(0.76, 0.62, 0.70, 1.0);
    let day = Color::new(1.0, 0.99, 0.96, 1.0);
    let dusk = Color::new(0.80, 0.58, 0.50, 1.0);

    let base = if t < 0.18 {
        night
    } else if t < 0.24 {
        lerp_color(night, dawn, (t - 0.18) / 0.06)
    } else if t < 0.30 {
        lerp_color(dawn, day, (t - 0.24) / 0.06)
    } else if t < 0.62 {
        day
    } else if t < 0.68 {
        lerp_color(day, dusk, (t - 0.62) / 0.06)
    } else if t < 0.74 {
        lerp_color(dusk, night, (t - 0.68) / 0.06)
    } else {
        night
    };

    match weather {
        Weather::Clear => base,
        Weather::Rainstorm => Color::new(
            (base.r * 0.86).max(0.28),
            (base.g * 0.90).max(0.31),
            (base.b * 0.96).max(0.48),
            1.0,
        ),
        Weather::Heatwave => Color::new(
            (base.r * 1.05).min(1.0),
            (base.g * 0.97).min(1.0),
            (base.b * 0.88).min(1.0),
            1.0,
        ),
    }
}

pub fn night_rim_strength(ambient: Color) -> f32 {
    let luma = 0.2126 * ambient.r + 0.7152 * ambient.g + 0.0722 * ambient.b;
    ((0.72 - luma) / 0.38).clamp(0.0, 1.0)
}

pub fn collect_scene_glows_and_fireflies(
    world: &World,
    agents: &Agents,
    view_min: (usize, usize),
    view_max: (usize, usize),
    cam_center: Vec2,
    anim_time: f32,
) -> (Vec<(Vec2, Color)>, Vec<GlowLight>) {
    collect_scene_glows_and_fireflies_with_cap(
        world,
        agents,
        view_min,
        view_max,
        cam_center,
        anim_time,
        MAX_GLOW_LIGHTS,
    )
}

pub fn collect_scene_glows_and_fireflies_with_cap(
    world: &World,
    agents: &Agents,
    view_min: (usize, usize),
    view_max: (usize, usize),
    cam_center: Vec2,
    anim_time: f32,
    max_glows: usize,
) -> (Vec<(Vec2, Color)>, Vec<GlowLight>) {
    let mut glows = agents.collect_glow_lights();
    let mut fireflies = Vec::new();
    let twilight = world.time_of_day < 0.28 || world.time_of_day >= 0.64;

    for y in view_min.1..view_max.1 {
        for x in view_min.0..view_max.0 {
            let Some(cell) = world.get(x as i32, y as i32) else {
                continue;
            };
            let px = (x as f32 + 0.5) * TILE_PX;
            let py = (y as f32 + 0.5) * TILE_PX;
            let tile_pulse = (anim_time * 5.2 + x as f32 * 1.7 + y as f32 * 2.3).sin() * 4.2;
            match cell.material {
                Material::Campfire if cell.is_lit_campfire() => {
                    glows.push(GlowLight {
                        pos: vec2(px, py),
                        radius: agents::CAMPFIRE_LIGHT_RADIUS_TILES * TILE_PX + tile_pulse,
                        color: Color::new(1.0, 0.72, 0.34, 0.92),
                        bloom_alpha: 0.28,
                    });
                }
                Material::Lava => {
                    glows.push(GlowLight {
                        pos: vec2(px, py),
                        radius: 54.0 + tile_pulse * 0.55,
                        color: Color::new(1.0, 0.42, 0.14, 0.88),
                        bloom_alpha: 0.30,
                    });
                }
                Material::Magma => {
                    glows.push(GlowLight {
                        pos: vec2(px, py),
                        radius: 28.0 + tile_pulse * 0.25,
                        color: Color::new(0.88, 0.32, 0.10, 0.45),
                        bloom_alpha: 0.18,
                    });
                }
                Material::Fire => {
                    glows.push(GlowLight {
                        pos: vec2(px, py),
                        radius: 44.0 + tile_pulse * 0.85,
                        color: Color::new(1.0, 0.56, 0.20, 0.85),
                        bloom_alpha: 0.32,
                    });
                }
                Material::Cabin if twilight => {
                    glows.push(GlowLight {
                        pos: vec2(px + 3.0, py + 2.0),
                        radius: 34.0,
                        color: Color::new(0.98, 0.78, 0.38, 0.55),
                        bloom_alpha: 0.20,
                    });
                }
                Material::Tree | Material::BerryBush
                    if twilight && hash2(x as u32, y as u32, 73).is_multiple_of(13) =>
                {
                    let phase = anim_time * 1.4 + (x as f32 * 0.7) + (y as f32 * 1.1);
                    let fpos = vec2(px + phase.cos() * 7.0, py + (phase * 1.3).sin() * 6.0);
                    let twinkle = (phase * 2.2).sin() * 0.35 + 0.65;
                    let col = Color::new(0.76, 1.0, 0.42, 0.65 * twinkle);
                    fireflies.push((fpos, Color::new(0.86, 1.0, 0.52, twinkle)));
                    glows.push(GlowLight {
                        pos: fpos,
                        radius: 22.0 + twinkle * 4.0,
                        color: col,
                        bloom_alpha: 0.34,
                    });
                }
                _ => {}
            }
        }
    }

    if glows.len() > max_glows {
        glows.sort_by(|a, b| {
            a.pos
                .distance_squared(cam_center)
                .total_cmp(&b.pos.distance_squared(cam_center))
        });
        glows.truncate(max_glows);
    }

    (fireflies, glows)
}

pub fn draw_firefly_dots(fireflies: &[(Vec2, Color)]) {
    for &(pos, col) in fireflies {
        draw_rectangle(pos.x - 1.0, pos.y - 1.0, 2.0, 2.0, col);
    }
}
