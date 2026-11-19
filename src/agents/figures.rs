use macroquad::prelude::*;

use crate::render::MICRO_WORLD_PX;
use crate::world::{Cell, Material, TILE_PX};

use super::{CritterKind, ItemKind, Payload, ThoughtIcon};

pub(super) const M: f32 = MICRO_WORLD_PX;
pub(super) const SHADOW_STRIP: Color = Color::from_rgba(14, 16, 28, 72);

#[derive(Copy, Clone)]
pub(super) enum PropKind {
    Tree,
    BerryBush { ripe: bool },
    Crop(u8),
    Cabin { lit_window: bool },
    Campfire { lit: bool },
    Chest,
}

#[derive(Copy, Clone)]
pub(super) enum ProjVisual {
    Bomb,
    Spell(Payload),
}

pub(super) struct VillagerDraw {
    pub(super) id: usize,
    pub(super) pos: Vec2,
    pub(super) dir: u8,
    pub(super) step: usize,
    pub(super) bob: f32,
    pub(super) hit_flash: bool,
    pub(super) bucket: bool,
    pub(super) carrying: Option<ItemKind>,
    pub(super) night_rim: f32,
}

pub(super) struct MageDraw {
    pub(super) pos: Vec2,
    pub(super) dir: u8,
    pub(super) step: usize,
    pub(super) bob: f32,
    pub(super) aim_dir: Vec2,
    pub(super) wand_color: Color,
    pub(super) hit_flash: bool,
    pub(super) cast_flash: bool,
    pub(super) night_rim: f32,
}

pub(super) struct CritterDraw {
    pub(super) kind: CritterKind,
    pub(super) pos: Vec2,
    pub(super) flip_x: bool,
    pub(super) step: usize,
    pub(super) bob: f32,
    pub(super) stunned: bool,
    pub(super) anim_tick: usize,
    pub(super) is_night: bool,
}

#[inline]
pub(super) fn snap_m(v: f32) -> f32 {
    (v / M).round() * M
}

pub(super) fn classify_tall_prop(cell: &Cell, twilight: bool) -> Option<PropKind> {
    match cell.material {
        Material::Tree => Some(PropKind::Tree),
        Material::BerryBush => Some(PropKind::BerryBush {
            ripe: cell.has_berries(),
        }),
        Material::Crop(stage) => Some(PropKind::Crop(stage)),
        Material::Cabin => Some(PropKind::Cabin {
            lit_window: twilight,
        }),
        Material::Campfire => Some(PropKind::Campfire {
            lit: cell.is_lit_campfire(),
        }),
        Material::Chest => Some(PropKind::Chest),
        _ => None,
    }
}

