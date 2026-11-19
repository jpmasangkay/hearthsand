use macroquad::prelude::*;

use crate::world::{
    Cell, Material, World, CHUNKS_X, CHUNKS_Y, CHUNK_SIZE, GRID_H, GRID_W, TILE_PX,
};

pub const MICRO_PX: usize = 4;
pub const TERRAIN_IMG_W: usize = GRID_W * MICRO_PX;
pub const TERRAIN_IMG_H: usize = GRID_H * MICRO_PX;
pub const MICRO_WORLD_PX: f32 = TILE_PX / MICRO_PX as f32;

/// Minimum unlit terrain pixel luminance floor (0..1) so rim-darkened or dark materials
/// never vanish at night or during rainstorms.
pub const MIN_TERRAIN_UNLIT_LUMA: f32 = 0.22;

/// Minimum post-lighting terrain pixel luminance floor (0..1) at night and in rain.
#[cfg(test)]
pub const MIN_LIT_TERRAIN_LUMA: f32 = 0.075;

const PAL_DEEP_WATER: usize = 0;
const PAL_SHALLOW_WATER: usize = 1;
const PAL_SAND: usize = 2;
const PAL_DIRT: usize = 3;
const PAL_TRENCH: usize = 4;
const PAL_TILLED_SOIL: usize = 5;
const PAL_WET_SOIL: usize = 6;
const PAL_GRASS: usize = 7;
const PAL_STONE: usize = 8;
const PAL_MAGMA: usize = 9;
const PAL_WOOD: usize = 10;
const PAL_ICE: usize = 11;
const PAL_ASH: usize = 12;
const PAL_GUNPOWDER: usize = 13;
const PAL_OIL: usize = 14;
const PAL_FIRE: usize = 15;
const PAL_LAVA: usize = 16;
const PAL_ACID: usize = 17;
const PAL_SMOKE: usize = 18;
const PAL_STEAM: usize = 19;

/// Single flat-shade terrain palette table (4 RGB shades per material).
pub const TERRAIN_PALETTES: [[[u8; 3]; 4]; 20] = [
    // 0: DeepWater
    [[28, 72, 142], [34, 82, 156], [42, 94, 170], [50, 106, 184]],
    // 1: ShallowWater
    [
        [46, 122, 178],
        [56, 138, 194],
        [68, 154, 208],
        [86, 172, 222],
    ],
    // 2: Sand
    [
        [208, 184, 124],
        [218, 194, 134],
        [226, 204, 144],
        [198, 174, 114],
    ],
    // 3: Dirt
    [[128, 94, 62], [138, 102, 68], [148, 110, 74], [118, 86, 56]],
    // 4: Trench (dry)
    [[94, 68, 46], [102, 74, 50], [110, 80, 54], [86, 62, 42]],
    // 5: TilledSoil
    [[110, 78, 52], [120, 86, 58], [130, 94, 64], [100, 72, 48]],
    // 6: WetSoil (raised luminance for night/rain readability while distinctly darker than TilledSoil)
    [[92, 68, 48], [100, 74, 52], [108, 80, 56], [84, 62, 44]],
    // 7: Grass
    [[64, 132, 54], [74, 144, 62], [86, 156, 68], [56, 122, 48]],
    // 8: Stone
    [
        [100, 104, 114],
        [114, 118, 128],
        [128, 132, 142],
        [88, 92, 102],
    ],
    // 9: Magma
    [[96, 56, 44], [114, 64, 46], [176, 68, 28], [228, 108, 34]],
    // 10: WoodWall
    [[132, 86, 46], [148, 98, 54], [160, 108, 60], [116, 74, 38]],
    // 11: Ice
    [
        [164, 218, 240],
        [182, 230, 248],
        [204, 242, 255],
        [146, 204, 230],
    ],
    // 12: Ash
    [[82, 82, 88], [94, 94, 100], [106, 106, 112], [74, 74, 80]],
    // 13: Gunpowder
    [[68, 66, 76], [78, 76, 86], [88, 84, 96], [60, 58, 68]],
    // 14: Oil
    [[64, 60, 78], [72, 68, 88], [80, 76, 98], [58, 54, 72]],
    // 15: Fire
    [
        [232, 76, 22],
        [248, 128, 28],
        [255, 186, 48],
        [255, 234, 116],
    ],
    // 16: Lava
    [[204, 52, 16], [232, 82, 20], [250, 124, 28], [255, 178, 56]],
    // 17: Acid
    [[96, 202, 36], [124, 226, 44], [154, 246, 64], [78, 180, 30]],
    // 18: Smoke
    [
        [136, 138, 146],
        [152, 154, 162],
        [166, 168, 176],
        [124, 126, 134],
    ],
    // 19: Steam
    [
        [214, 226, 238],
        [226, 236, 246],
        [238, 244, 252],
        [202, 216, 230],
    ],
];

