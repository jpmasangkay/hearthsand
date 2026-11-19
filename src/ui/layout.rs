use macroquad::prelude::*;

use crate::agents::Payload;
use crate::world::Preset;

use super::theme::{clamp_rect_to_window, place_tooltip, FONT_MD, FONT_SM, PAD};
use super::{PaletteCategory, PaletteEntry, SpeedMode};

pub const BTN_H: f32 = 26.0;
pub const TOP_H: f32 = 28.0;

pub const CONTROLS_ROWS: [(&str, &str); 11] = [
    ("LClick", "Paint material / Cast wand"),
    ("RDrag / WASD", "Pan camera / Move mage"),
    ("Mouse Wheel", "Zoom camera (0.25x snap)"),
    ("[ / ] keys", "Brush size (or click [-] [+])"),
    ("Home / F", "Center on village / Fit world"),
    ("Space", "Pause (God) / Dash (Mage)"),
    ("1 / 2 / 3 / 4", "Speed: Pause / 1x / 3x / 5x"),
    ("Tab / B", "Toggle God-Mage / Wand Builder"),
    ("V / T", "Cycle weather / Advance time"),
    ("P / R", "Next preset / Reset world seed"),
    ("F3 / ?", "Toggle full stats / Controls"),
];

#[derive(Clone, Debug)]
pub struct TopLeftLayout {
    pub bounds: Rect,
    pub play_pause: Rect,
    pub speeds: [(SpeedMode, Rect); 3],
}

#[derive(Clone, Debug)]
pub struct TopRightLayout {
    pub bounds: Rect,
    pub fps_box: Rect,
    pub menu_btn: Rect,
}

#[derive(Clone, Debug)]
pub struct MenuPanelLayout {
    pub panel: Rect,
    pub preset_label_y: f32,
    pub presets: [(Preset, Rect); 3],
    pub weather_btn: Rect,
    pub time_btn: Rect,
    pub mode_btn: Rect,
    pub wand_btn: Rect,
    pub snap_btn: Rect,
    pub reset_btn: Rect,
}

#[derive(Clone, Debug)]
pub struct StatsPanelLayout {
    pub panel: Rect,
    pub header_y: f32,
    pub first_row_y: f32,
    pub row_step: f32,
}

#[derive(Clone, Debug)]
pub struct WandDrawerLayout {
    pub panel: Rect,
    pub close_btn: Rect,
    pub header_y: f32,
    pub cost_y: f32,
    pub payload_label_y: f32,
    pub payloads: Vec<(Payload, Rect)>,
    pub slots_label_y: f32,
    pub clear_btn: Rect,
    pub slots: [Rect; 3],
    pub runes_label_y: f32,
    pub runes: Vec<Rect>,
    pub summary_y: f32,
    pub hint_y: f32,
}

#[derive(Clone, Debug)]
pub struct BottomGodLayout {
    pub panel: Rect,
    pub tabs: Vec<(PaletteCategory, Rect)>,
    pub brush_rect: Rect,
    pub brush_minus_btn: Rect,
    pub brush_plus_btn: Rect,
    pub help_btn: Rect,
    pub entries: Vec<(PaletteEntry, Rect)>,
}

#[derive(Clone, Debug)]
pub struct BottomMageLayout {
    pub panel: Rect,
    pub payload_btn: Rect,
    pub mana_bar: Rect,
    pub wand_btn: Rect,
    pub slots: [Rect; 3],
    pub dash_right_x: f32,
    pub dash_y: f32,
    pub help_btn: Rect,
}

#[derive(Clone, Debug)]
pub struct ControlsModalLayout {
    pub panel: Rect,
    pub close_btn: Rect,
    pub header_y: f32,
    pub key_col_x: f32,
    pub desc_col_x: f32,
    pub first_row_y: f32,
    pub row_step: f32,
}