pub(super) fn draw_tall_prop(x: usize, y: usize, kind: PropKind, variant: u8, anim_tick: usize) {
    let tx = x as f32 * TILE_PX;
    let ty = y as f32 * TILE_PX;
    match kind {
        PropKind::Tree => {
            draw_rectangle(tx, ty + 3.0 * M, 4.0 * M, M, SHADOW_STRIP);
            draw_rectangle(
                tx + M,
                ty + 2.0 * M,
                2.0 * M,
                2.0 * M,
                Color::from_rgba(98, 64, 38, 255),
            );
            let dark = Color::from_rgba(38, 86, 36, 255);
            let mid = if variant & 2 == 0 {
                Color::from_rgba(52, 114, 46, 255)
            } else {
                Color::from_rgba(46, 106, 44, 255)
            };
            let hi = Color::from_rgba(74, 142, 60, 255);
            draw_rectangle(tx, ty, 4.0 * M, 2.0 * M, dark);
            draw_rectangle(tx, ty - M, 4.0 * M, 2.0 * M, mid);
            draw_rectangle(tx + M, ty - 2.0 * M, 2.0 * M, M, hi);
            draw_rectangle(tx + M, ty - M, M, M, hi);
        }
        PropKind::BerryBush { ripe } => {
            draw_rectangle(tx + 0.5 * M, ty + 3.0 * M, 3.0 * M, 0.75 * M, SHADOW_STRIP);
            draw_rectangle(
                tx,
                ty + M,
                4.0 * M,
                2.0 * M,
                Color::from_rgba(44, 96, 44, 255),
            );
            draw_rectangle(
                tx + M,
                ty,
                2.0 * M,
                2.0 * M,
                Color::from_rgba(62, 124, 54, 255),
            );
            if ripe {
                let berry = Color::from_rgba(224, 56, 74, 255);
                draw_rectangle(tx + M, ty + M, M * 0.75, M * 0.75, berry);
                draw_rectangle(tx + 2.25 * M, ty + 1.25 * M, M * 0.75, M * 0.75, berry);
                draw_rectangle(tx + 1.5 * M, ty + 0.25 * M, M * 0.75, M * 0.75, berry);
            }
        }
        PropKind::Crop(stage) => match stage {
            1 => {
                let c = Color::from_rgba(116, 188, 72, 255);
                draw_rectangle(tx + M, ty + 2.0 * M, M, M, c);
                draw_rectangle(tx + 2.0 * M, ty + M, M, M, c);
            }
            2 => {
                let c0 = Color::from_rgba(78, 156, 54, 255);
                let c1 = Color::from_rgba(104, 184, 66, 255);
                draw_rectangle(tx + M, ty + M, M, 2.0 * M, c0);
                draw_rectangle(tx + 2.0 * M, ty + M, M, 2.0 * M, c1);
            }
            3 => {
                let stem = Color::from_rgba(92, 162, 54, 255);
                let tip = Color::from_rgba(176, 206, 82, 255);
                draw_rectangle(tx, ty + M, M, 2.0 * M, stem);
                draw_rectangle(tx + 2.0 * M, ty + M, M, 2.0 * M, stem);
                draw_rectangle(tx + M, ty, M, 2.0 * M, tip);
                draw_rectangle(tx + 3.0 * M, ty + M, M, M, tip);
            }
            _ => {
                let gold0 = Color::from_rgba(218, 174, 52, 255);
                let gold1 = Color::from_rgba(244, 208, 84, 255);
                draw_rectangle(tx, ty + M, 4.0 * M, 2.0 * M, gold0);
                draw_rectangle(tx, ty, M, 2.0 * M, gold1);
                draw_rectangle(tx + 2.0 * M, ty, M, 2.0 * M, gold1);
                draw_rectangle(tx + 3.0 * M, ty + M, M, M, gold1);
            }
        },
        PropKind::Cabin { lit_window } => {
            draw_rectangle(tx, ty + 3.5 * M, 4.0 * M, 0.75 * M, SHADOW_STRIP);
            draw_rectangle(
                tx,
                ty + M,
                4.0 * M,
                3.0 * M,
                Color::from_rgba(148, 102, 62, 255),
            );
            draw_rectangle(
                tx + M,
                ty + 2.0 * M,
                M,
                2.0 * M,
                Color::from_rgba(76, 48, 28, 255),
            );
            let win = if lit_window {
                Color::from_rgba(255, 212, 104, 255)
            } else {
                Color::from_rgba(88, 118, 146, 255)
            };
            draw_rectangle(tx + 2.5 * M, ty + 2.0 * M, M, M, win);
            draw_rectangle(tx, ty, 4.0 * M, M, Color::from_rgba(142, 68, 44, 255));
            draw_rectangle(
                tx + 0.5 * M,
                ty - M,
                3.0 * M,
                M,
                Color::from_rgba(168, 84, 54, 255),
            );
        }
        PropKind::Campfire { lit } => {
            let stone = Color::from_rgba(104, 108, 118, 255);
            let log = Color::from_rgba(92, 58, 34, 255);
            draw_rectangle(tx + 0.5 * M, ty + 2.5 * M, 3.0 * M, M, stone);
            draw_rectangle(tx + M, ty + M, 2.0 * M, 2.0 * M, log);
            if lit {
                let f = anim_tick % 2;
                let outer = Color::from_rgba(246, 118, 28, 255);
                let core = Color::from_rgba(255, 224, 96, 255);
                draw_rectangle(
                    tx + M,
                    ty + (if f == 0 { 0.5 } else { 0.75 }) * M,
                    2.0 * M,
                    1.5 * M,
                    outer,
                );
                draw_rectangle(
                    tx + 1.5 * M,
                    ty + (if f == 0 { 0.0 } else { 0.5 }) * M,
                    M,
                    M,
                    core,
                );
            }
        }
        PropKind::Chest => {
            draw_rectangle(tx + 0.5 * M, ty + 3.0 * M, 3.0 * M, 0.75 * M, SHADOW_STRIP);
            draw_rectangle(
                tx + 0.5 * M,
                ty + M,
                3.0 * M,
                2.0 * M,
                Color::from_rgba(154, 104, 52, 255),
            );
            draw_rectangle(
                tx + 0.5 * M,
                ty + M,
                3.0 * M,
                0.5 * M,
                Color::from_rgba(178, 126, 68, 255),
            );
            draw_rectangle(
                tx + 1.5 * M,
                ty + 1.75 * M,
                M,
                0.75 * M,
                Color::from_rgba(236, 198, 76, 255),
            );
        }
    }
}

