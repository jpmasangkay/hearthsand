use macroquad::prelude::*;

use crate::world::{World, TILE_PX};

use super::figures::{
    classify_tall_prop, draw_critter_figure, draw_mage_figure, draw_mage_ghost_figure,
    draw_projectile_figure, draw_tall_prop, draw_thought_icon, draw_villager_figure, snap_m,
    CritterDraw, MageDraw, ProjVisual, PropKind, VillagerDraw, M, SHADOW_STRIP,
};
use super::{Agents, CritterKind, ItemKind, ProjectileKind, ThoughtIcon};

const THOUGHT_MIN_ZOOM: f32 = 1.15;

pub struct DrawSceneOpts {
    pub view_min: (usize, usize),
    pub view_max: (usize, usize),
    pub anim_tick: usize,
    pub cam_zoom: f32,
    pub night_rim: f32,
}

impl Agents {
    pub fn draw_sorted(&self, world: &World, opts: DrawSceneOpts) {
        enum DrawItem {
            Prop {
                y_sort: f32,
                x: usize,
                y: usize,
                kind: PropKind,
                variant: u8,
            },
            Villager {
                y_sort: f32,
                id: usize,
                pos: Vec2,
                dir: u8,
                step: usize,
                bob: f32,
                hit_flash: bool,
                bucket: bool,
                carrying: Option<ItemKind>,
            },
            Critter {
                y_sort: f32,
                kind: CritterKind,
                pos: Vec2,
                flip_x: bool,
                step: usize,
                bob: f32,
                stunned: bool,
            },
            MageGhost {
                y_sort: f32,
                pos: Vec2,
                dir: u8,
                tint: Color,
            },
            Mage {
                y_sort: f32,
                pos: Vec2,
                dir: u8,
                step: usize,
                bob: f32,
                aim_dir: Vec2,
                wand_color: Color,
                hit_flash: bool,
                cast_flash: bool,
            },
            Drop {
                y_sort: f32,
                pos: Vec2,
                height: f32,
                color: Color,
            },
            Projectile {
                y_sort: f32,
                pos: Vec2,
                visual: ProjVisual,
            },
            Debris {
                y_sort: f32,
                pos: Vec2,
                height: f32,
                color: Color,
            },
        }

        impl DrawItem {
            fn y_sort(&self) -> f32 {
                match *self {
                    DrawItem::Prop { y_sort, .. }
                    | DrawItem::Villager { y_sort, .. }
                    | DrawItem::Critter { y_sort, .. }
                    | DrawItem::MageGhost { y_sort, .. }
                    | DrawItem::Mage { y_sort, .. }
                    | DrawItem::Drop { y_sort, .. }
                    | DrawItem::Projectile { y_sort, .. }
                    | DrawItem::Debris { y_sort, .. } => y_sort,
                }
            }
        }

        let twilight = world.time_of_day < 0.28 || world.time_of_day >= 0.64;
        let mut items = Vec::with_capacity(320);

        for y in opts.view_min.1..opts.view_max.1 {
            for x in opts.view_min.0..opts.view_max.0 {
                if let Some(cell) = world.get(x as i32, y as i32) {
                    if let Some(kind) = classify_tall_prop(cell, twilight) {
                        items.push(DrawItem::Prop {
                            y_sort: (y + 1) as f32 * TILE_PX,
                            x,
                            y,
                            kind,
                            variant: cell.variant,
                        });
                    }
                }
            }
        }

        let min_px_x = opts.view_min.0 as f32 * TILE_PX - 32.0;
        let max_px_x = opts.view_max.0 as f32 * TILE_PX + 32.0;
        let min_px_y = opts.view_min.1 as f32 * TILE_PX - 32.0;
        let max_px_y = opts.view_max.1 as f32 * TILE_PX + 32.0;
        let in_view =
            |p: Vec2| p.x >= min_px_x && p.x <= max_px_x && p.y >= min_px_y && p.y <= max_px_y;

        for g in &self.mage_ghosts {
            if in_view(g.pos) {
                items.push(DrawItem::MageGhost {
                    y_sort: g.pos.y + 5.8,
                    pos: g.pos,
                    dir: g.dir,
                    tint: Color::new(
                        g.tint.r * 0.6 + 0.35,
                        g.tint.g * 0.6 + 0.4,
                        g.tint.b * 0.5 + 0.5,
                        g.alpha * 0.65,
                    ),
                });
            }
        }

        if let Some(mp) = self.mage_pos {
            if in_view(mp) {
                let moving = self.mage.vel.length_squared() > 1.0;
                let step = if moving {
                    (self.mage.anim_time as usize) % 2
                } else {
                    0
                };
                let bob = if moving {
                    (self.mage.anim_time * 1.5).sin().abs() * 2.0
                } else {
                    0.0
                };
                items.push(DrawItem::Mage {
                    y_sort: mp.y + 6.0,
                    pos: mp,
                    dir: self.mage_dir,
                    step,
                    bob,
                    aim_dir: self.mage.aim_dir,
                    wand_color: self.mage.wand.payload.color(),
                    hit_flash: self.mage.hit_flash > 0.0,
                    cast_flash: self.mage.action_tween > 0.12,
                });
            }
        }

        let mut thoughts: Vec<(Vec2, ThoughtIcon)> = Vec::new();

        for v in &self.villagers {
            if !in_view(v.pos) {
                continue;
            }
            let moving = v.vel.length_squared() > 1.0;
            let step = if moving {
                (v.anim_time as usize) % 2
            } else {
                0
            };
            let bob = if moving {
                (v.anim_time * 1.5).sin().abs() * 2.0
            } else {
                0.0
            };
            items.push(DrawItem::Villager {
                y_sort: v.pos.y + 6.0,
                id: v.id,
                pos: v.pos,
                dir: v.dir,
                step,
                bob,
                hit_flash: v.hit_flash > 0.0,
                bucket: v.bucket_water > 0,
                carrying: v.carrying,
            });
            if opts.cam_zoom >= THOUGHT_MIN_ZOOM {
                if let Some(icon) = v.thought {
                    thoughts.push((vec2(v.pos.x, v.pos.y - bob - 5.5 * M), icon));
                }
            }
        }

        for c in &self.critters {
            if !in_view(c.pos) {
                continue;
            }
            let step = (c.anim_time as usize) % 2;
            let bob = if c.kind == CritterKind::Rabbit {
                c.hop_height
            } else if c.vel.length_squared() > 1.0 {
                (c.anim_time * 2.0).sin().abs() * 1.5
            } else {
                0.0
            };
            items.push(DrawItem::Critter {
                y_sort: c.pos.y + 5.0,
                kind: c.kind,
                pos: c.pos,
                flip_x: c.dir == 2,
                step,
                bob,
                stunned: c.stun_timer > 0.0,
            });
        }

        for d in &self.drops {
            if !in_view(d.pos) {
                continue;
            }
            let idle_bob = (d.age * 5.0).sin().abs() * 1.5;
            items.push(DrawItem::Drop {
                y_sort: d.pos.y + 2.0,
                pos: d.pos,
                height: d.height + idle_bob,
                color: d.kind.color(),
            });
        }

        for p in &self.projectiles {
            if !in_view(p.pos) {
                continue;
            }
            let visual = match p.kind {
                ProjectileKind::Bomb => ProjVisual::Bomb,
            };
            items.push(DrawItem::Projectile {
                y_sort: p.pos.y + 4.0,
                pos: p.pos,
                visual,
            });
        }

        for s in &self.spells {
            if in_view(s.pos) {
                items.push(DrawItem::Projectile {
                    y_sort: s.pos.y + 4.5,
                    pos: s.pos,
                    visual: ProjVisual::Spell(s.payload),
                });
            }
        }

        for orb in &self.orbit_shards {
            let opos = self.mage.pos + vec2(orb.angle.cos(), orb.angle.sin()) * orb.radius;
            if in_view(opos) {
                items.push(DrawItem::Projectile {
                    y_sort: opos.y + 4.5,
                    pos: opos,
                    visual: ProjVisual::Spell(orb.payload),
                });
            }
        }

        for d in &world.debris {
            if !in_view(d.pos) {
                continue;
            }
            items.push(DrawItem::Debris {
                y_sort: d.pos.y + 2.0,
                pos: d.pos,
                height: d.height,
                color: d.color,
            });
        }

        items.sort_by(|a, b| a.y_sort().total_cmp(&b.y_sort()));

        for item in items {
            match item {
                DrawItem::Prop {
                    x,
                    y,
                    kind,
                    variant,
                    ..
                } => {
                    draw_tall_prop(x, y, kind, variant, opts.anim_tick);
                }
                DrawItem::Villager {
                    id,
                    pos,
                    dir,
                    step,
                    bob,
                    hit_flash,
                    bucket,
                    carrying,
                    ..
                } => {
                    draw_villager_figure(VillagerDraw {
                        id,
                        pos,
                        dir,
                        step,
                        bob,
                        hit_flash,
                        bucket,
                        carrying,
                        night_rim: opts.night_rim,
                    });
                }
                DrawItem::Critter {
                    kind,
                    pos,
                    flip_x,
                    step,
                    bob,
                    stunned,
                    ..
                } => {
                    draw_critter_figure(CritterDraw {
                        kind,
                        pos,
                        flip_x,
                        step,
                        bob,
                        stunned,
                        anim_tick: opts.anim_tick,
                        is_night: world.is_night(),
                    });
                }
                DrawItem::MageGhost { pos, dir, tint, .. } => {
                    draw_mage_ghost_figure(pos, dir, tint);
                }
                DrawItem::Mage {
                    pos,
                    dir,
                    step,
                    bob,
                    aim_dir,
                    wand_color,
                    hit_flash,
                    cast_flash,
                    ..
                } => {
                    draw_mage_figure(MageDraw {
                        pos,
                        dir,
                        step,
                        bob,
                        aim_dir,
                        wand_color,
                        hit_flash,
                        cast_flash,
                        night_rim: opts.night_rim,
                    });
                }
                DrawItem::Drop {
                    pos, height, color, ..
                } => {
                    let bx = snap_m(pos.x - 0.5 * M);
                    let by = snap_m(pos.y);
                    draw_rectangle(bx, by + 0.5 * M, M, 0.5 * M, SHADOW_STRIP);
                    draw_rectangle(bx, by - height - 0.5 * M, M, M, color);
                }
                DrawItem::Projectile { pos, visual, .. } => {
                    draw_projectile_figure(pos, visual, opts.anim_tick);
                }
                DrawItem::Debris {
                    pos, height, color, ..
                } => {
                    let bx = snap_m(pos.x - 0.5 * M);
                    let by = snap_m(pos.y);
                    draw_rectangle(bx, by, M, 0.5 * M, SHADOW_STRIP);
                    draw_rectangle(bx, by - height - 0.5 * M, M, M, color);
                }
            }
        }

        // Gravity vortex rings & swirling arms over world layer
        for v in &self.vortices {
            if !in_view(v.pos) {
                continue;
            }
            let r_outer = v.radius * 0.45 + ((opts.anim_tick % 4) as f32) * 1.8;
            draw_circle_lines(
                v.pos.x,
                v.pos.y,
                r_outer,
                1.5,
                Color::from_rgba(176, 108, 255, 190),
            );
            draw_circle_lines(
                v.pos.x,
                v.pos.y,
                (r_outer * 0.55).max(4.0),
                1.5,
                Color::from_rgba(232, 196, 255, 220),
            );
            let spin = opts.anim_tick as f32 * 0.65;
            for arm in 0..3 {
                let a0 = spin + arm as f32 * (std::f32::consts::TAU / 3.0);
                let a1 = a0 + 0.55;
                let p_in = v.pos + vec2(a0.cos(), a0.sin()) * (r_outer * 0.35);
                let p_out = v.pos + vec2(a1.cos(), a1.sin()) * (r_outer * 0.92);
                draw_line(
                    p_in.x,
                    p_in.y,
                    p_out.x,
                    p_out.y,
                    1.2,
                    Color::from_rgba(214, 156, 255, 185),
                );
            }
        }

        for (arc_idx, arc) in self.lightning_arcs.iter().enumerate() {
            let ratio = (arc.ttl / arc.max_ttl.max(0.05)).clamp(0.0, 1.0);
            let outer = Color::new(arc.color.r, arc.color.g, arc.color.b, ratio * 0.92);
            let core = Color::new(1.0, 1.0, 1.0, ratio);
            for (seg_idx, w) in arc.points.windows(2).enumerate() {
                let p0 = w[0];
                let p1 = w[1];
                let delta = p1 - p0;
                let len = delta.length().max(1.0);
                let perp = vec2(-delta.y / len, delta.x / len);
                let amp = (len * 0.14).clamp(2.5, 9.0);
                let h1 = ((opts.anim_tick + seg_idx * 7 + arc_idx * 13) % 5) as f32 - 2.0;
                let h2 = ((opts.anim_tick * 3 + seg_idx * 11 + arc_idx * 5) % 5) as f32 - 2.0;
                let q1 = p0.lerp(p1, 0.33) + perp * (h1 * amp * 0.45);
                let q2 = p0.lerp(p1, 0.67) + perp * (h2 * amp * 0.45);
                let pts = [p0, q1, q2, p1];
                for pair in pts.windows(2) {
                    draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, 2.6, outer);
                    draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, 1.1, core);
                }
            }
        }

        // 3x3 micro-pixel colored thought icons directly above villager heads (no white box)
        for (anchor, icon) in thoughts {
            draw_thought_icon(anchor, icon, opts.anim_tick);
        }
    }
}