pub fn layout_top_left<F: Fn(&str, u16) -> f32>(sw: f32, sh: f32, measure_w: &F) -> TopLeftLayout {
    let play_w = measure_w("Pause", FONT_SM).max(measure_w("Play", FONT_SM)) + PAD * 2.0;
    let seg_w = measure_w("5x", FONT_SM) + PAD * 2.0;
    let total_w = play_w + PAD + seg_w * 3.0;
    let bounds = clamp_rect_to_window(Rect::new(PAD, PAD, total_w, TOP_H), sw, sh, PAD);
    let play_pause = Rect::new(bounds.x, bounds.y, play_w.min(bounds.w), bounds.h);
    let seg_x = play_pause.right() + PAD;
    let speeds = [
        (
            SpeedMode::Normal,
            Rect::new(seg_x, bounds.y, seg_w, bounds.h),
        ),
        (
            SpeedMode::Fast,
            Rect::new(seg_x + seg_w, bounds.y, seg_w, bounds.h),
        ),
        (
            SpeedMode::Turbo,
            Rect::new(seg_x + seg_w * 2.0, bounds.y, seg_w, bounds.h),
        ),
    ];
    TopLeftLayout {
        bounds,
        play_pause,
        speeds,
    }
}

pub fn layout_top_right<F: Fn(&str, u16) -> f32>(
    sw: f32,
    sh: f32,
    fps_text: &str,
    measure_w: &F,
) -> TopRightLayout {
    let menu_w = measure_w("Menu", FONT_SM) + PAD * 2.0;
    let fps_w = measure_w(fps_text, FONT_SM) + PAD * 2.0;
    let total_w = fps_w + PAD + menu_w;
    let bounds = clamp_rect_to_window(
        Rect::new(sw - PAD - total_w, PAD, total_w, TOP_H),
        sw,
        sh,
        PAD,
    );
    let fps_box = Rect::new(bounds.x, bounds.y, fps_w.min(bounds.w), bounds.h);
    let menu_btn = Rect::new(
        (bounds.right() - menu_w).max(bounds.x),
        bounds.y,
        menu_w.min(bounds.w),
        bounds.h,
    );
    TopRightLayout {
        bounds,
        fps_box,
        menu_btn,
    }
}

pub fn layout_menu_panel<F: Fn(&str, u16) -> f32>(
    sw: f32,
    sh: f32,
    top_right: &TopRightLayout,
    labels: &[&str],
    measure_w: &F,
) -> MenuPanelLayout {
    let mut max_w: f32 = 220.0;
    for &p in &Preset::ALL {
        max_w = max_w.max(measure_w(p.name(), FONT_SM) + PAD * 4.0);
    }
    for &lbl in labels {
        max_w = max_w.max(measure_w(lbl, FONT_SM) + PAD * 4.0);
    }
    // Layout: PAD + label(16) + PAD + 3*BTN_H + 2*PAD + PAD + 6*BTN_H + 5*PAD + PAD
    let total_h = PAD + 16.0 + PAD + 3.0 * BTN_H + 2.0 * PAD + PAD + 6.0 * BTN_H + 5.0 * PAD + PAD;
    let raw = Rect::new(
        top_right.menu_btn.right() - max_w,
        top_right.menu_btn.bottom() + PAD,
        max_w,
        total_h,
    );
    let panel = clamp_rect_to_window(raw, sw, sh, PAD);
    let bx = panel.x + PAD;
    let bw = (panel.w - PAD * 2.0).max(1.0);
    let preset_label_y = panel.y + PAD + 13.0;
    let mut y = panel.y + PAD + 16.0 + PAD;

    let presets = [
        (Preset::ALL[0], Rect::new(bx, y, bw, BTN_H)),
        (Preset::ALL[1], Rect::new(bx, y + BTN_H + PAD, bw, BTN_H)),
        (
            Preset::ALL[2],
            Rect::new(bx, y + 2.0 * (BTN_H + PAD), bw, BTN_H),
        ),
    ];
    y += 3.0 * BTN_H + 3.0 * PAD;

    let weather_btn = Rect::new(bx, y, bw, BTN_H);
    y += BTN_H + PAD;
    let time_btn = Rect::new(bx, y, bw, BTN_H);
    y += BTN_H + PAD;
    let mode_btn = Rect::new(bx, y, bw, BTN_H);
    y += BTN_H + PAD;
    let wand_btn = Rect::new(bx, y, bw, BTN_H);
    y += BTN_H + PAD;
    let snap_btn = Rect::new(bx, y, bw, BTN_H);
    y += BTN_H + PAD;
    let reset_btn = Rect::new(bx, y, bw, BTN_H);

    MenuPanelLayout {
        panel,
        preset_label_y,
        presets,
        weather_btn,
        time_btn,
        mode_btn,
        wand_btn,
        snap_btn,
        reset_btn,
    }
}