pub(super) fn draw_villager_figure(d: VillagerDraw) {
    let foot_x = snap_m(d.pos.x);
    let foot_y = snap_m(d.pos.y + 6.0);
    let left = foot_x - 1.5 * M;
    let top = foot_y - 5.0 * M - d.bob;

    draw_rectangle(left, foot_y - 0.5 * M, 3.0 * M, 0.75 * M, SHADOW_STRIP);

    let (tunic, tunic_shade) = if d.hit_flash {
        (
            Color::from_rgba(244, 108, 108, 255),
            Color::from_rgba(204, 72, 72, 255),
        )
    } else {
        match d.id % 4 {
            0 => (
                Color::from_rgba(74, 134, 184, 255),
                Color::from_rgba(56, 106, 152, 255),
            ),
            1 => (
                Color::from_rgba(182, 122, 68, 255),
                Color::from_rgba(148, 94, 48, 255),
            ),
            2 => (
                Color::from_rgba(88, 152, 92, 255),
                Color::from_rgba(66, 120, 70, 255),
            ),
            _ => (
                Color::from_rgba(166, 92, 114, 255),
                Color::from_rgba(134, 68, 90, 255),
            ),
        }
    };
    let hair = Color::from_rgba(84, 54, 34, 255);
    let skin = if d.hit_flash {
        Color::from_rgba(255, 168, 168, 255)
    } else {
        Color::from_rgba(238, 198, 156, 255)
    };
    let boots = Color::from_rgba(58, 42, 30, 255);

    draw_rectangle(left, top, 3.0 * M, M, hair);
    draw_rectangle(
        left,
        top + M,
        3.0 * M,
        M,
        if d.dir == 1 { hair } else { skin },
    );
    if d.dir == 2 {
        draw_rectangle(left + 2.0 * M, top + M, M, M, hair);
    } else if d.dir == 3 {
        draw_rectangle(left, top + M, M, M, hair);
    }
    draw_rectangle(left, top + 2.0 * M, 3.0 * M, 2.0 * M, tunic);
    draw_rectangle(left, top + 3.0 * M, 3.0 * M, M, tunic_shade);
    if d.step == 0 {
        draw_rectangle(left, top + 4.0 * M, M, M, boots);
        draw_rectangle(left + 2.0 * M, top + 4.0 * M, M, M, boots);
    } else {
        draw_rectangle(left + 0.5 * M, top + 4.0 * M, M, M, boots);
        draw_rectangle(left + 1.5 * M, top + 4.0 * M, M, M, boots);
    }

    if d.night_rim > 0.05 {
        let rim = Color::new(0.76, 0.88, 1.0, 0.45 * d.night_rim);
        draw_rectangle(left, top, 3.0 * M, 0.35 * M, rim);
        draw_rectangle(left, top + 2.0 * M, 0.35 * M, 2.0 * M, rim);
    }

    if d.bucket {
        draw_rectangle(
            left + 2.5 * M,
            top + 2.5 * M,
            M,
            M,
            Color::from_rgba(76, 174, 255, 255),
        );
    } else if let Some(kind) = d.carrying {
        draw_rectangle(left - 0.5 * M, top + 2.0 * M, M, M, kind.color());
    }
}