#[inline]
pub fn hash2(x: u32, y: u32, seed: u32) -> u32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(1_442_695_041));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^ (h >> 16)
}

#[inline]
fn ground_material(cell: &Cell) -> Material {
    match cell.material {
        Material::Tree | Material::BerryBush => Material::Grass,
        Material::Crop(_) => {
            if cell.is_wet_crop() {
                Material::WetSoil
            } else {
                Material::TilledSoil
            }
        }
        Material::Cabin | Material::Campfire | Material::Chest => Material::Dirt,
        other => other,
    }
}

#[inline]
fn palette_index(mat: Material) -> usize {
    match mat {
        Material::DeepWater => PAL_DEEP_WATER,
        Material::ShallowWater => PAL_SHALLOW_WATER,
        Material::Sand => PAL_SAND,
        Material::Dirt => PAL_DIRT,
        Material::Trench => PAL_TRENCH,
        Material::TilledSoil => PAL_TILLED_SOIL,
        Material::WetSoil => PAL_WET_SOIL,
        Material::Grass => PAL_GRASS,
        Material::Stone => PAL_STONE,
        Material::Magma => PAL_MAGMA,
        Material::WoodWall => PAL_WOOD,
        Material::Ice => PAL_ICE,
        Material::Ash => PAL_ASH,
        Material::Gunpowder => PAL_GUNPOWDER,
        Material::Oil => PAL_OIL,
        Material::Fire => PAL_FIRE,
        Material::Lava => PAL_LAVA,
        Material::Acid => PAL_ACID,
        Material::Smoke => PAL_SMOKE,
        Material::Steam => PAL_STEAM,
        Material::Tree | Material::BerryBush => PAL_GRASS,
        Material::Crop(_) => PAL_TILLED_SOIL,
        Material::Cabin | Material::Campfire | Material::Chest => PAL_DIRT,
    }
}

#[inline]
fn is_soft_blendable(mat: Material) -> bool {
    matches!(
        mat,
        Material::Grass | Material::Dirt | Material::Sand | Material::Ash
    )
}

#[inline]
fn is_water_surface(cell: &Cell) -> bool {
    matches!(cell.material, Material::DeepWater | Material::ShallowWater)
        || (cell.material == Material::Trench && cell.water_volume > 0)
}

#[cfg(test)]
#[inline]
pub fn palette_shades(mat: Material) -> [[u8; 3]; 4] {
    TERRAIN_PALETTES[palette_index(mat)]
}

#[inline]
pub fn rgb_luminance(rgb: [u8; 3]) -> f32 {
    (0.2126 * rgb[0] as f32 + 0.7152 * rgb[1] as f32 + 0.0722 * rgb[2] as f32) / 255.0
}

#[inline]
pub fn enforce_min_terrain_rgb(rgb: [u8; 3]) -> [u8; 3] {
    let luma = rgb_luminance(rgb);
    if luma >= MIN_TERRAIN_UNLIT_LUMA {
        return rgb;
    }
    if luma <= 1e-4 {
        let v = (MIN_TERRAIN_UNLIT_LUMA * 255.0).ceil() as u8;
        return [v, v, v];
    }
    let scale = MIN_TERRAIN_UNLIT_LUMA / luma;
    [
        ((rgb[0] as f32 * scale).ceil() as u16).min(255) as u8,
        ((rgb[1] as f32 * scale).ceil() as u16).min(255) as u8,
        ((rgb[2] as f32 * scale).ceil() as u16).min(255) as u8,
    ]
}