pub fn layout_stats_panel<F: Fn(&str, u16) -> f32>(
    sw: f32,
    sh: f32,
    top_right: &TopRightLayout,
    menu_rect: Option<Rect>,
    rows: &[(&str, String)],
    measure_w: &F,
) -> StatsPanelLayout {
    let header_w = measure_w("STATS [F3]", FONT_MD) + PAD * 2.0;
    let mut max_w = header_w.max(204.0);
    for (k, v) in rows {
        let row_w = measure_w(k, FONT_SM) + PAD * 3.0 + measure_w(v, FONT_SM) + PAD * 2.0;
        max_w = max_w.max(row_w);
    }
    let row_step = 20.0;
    let total_h = PAD + 20.0 + PAD + rows.len() as f32 * row_step + PAD;
    let anchor_right = match menu_rect {
        Some(mr) => mr.x - PAD,
        None => top_right.menu_btn.right(),
    };
    let raw = Rect::new(
        anchor_right - max_w,
        top_right.menu_btn.bottom() + PAD,
        max_w,
        total_h,
    );
    let panel = clamp_rect_to_window(raw, sw, sh, PAD);
    StatsPanelLayout {
        panel,
        header_y: panel.y + PAD + 15.0,
        first_row_y: panel.y + PAD + 20.0 + PAD + 13.0,
        row_step,
    }
}

pub fn layout_wand_drawer<F: Fn(&str, u16) -> f32>(
    sw: f32,
    sh: f32,
    top_left: &TopLeftLayout,
    measure_w: &F,
) -> WandDrawerLayout {
    let panel_w = 328.0_f32.max(measure_w("WAND BUILDER", FONT_MD) + 80.0);
    // PAD + header(24) + PAD + cost(16) + PAD + payload_lbl(16) + PAD + 2*BTN_H + PAD
    // + PAD + slots_hdr(24) + PAD + BTN_H + PAD + runes_lbl(16) + PAD + 2*BTN_H + PAD
    // + PAD + summary(16) + PAD + hint(16) + PAD
    let total_h = PAD
        + 24.0
        + PAD
        + 16.0
        + PAD
        + 16.0
        + PAD
        + 2.0 * BTN_H
        + PAD
        + PAD
        + 24.0
        + PAD
        + BTN_H
        + PAD
        + 16.0
        + PAD
        + 2.0 * BTN_H
        + PAD
        + PAD
        + 16.0
        + PAD
        + 16.0
        + PAD;
    let panel = clamp_rect_to_window(
        Rect::new(PAD, top_left.bounds.bottom() + PAD, panel_w, total_h),
        sw,
        sh,
        PAD,
    );

    let inner_x = panel.x + PAD;
    let inner_w = (panel.w - PAD * 2.0).max(1.0);
    let col_w = ((inner_w - PAD * 2.0) / 3.0).floor().max(1.0);

    let mut y = panel.y + PAD;
    let header_y = y + 17.0;
    let close_w = measure_w("X", FONT_SM) + PAD * 2.0;
    let close_btn = Rect::new(panel.right() - PAD - close_w, y, close_w, 24.0);
    y += 24.0 + PAD;

    let cost_y = y + 13.0;
    y += 16.0 + PAD;

    let payload_label_y = y + 13.0;
    y += 16.0 + PAD;

    let mut payloads = Vec::with_capacity(Payload::ALL.len());
    for (idx, &p) in Payload::ALL.iter().enumerate() {
        let col = (idx % 3) as f32;
        let row = (idx / 3) as f32;
        let rx = inner_x + col * (col_w + PAD);
        let ry = y + row * (BTN_H + PAD);
        payloads.push((p, Rect::new(rx, ry, col_w, BTN_H)));
    }
    y += 2.0 * BTN_H + PAD * 2.0;

    let slots_label_y = y + 16.0;
    let clear_w = measure_w("Clear", FONT_SM) + PAD * 2.0;
    let clear_btn = Rect::new(panel.right() - PAD - clear_w, y, clear_w, 24.0);
    y += 24.0 + PAD;

    let slots = [
        Rect::new(inner_x, y, col_w, BTN_H),
        Rect::new(inner_x + col_w + PAD, y, col_w, BTN_H),
        Rect::new(inner_x + 2.0 * (col_w + PAD), y, col_w, BTN_H),
    ];
    y += BTN_H + PAD;

    let runes_label_y = y + 13.0;
    y += 16.0 + PAD;

    let mut runes = Vec::with_capacity(5);
    for idx in 0..5 {
        let col = (idx % 3) as f32;
        let row = (idx / 3) as f32;
        let rx = inner_x + col * (col_w + PAD);
        let ry = y + row * (BTN_H + PAD);
        runes.push(Rect::new(rx, ry, col_w, BTN_H));
    }
    y += 2.0 * BTN_H + PAD * 2.0;

    let summary_y = y + 13.0;
    let hint_y = y + 16.0 + PAD + 13.0;

    WandDrawerLayout {
        panel,
        close_btn,
        header_y,
        cost_y,
        payload_label_y,
        payloads,
        slots_label_y,
        clear_btn,
        slots,
        runes_label_y,
        runes,
        summary_y,
        hint_y,
    }
}