pub(super) fn draw_mage_figure(d: MageDraw) {
    let foot_x = snap_m(d.pos.x);
    let foot_y = snap_m(d.pos.y + 6.0);
    let left = foot_x - 1.5 * M;
    let top = foot_y - 5.0 * M - d.bob;

    draw_rectangle(left, foot_y - 0.5 * M, 3.0 * M, 0.75 * M, SHADOW_STRIP);

    let hat = Color::from_rgba(74, 52, 136, 255);
    let hat_band = Color::from_rgba(238, 196, 78, 255);
    let robe = if d.hit_flash {
        Color::from_rgba(246, 118, 128, 255)
    } else {
        Color::from_rgba(116, 82, 192, 255)
    };
    let robe_dark = Color::from_rgba(82, 56, 146, 255);
    let skin = Color::from_rgba(240, 206, 172, 255);
    let boots = Color::from_rgba(44, 34, 62, 255);

    draw_rectangle(left + M, top - M, M, M, hat);
    draw_rectangle(left, top, 3.0 * M, M, hat);
    draw_rectangle(left + 0.5 * M, top + 0.65 * M, 2.0 * M, 0.35 * M, hat_band);
    draw_rectangle(
        left + 0.5 * M,
        top + M,
        2.0 * M,
        M,
        if d.dir == 1 { hat } else { skin },
    );
    draw_rectangle(left, top + 2.0 * M, 3.0 * M, 2.0 * M, robe);
    draw_rectangle(left, top + 3.0 * M, 3.0 * M, M, robe_dark);
    if d.step == 0 {
        draw_rectangle(left, top + 4.0 * M, M, M, boots);
        draw_rectangle(left + 2.0 * M, top + 4.0 * M, M, M, boots);
    } else {
        draw_rectangle(left + 0.5 * M, top + 4.0 * M, M, M, boots);
        draw_rectangle(left + 1.5 * M, top + 4.0 * M, M, M, boots);
    }

    if d.night_rim > 0.05 {
        let rim = Color::new(0.82, 0.80, 1.0, 0.40 * d.night_rim);
        draw_rectangle(left + M, top - M, M, 0.35 * M, rim);
    }

    let hand = vec2(foot_x, top + 2.5 * M);
    let tip = hand + d.aim_dir * (2.5 * M);
    draw_line(
        hand.x,
        hand.y,
        tip.x,
        tip.y,
        M * 0.4,
        Color::from_rgba(148, 98, 52, 255),
    );
    let tip_x = snap_m(tip.x - 0.5 * M);
    let tip_y = snap_m(tip.y - 0.5 * M);
    if d.cast_flash {
        draw_rectangle(tip_x - 0.25 * M, tip_y - 0.25 * M, 1.5 * M, 1.5 * M, WHITE);
    }
    draw_rectangle(tip_x, tip_y, M, M, d.wand_color);
}

pub(super) fn draw_mage_ghost_figure(pos: Vec2, dir: u8, tint: Color) {
    let foot_x = snap_m(pos.x);
    let foot_y = snap_m(pos.y + 6.0);
    let left = foot_x - 1.5 * M;
    let top = foot_y - 5.0 * M;
    let tip_x = match dir {
        2 => left + 0.5 * M,
        3 => left + 1.5 * M,
        _ => left + M,
    };
    draw_rectangle(tip_x, top - M, M, M, tint);
    draw_rectangle(left, top, 3.0 * M, 4.0 * M, tint);
}

pub(super) fn draw_critter_figure(d: CritterDraw) {
    let fx = snap_m(d.pos.x);
    let fy = snap_m(d.pos.y + 5.0);
    match d.kind {
        CritterKind::Rabbit => {
            let left = fx - M;
            let top = fy - 2.0 * M - d.bob;
            draw_rectangle(left, fy - 0.5 * M, 2.0 * M, 0.5 * M, SHADOW_STRIP);
            let fur = if d.stunned {
                Color::from_rgba(164, 232, 255, 255)
            } else {
                Color::from_rgba(226, 216, 198, 255)
            };
            let ear_x = if d.flip_x { left } else { left + M };
            draw_rectangle(ear_x, top - M, 0.75 * M, M, fur);
            draw_rectangle(left, top, 2.0 * M, 2.0 * M, fur);
            let nose_x = if d.flip_x { left } else { left + 1.5 * M };
            draw_rectangle(
                nose_x,
                top + 0.5 * M,
                0.5 * M,
                0.5 * M,
                Color::from_rgba(232, 144, 158, 255),
            );
        }
        CritterKind::Wolf => {
            let left = fx - 2.0 * M;
            let top = fy - 2.5 * M - d.bob;
            draw_rectangle(left, fy - 0.5 * M, 4.0 * M, 0.75 * M, SHADOW_STRIP);
            let body = if d.stunned {
                Color::from_rgba(164, 232, 255, 255)
            } else {
                Color::from_rgba(104, 110, 124, 255)
            };
            let dark = Color::from_rgba(72, 78, 92, 255);
            draw_rectangle(left, top, 4.0 * M, 1.5 * M, body);
            let (head_x, tail_x) = if d.flip_x {
                (left, left + 3.0 * M)
            } else {
                (left + 3.0 * M, left)
            };
            draw_rectangle(head_x, top - 0.75 * M, M, M, dark);
            draw_rectangle(tail_x, top - 0.5 * M, M, 0.5 * M, body);
            let leg_off = if d.step == 0 { 0.0 } else { 0.5 * M };
            draw_rectangle(left + leg_off, top + 1.5 * M, M, M, dark);
            draw_rectangle(left + 2.5 * M - leg_off, top + 1.5 * M, M, M, dark);
            if d.is_night {
                let eye_x = if d.flip_x { left } else { left + 3.5 * M };
                draw_rectangle(
                    eye_x,
                    top - 0.25 * M,
                    0.5 * M,
                    0.5 * M,
                    Color::from_rgba(255, 206, 72, 255),
                );
            }
        }
    }

    if d.stunned {
        let spark_x = fx + ((d.anim_tick % 3) as f32 - 1.0) * M;
        draw_rectangle(
            spark_x,
            fy - 4.0 * M,
            0.5 * M,
            0.5 * M,
            Color::from_rgba(255, 240, 96, 255),
        );
    }
}