/// Computes the post-lighting RGB and applies the minimum terrain luminance floor (`MIN_LIT_TERRAIN_LUMA`)
/// so terrain remains readable at night and during rainstorms.
#[cfg(test)]
#[inline]
pub fn lit_terrain_rgb(rgb: [u8; 3], ambient: Color) -> [u8; 3] {
    let base = enforce_min_terrain_rgb(rgb);
    let mut r = (base[0] as f32 / 255.0) * ambient.r;
    let mut g = (base[1] as f32 / 255.0) * ambient.g;
    let mut b = (base[2] as f32 / 255.0) * ambient.b;
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    if luma < MIN_LIT_TERRAIN_LUMA {
        if luma > 1e-5 {
            let scale = MIN_LIT_TERRAIN_LUMA / luma;
            r = (r * scale).min(1.0);
            g = (g * scale).min(1.0);
            b = (b * scale).min(1.0);
        } else {
            r = MIN_LIT_TERRAIN_LUMA;
            g = MIN_LIT_TERRAIN_LUMA;
            b = MIN_LIT_TERRAIN_LUMA;
        }
    }
    [
        (r * 255.0).round().clamp(0.0, 255.0) as u8,
        (g * 255.0).round().clamp(0.0, 255.0) as u8,
        (b * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

#[cfg(test)]
#[inline]
pub fn lit_terrain_luminance(rgb: [u8; 3], ambient: Color) -> f32 {
    rgb_luminance(lit_terrain_rgb(rgb, ambient))
}

#[inline]
fn darken_rgb(rgb: [u8; 3], scale_num: u16) -> [u8; 3] {
    enforce_min_terrain_rgb([
        ((rgb[0] as u16 * scale_num) >> 8) as u8,
        ((rgb[1] as u16 * scale_num) >> 8) as u8,
        ((rgb[2] as u16 * scale_num) >> 8) as u8,
    ])
}

#[inline]
fn blend_rgb(a: [u8; 3], b: [u8; 3], weight_b: u16) -> [u8; 3] {
    let weight_a = 256_u16.saturating_sub(weight_b);
    [
        ((a[0] as u16 * weight_a + b[0] as u16 * weight_b) >> 8) as u8,
        ((a[1] as u16 * weight_a + b[1] as u16 * weight_b) >> 8) as u8,
        ((a[2] as u16 * weight_a + b[2] as u16 * weight_b) >> 8) as u8,
    ]
}

#[inline]
fn pick_shade(
    mat: Material,
    gx: u32,
    gy: u32,
    mx: u32,
    my: u32,
    variant: u32,
    phases: AnimPhases,
) -> [u8; 3] {
    let pal = &TERRAIN_PALETTES[palette_index(mat)];
    let px = gx * MICRO_PX as u32 + mx;
    let py = gy * MICRO_PX as u32 + my;

    match mat {
        Material::DeepWater | Material::ShallowWater => {
            let wave = hash2(px.wrapping_add(phases.water), py, variant ^ 0x17);
            let shimmer = hash2(
                px.wrapping_add(phases.water.wrapping_mul(2)),
                py.wrapping_add(phases.water & 1),
                variant ^ 0x91,
            );
            let idx = if shimmer.is_multiple_of(19) {
                3
            } else {
                (wave & 3) as usize
            };
            pal[idx]
        }
        Material::Lava => {
            let h = hash2(
                px.wrapping_add(phases.lava),
                py.wrapping_sub(phases.lava >> 1),
                variant ^ 0x4B,
            );
            pal[(h & 3) as usize]
        }
        Material::Fire => {
            let h = hash2(px, py.wrapping_add(phases.fire), variant ^ 0x73);
            let bias = if my == 0 {
                0
            } else if my == 3 {
                1
            } else {
                0
            };
            pal[(((h & 3) as usize) + bias).min(3)]
        }
        Material::Acid => {
            let h = hash2(
                px.wrapping_add(phases.acid & 3),
                py.wrapping_add(phases.acid),
                variant ^ 0xA5,
            );
            pal[(h & 3) as usize]
        }
        Material::TilledSoil | Material::WetSoil => {
            let h = hash2(gx, gy, (mx << 2) | my | (variant << 4));
            let idx = if my.is_multiple_of(2) {
                if (h & 3) == 0 {
                    3
                } else {
                    0
                }
            } else {
                1 + ((h & 1) as usize)
            };
            pal[idx]
        }
        Material::Magma => {
            let h = hash2(px, py, variant ^ 0x5D);
            let idx = if (h & 7) == 0 {
                3
            } else if (h & 3) == 0 {
                2
            } else {
                (h & 1) as usize
            };
            pal[idx]
        }
        _ => {
            let h = hash2(
                gx.wrapping_mul(5).wrapping_add(mx),
                gy.wrapping_mul(5).wrapping_add(my),
                variant,
            );
            pal[(h & 3) as usize]
        }
    }
}

#[derive(Copy, Clone)]
struct AnimPhases {
    water: u32,
    lava: u32,
    fire: u32,
    acid: u32,
}

impl AnimPhases {
    /// Quantized to a 10 Hz animation clock so both full and incremental rasterizers match bit-for-bit.
    #[inline]
    fn from_time(time: f32) -> (u32, Self) {
        let step_10hz = (time.max(0.0) * 10.0).floor() as u32;
        let phases = Self {
            water: (step_10hz * 3) / 10,
            lava: (step_10hz * 2) / 10,
            fire: (step_10hz * 8) / 10,
            acid: (step_10hz * 4) / 10,
        };
        (step_10hz, phases)
    }
}

#[inline]
fn is_animated_cell(cell: &Cell) -> bool {
    matches!(
        cell.material,
        Material::DeepWater
            | Material::ShallowWater
            | Material::Lava
            | Material::Fire
            | Material::Acid
    ) || (cell.material == Material::Trench && cell.water_volume > 0)
}

#[inline]
fn rasterize_cell(cells: &[Cell], bytes: &mut [u8], x: usize, y: usize, phases: AnimPhases) {
    let row_stride = TERRAIN_IMG_W * 4;
    let row_base = y * GRID_W;
    let cell = &cells[row_base + x];
    let mat = ground_material(cell);
    let variant = (cell.variant >> 1) as u32;

    let n_cell = (y > 0).then(|| &cells[(y - 1) * GRID_W + x]);
    let s_cell = (y + 1 < GRID_H).then(|| &cells[(y + 1) * GRID_W + x]);
    let w_cell = (x > 0).then(|| &cells[row_base + x - 1]);
    let e_cell = (x + 1 < GRID_W).then(|| &cells[row_base + x + 1]);

    let n_mat = n_cell.map_or(mat, ground_material);
    let s_mat = s_cell.map_or(mat, ground_material);
    let w_mat = w_cell.map_or(mat, ground_material);
    let e_mat = e_cell.map_or(mat, ground_material);

    let water_top_highlight =
        is_water_surface(cell) && n_cell.is_some_and(|nc| !is_water_surface(nc));

    let trench_water = cell.material == Material::Trench && cell.water_volume > 0;
    let trench_water_row = if !trench_water {
        4
    } else if cell.water_volume >= 60 {
        0
    } else if cell.water_volume >= 25 {
        1
    } else {
        2
    };

    let cell_px_x = x * MICRO_PX;
    let cell_px_y = y * MICRO_PX;

    for my in 0..MICRO_PX {
        let py_off = (cell_px_y + my) * row_stride;
        for mx in 0..MICRO_PX {
            let mut rgb = if trench_water && my >= trench_water_row {
                let w_rgb = pick_shade(
                    Material::ShallowWater,
                    x as u32,
                    y as u32,
                    mx as u32,
                    my as u32,
                    variant,
                    phases,
                );
                if my == trench_water_row {
                    blend_rgb(w_rgb, [196, 234, 250], 90)
                } else {
                    w_rgb
                }
            } else {
                pick_shade(
                    mat, x as u32, y as u32, mx as u32, my as u32, variant, phases,
                )
            };

            // Soft organic dither between neighboring soft ground types
            if is_soft_blendable(mat) {
                let edge_neighbor = if my == 0 && n_mat != mat && is_soft_blendable(n_mat) {
                    Some(n_mat)
                } else if my == 3 && s_mat != mat && is_soft_blendable(s_mat) {
                    Some(s_mat)
                } else if mx == 0 && w_mat != mat && is_soft_blendable(w_mat) {
                    Some(w_mat)
                } else if mx == 3 && e_mat != mat && is_soft_blendable(e_mat) {
                    Some(e_mat)
                } else {
                    None
                };
                if let Some(nb) = edge_neighbor {
                    if hash2(
                        (cell_px_x + mx) as u32,
                        (cell_px_y + my) as u32,
                        variant ^ 0x3C,
                    )
                    .is_multiple_of(3)
                    {
                        rgb = pick_shade(
                            nb, x as u32, y as u32, mx as u32, my as u32, variant, phases,
                        );
                    }
                }
            }

            // Darken rims on exposed edges of Stone, Dirt, and WoodWall
            let exposed_edge = match mat {
                Material::Stone => {
                    let is_rock = |m: Material| matches!(m, Material::Stone | Material::Magma);
                    (my == 0 && !is_rock(n_mat))
                        || (my == 3 && !is_rock(s_mat))
                        || (mx == 0 && !is_rock(w_mat))
                        || (mx == 3 && !is_rock(e_mat))
                }
                Material::Dirt => {
                    let is_low = |m: Material| {
                        matches!(
                            m,
                            Material::DeepWater
                                | Material::ShallowWater
                                | Material::Trench
                                | Material::Sand
                                | Material::Ash
                        )
                    };
                    (my == 0 && is_low(n_mat))
                        || (my == 3 && is_low(s_mat))
                        || (mx == 0 && is_low(w_mat))
                        || (mx == 3 && is_low(e_mat))
                }
                Material::WoodWall => {
                    let is_wood = |m: Material| m == Material::WoodWall;
                    (my == 0 && !is_wood(n_mat))
                        || (my == 3 && !is_wood(s_mat))
                        || (mx == 0 && !is_wood(w_mat))
                        || (mx == 3 && !is_wood(e_mat))
                }
                _ => false,
            };
            if exposed_edge {
                let rim_scale = if mat == Material::Stone && my == 3 {
                    176 // ~0.69 on lower cliff lip
                } else if mat == Material::Dirt {
                    204 // ~0.80 subtle bank edge
                } else {
                    188 // ~0.73 exposed rim
                };
                rgb = darken_rgb(rgb, rim_scale);
            }

            // Highlight row along top exposed water surface
            if water_top_highlight && my == 0 && !trench_water {
                rgb = blend_rgb(rgb, [196, 234, 250], 96);
            }

            rgb = enforce_min_terrain_rgb(rgb);
            let off = py_off + (cell_px_x + mx) * 4;
            bytes[off] = rgb[0];
            bytes[off + 1] = rgb[1];
            bytes[off + 2] = rgb[2];
            bytes[off + 3] = 255;
        }
    }
}

/// Pure function rasterizing the full 160x100 world into a 640x400 nearest-filter Image
/// (4x4 micro-pixels per cell).
pub fn render_terrain(world: &World, time: f32) -> Image {
    let mut img = Image::gen_image_color(
        TERRAIN_IMG_W as u16,
        TERRAIN_IMG_H as u16,
        Color::from_rgba(30, 30, 46, 255),
    );
    rasterize_terrain_into(world, time, &mut img);
    img
}

/// Rasterizes the full world terrain into an existing 640x400 Image buffer without allocating.
pub fn rasterize_terrain_into(world: &World, time: f32, img: &mut Image) {
    let (_, phases) = AnimPhases::from_time(time);
    let cells = &world.cells;
    let bytes = &mut img.bytes;
    for y in 0..GRID_H {
        for x in 0..GRID_W {
            rasterize_cell(cells, bytes, x, y, phases);
        }
    }
}

fn chunk_visual_signature(world: &World, cx: usize, cy: usize) -> u64 {
    let x0 = cx * CHUNK_SIZE;
    let x1 = (x0 + CHUNK_SIZE).min(GRID_W);
    let y0 = cy * CHUNK_SIZE;
    let y1 = (y0 + CHUNK_SIZE).min(GRID_H);
    let mut sig = 0xCBF2_9CE4_8422_2325_u64;
    for y in y0..y1 {
        let base = y * GRID_W;
        for x in x0..x1 {
            let c = &world.cells[base + x];
            let water_bucket = if c.material == Material::Trench {
                if c.water_volume >= 60 {
                    3
                } else if c.water_volume >= 25 {
                    2
                } else if c.water_volume > 0 {
                    1
                } else {
                    0
                }
            } else {
                0
            };
            let v = (palette_index(ground_material(c)) as u64)
                | ((c.variant as u64) << 6)
                | ((water_bucket as u64) << 14);
            sig ^= v;
            sig = sig.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    sig
}

/// Incremental dirty-chunk terrain rasterizer with 10 Hz animated-cell refresh (B11).
/// Output is byte-for-byte identical to `render_terrain(world, time)`.
pub struct TerrainRasterizer {
    pub img: Image,
    chunk_sigs: Vec<u64>,
    chunk_anim_cells: Vec<Vec<(usize, usize)>>,
    last_anim_step: u32,
    initialized: bool,
}

impl TerrainRasterizer {
    pub fn new(world: &World, time: f32) -> Self {
        let (anim_step, _) = AnimPhases::from_time(time);
        let mut r = Self {
            img: render_terrain(world, time),
            chunk_sigs: vec![0; CHUNKS_X * CHUNKS_Y],
            chunk_anim_cells: vec![Vec::new(); CHUNKS_X * CHUNKS_Y],
            last_anim_step: anim_step,
            initialized: true,
        };
        for cy in 0..CHUNKS_Y {
            for cx in 0..CHUNKS_X {
                let cidx = cy * CHUNKS_X + cx;
                r.chunk_sigs[cidx] = chunk_visual_signature(world, cx, cy);
                r.rebuild_chunk_anim_list(world, cx, cy);
            }
        }
        r
    }

    pub fn invalidate_all(&mut self) {
        self.initialized = false;
    }

    /// Updates dirty chunks and refreshes animated cells at 10 Hz. Returns `true` if `self.img` changed.
    pub fn update(&mut self, world: &World, time: f32) -> bool {
        let (anim_step, phases) = AnimPhases::from_time(time);
        let num_chunks = CHUNKS_X * CHUNKS_Y;

        if !self.initialized {
            for cy in 0..CHUNKS_Y {
                for cx in 0..CHUNKS_X {
                    let cidx = cy * CHUNKS_X + cx;
                    self.chunk_sigs[cidx] = chunk_visual_signature(world, cx, cy);
                    self.rebuild_chunk_anim_list(world, cx, cy);
                }
            }
            rasterize_terrain_into(world, time, &mut self.img);
            self.last_anim_step = anim_step;
            self.initialized = true;
            return true;
        }

        let mut dirty = [false; CHUNKS_X * CHUNKS_Y];
        let mut any_dirty = false;
        for cy in 0..CHUNKS_Y {
            for cx in 0..CHUNKS_X {
                let cidx = cy * CHUNKS_X + cx;
                let sig = chunk_visual_signature(world, cx, cy);
                if sig != self.chunk_sigs[cidx] {
                    self.chunk_sigs[cidx] = sig;
                    self.rebuild_chunk_anim_list(world, cx, cy);
                    dirty[cidx] = true;
                    any_dirty = true;
                    // Also mark cardinal neighbor chunks dirty for cross-border rim/highlight/dither
                    if cx > 0 {
                        dirty[cidx - 1] = true;
                    }
                    if cx + 1 < CHUNKS_X {
                        dirty[cidx + 1] = true;
                    }
                    if cy > 0 {
                        dirty[cidx - CHUNKS_X] = true;
                    }
                    if cy + 1 < CHUNKS_Y {
                        dirty[cidx + CHUNKS_X] = true;
                    }
                }
            }
        }

        let anim_ticked = anim_step != self.last_anim_step;
        if anim_ticked {
            self.last_anim_step = anim_step;
        }

        if !any_dirty && !anim_ticked {
            return false;
        }

        let cells = &world.cells;
        let bytes = &mut self.img.bytes;
        let mut updated_any = any_dirty;

        for cidx in 0..num_chunks {
            if dirty[cidx] {
                let cx = cidx % CHUNKS_X;
                let cy = cidx / CHUNKS_X;
                let x0 = cx * CHUNK_SIZE;
                let x1 = (x0 + CHUNK_SIZE).min(GRID_W);
                let y0 = cy * CHUNK_SIZE;
                let y1 = (y0 + CHUNK_SIZE).min(GRID_H);
                for y in y0..y1 {
                    for x in x0..x1 {
                        rasterize_cell(cells, bytes, x, y, phases);
                    }
                }
            } else if anim_ticked && !self.chunk_anim_cells[cidx].is_empty() {
                updated_any = true;
                for &(x, y) in &self.chunk_anim_cells[cidx] {
                    rasterize_cell(cells, bytes, x, y, phases);
                }
            }
        }

        updated_any
    }

    fn rebuild_chunk_anim_list(&mut self, world: &World, cx: usize, cy: usize) {
        let cidx = cy * CHUNKS_X + cx;
        let list = &mut self.chunk_anim_cells[cidx];
        list.clear();
        let x0 = cx * CHUNK_SIZE;
        let x1 = (x0 + CHUNK_SIZE).min(GRID_W);
        let y0 = cy * CHUNK_SIZE;
        let y1 = (y0 + CHUNK_SIZE).min(GRID_H);
        for y in y0..y1 {
            let base = y * GRID_W;
            for x in x0..x1 {
                if is_animated_cell(&world.cells[base + x]) {
                    list.push((x, y));
                }
            }
        }
    }
}