pub fn layout_bottom_god<F: Fn(&str, u16) -> f32>(
    sw: f32,
    sh: f32,
    active_tab: PaletteCategory,
    brush_text: &str,
    measure_w: &F,
) -> BottomGodLayout {
    let panel_w = (sw - PAD * 2.0).max(1.0);
    let inner_w = (panel_w - PAD * 2.0).max(1.0);

    let help_w = BTN_H;
    let step_btn_w = BTN_H;
    let brush_w = measure_w(brush_text, FONT_SM) + PAD * 2.0;
    let right_reserved = brush_w + PAD + step_btn_w + PAD + step_btn_w + PAD + help_w + PAD;
    let tabs_max_w = (inner_w - right_reserved).max(120.0);

    // Lay out tabs relative to panel top-left, wrapping if window is narrow
    let mut tab_rel: Vec<(PaletteCategory, Rect)> = Vec::with_capacity(PaletteCategory::ALL.len());
    let mut tx = PAD;
    let mut ty = PAD;
    for &cat in &PaletteCategory::ALL {
        let tw = (measure_w(cat.label(), FONT_SM) + PAD * 2.0).min(inner_w);
        let row_limit = if ty <= PAD + 1.0 {
            PAD + tabs_max_w
        } else {
            PAD + inner_w
        };
        if tx > PAD && tx + tw > row_limit {
            tx = PAD;
            ty += BTN_H + PAD;
        }
        tab_rel.push((cat, Rect::new(tx, ty, tw, BTN_H)));
        tx += tw + PAD;
    }
    let tabs_bottom = ty + BTN_H;

    // Lay out wrapping color-filled element/spell TEXT buttons below the tabs
    let mut entry_rel: Vec<(PaletteEntry, Rect)> = Vec::new();
    let mut ex = PAD;
    let mut ey = tabs_bottom + PAD;
    for &entry in active_tab.entries() {
        let ew = (measure_w(entry.label(), FONT_SM) + PAD * 2.0).min(inner_w);
        if ex > PAD && ex + ew > PAD + inner_w {
            ex = PAD;
            ey += BTN_H + PAD;
        }
        entry_rel.push((entry, Rect::new(ex, ey, ew, BTN_H)));
        ex += ew + PAD;
    }
    let total_h = ey + BTN_H + PAD;

    let panel = clamp_rect_to_window(
        Rect::new(PAD, sh - PAD - total_h, panel_w, total_h),
        sw,
        sh,
        PAD,
    );

    let tabs = tab_rel
        .into_iter()
        .map(|(cat, r)| (cat, Rect::new(panel.x + r.x, panel.y + r.y, r.w, r.h)))
        .collect();
    let entries = entry_rel
        .into_iter()
        .map(|(e, r)| (e, Rect::new(panel.x + r.x, panel.y + r.y, r.w, r.h)))
        .collect();

    let help_btn = Rect::new(panel.right() - PAD - help_w, panel.y + PAD, help_w, BTN_H);
    let brush_plus_btn = Rect::new(
        help_btn.x - PAD - step_btn_w,
        panel.y + PAD,
        step_btn_w,
        BTN_H,
    );
    let brush_minus_btn = Rect::new(
        brush_plus_btn.x - PAD - step_btn_w,
        panel.y + PAD,
        step_btn_w,
        BTN_H,
    );
    let brush_rect = Rect::new(
        brush_minus_btn.x - PAD - brush_w,
        panel.y + PAD,
        brush_w,
        BTN_H,
    );

    BottomGodLayout {
        panel,
        tabs,
        brush_rect,
        brush_minus_btn,
        brush_plus_btn,
        help_btn,
        entries,
    }
}