pub(super) fn draw_projectile_figure(pos: Vec2, visual: ProjVisual, anim_tick: usize) {
    let bx = snap_m(pos.x - M);
    let by = snap_m(pos.y - M);
    draw_rectangle(bx, by + 2.5 * M, 2.0 * M, 0.5 * M, SHADOW_STRIP);
    match visual {
        ProjVisual::Bomb => {
            draw_rectangle(bx, by, 2.0 * M, 2.0 * M, Color::from_rgba(44, 46, 56, 255));
            let spark = if anim_tick.is_multiple_of(2) {
                Color::from_rgba(255, 212, 72, 255)
            } else {
                Color::from_rgba(255, 114, 42, 255)
            };
            draw_rectangle(bx + M, by - 0.5 * M, 0.75 * M, 0.75 * M, spark);
        }
        ProjVisual::Spell(payload) => {
            let col = payload.color();
            draw_rectangle(bx, by, 2.0 * M, 2.0 * M, col);
            draw_rectangle(
                bx + 0.5 * M,
                by + 0.5 * M,
                M,
                M,
                Color::new(
                    (col.r + 0.35).min(1.0),
                    (col.g + 0.35).min(1.0),
                    (col.b + 0.35).min(1.0),
                    1.0,
                ),
            );
        }
    }
}

pub(super) fn draw_thought_icon(anchor: Vec2, icon: ThoughtIcon, anim_tick: usize) {
    let (color, bits): (Color, [[u8; 3]; 3]) = match icon {
        ThoughtIcon::Flame => (
            Color::from_rgba(255, 146, 44, 255),
            [[0, 1, 0], [1, 1, 1], [1, 1, 1]],
        ),
        ThoughtIcon::Bucket => (
            Color::from_rgba(78, 174, 255, 255),
            [[1, 0, 1], [1, 1, 1], [1, 1, 1]],
        ),
        ThoughtIcon::Berry => (
            Color::from_rgba(236, 72, 88, 255),
            [[0, 1, 0], [1, 1, 1], [0, 1, 0]],
        ),
        ThoughtIcon::Hoe => (
            Color::from_rgba(186, 126, 68, 255),
            [[1, 1, 1], [0, 1, 0], [0, 1, 0]],
        ),
        ThoughtIcon::Axe => (
            Color::from_rgba(184, 194, 208, 255),
            [[1, 1, 0], [1, 1, 1], [0, 1, 0]],
        ),
        ThoughtIcon::Heart => (
            Color::from_rgba(255, 122, 168, 255),
            [[1, 0, 1], [1, 1, 1], [0, 1, 0]],
        ),
        ThoughtIcon::Warning => (
            Color::from_rgba(255, 218, 66, 255),
            [[0, 1, 0], [0, 1, 0], [0, 1, 0]],
        ),
    };

    let bob = if (anim_tick / 3).is_multiple_of(2) {
        0.0
    } else {
        1.0
    };
    let origin_x = snap_m(anchor.x - 1.5 * M);
    let origin_y = snap_m(anchor.y - 3.0 * M) - bob;

    for (ry, row) in bits.iter().enumerate() {
        for (rx, &on) in row.iter().enumerate() {
            if on != 0 {
                if icon == ThoughtIcon::Warning && ry == 1 {
                    draw_rectangle(
                        origin_x + rx as f32 * M,
                        origin_y + ry as f32 * M,
                        M,
                        0.65 * M,
                        color,
                    );
                } else {
                    draw_rectangle(
                        origin_x + rx as f32 * M,
                        origin_y + ry as f32 * M,
                        M,
                        M,
                        color,
                    );
                }
            }
        }
    }
}