pub fn layout_bottom_mage<F: Fn(&str, u16) -> f32>(
    sw: f32,
    sh: f32,
    payload_label: &str,
    measure_w: &F,
) -> BottomMageLayout {
    let help_btn = clamp_rect_to_window(
        Rect::new(sw - PAD - BTN_H, sh - PAD - BTN_H, BTN_H, BTN_H),
        sw,
        sh,
        PAD,
    );
    let max_center_w = (sw - PAD * 2.0 - (BTN_H + PAD) * 2.0).max(260.0);
    let panel_w = 468.0_f32.min(max_center_w);
    let panel_h = PAD + BTN_H + PAD + BTN_H + PAD;
    let panel = clamp_rect_to_window(
        Rect::new((sw - panel_w) * 0.5, sh - PAD - panel_h, panel_w, panel_h),
        sw,
        sh,
        PAD,
    );

    let inner_x = panel.x + PAD;
    let inner_w = (panel.w - PAD * 2.0).max(1.0);
    let row1_y = panel.y + PAD;
    let row2_y = row1_y + BTN_H + PAD;

    let payload_w = (measure_w(payload_label, FONT_SM) + PAD * 2.0).min(inner_w * 0.35);
    let wand_w = (measure_w("Wand [B]", FONT_SM) + PAD * 2.0).min(inner_w * 0.35);
    let payload_btn = Rect::new(inner_x, row1_y, payload_w, BTN_H);
    let wand_btn = Rect::new(panel.right() - PAD - wand_w, row1_y, wand_w, BTN_H);
    let mana_x = payload_btn.right() + PAD;
    let mana_w = (wand_btn.x - PAD - mana_x).max(40.0);
    let mana_bar = Rect::new(mana_x, row1_y, mana_w, BTN_H);

    let dash_w = measure_w("Dash CD", FONT_SM) + PAD;
    let slots_total_w = (inner_w - dash_w - PAD).max(120.0);
    let slot_w = ((slots_total_w - PAD * 2.0) / 3.0).floor().max(32.0);
    let slots = [
        Rect::new(inner_x, row2_y, slot_w, BTN_H),
        Rect::new(inner_x + slot_w + PAD, row2_y, slot_w, BTN_H),
        Rect::new(inner_x + 2.0 * (slot_w + PAD), row2_y, slot_w, BTN_H),
    ];

    BottomMageLayout {
        panel,
        payload_btn,
        mana_bar,
        wand_btn,
        slots,
        dash_right_x: panel.right() - PAD,
        dash_y: row2_y + 18.0,
        help_btn,
    }
}

pub fn layout_controls_modal<F: Fn(&str, u16) -> f32>(
    sw: f32,
    sh: f32,
    measure_w: &F,
) -> ControlsModalLayout {
    let mut key_w: f32 = 100.0;
    let mut desc_w: f32 = 220.0;
    for (k, d) in &CONTROLS_ROWS {
        key_w = key_w.max(measure_w(k, FONT_SM));
        desc_w = desc_w.max(measure_w(d, FONT_SM));
    }
    let panel_w = (PAD + key_w + PAD * 2.0 + desc_w + PAD).max(380.0);
    let row_step = 22.0;
    let panel_h = PAD + 24.0 + PAD + CONTROLS_ROWS.len() as f32 * row_step + PAD;
    let panel = clamp_rect_to_window(
        Rect::new((sw - panel_w) * 0.5, (sh - panel_h) * 0.5, panel_w, panel_h),
        sw,
        sh,
        PAD,
    );
    let close_w = measure_w("X", FONT_SM) + PAD * 2.0;
    let close_btn = Rect::new(panel.right() - PAD - close_w, panel.y + PAD, close_w, 24.0);
    ControlsModalLayout {
        panel,
        close_btn,
        header_y: panel.y + PAD + 17.0,
        key_col_x: panel.x + PAD,
        desc_col_x: panel.x + PAD + key_w + PAD * 2.0,
        first_row_y: panel.y + PAD + 24.0 + PAD + 14.0,
        row_step,
    }
}

pub fn layout_tooltip<F: Fn(&str, u16) -> f32>(
    cursor: Vec2,
    lines: &[&str],
    sw: f32,
    sh: f32,
    measure_w: &F,
) -> Rect {
    let mut max_w: f32 = 100.0;
    for line in lines {
        max_w = max_w.max(measure_w(line, FONT_SM));
    }
    let w = max_w + PAD * 2.0;
    let h = PAD + lines.len() as f32 * 18.0 + PAD;
    place_tooltip(cursor, vec2(w, h), sw, sh, PAD)
}
