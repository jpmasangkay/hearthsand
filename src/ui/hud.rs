use macroquad::prelude::*;

use crate::agents::{Agents, Rune, Wand};
use crate::world::World;

use super::layout::{
    layout_bottom_god, layout_bottom_mage, layout_controls_modal, layout_menu_panel,
    layout_stats_panel, layout_tooltip, layout_top_left, layout_top_right, layout_wand_drawer,
    BottomGodLayout, BottomMageLayout, ControlsModalLayout, MenuPanelLayout, StatsPanelLayout,
    TopLeftLayout, TopRightLayout, WandDrawerLayout, CONTROLS_ROWS,
};
use super::theme::{
    button_text_baseline_y, draw_color_button, draw_panel, draw_ui_button, draw_ui_text,
    draw_ui_text_right, measure_ui_width, COL_ACCENT, COL_BORDER, COL_SURFACE, COL_TEXT, FONT_MD,
    FONT_SM, PAD,
};
use super::{wind_compass_label, PaletteEntry, SpeedMode, UiAction, UiState};

pub struct HudLayoutSnapshot {
    pub top_left: TopLeftLayout,
    pub top_right: TopRightLayout,
    pub menu: Option<MenuPanelLayout>,
    pub stats: Option<StatsPanelLayout>,
    pub wand: Option<WandDrawerLayout>,
    pub bottom_god: Option<BottomGodLayout>,
    pub bottom_mage: Option<BottomMageLayout>,
    pub controls: Option<ControlsModalLayout>,
    pub fps_text: String,
    pub brush_text: String,
    pub stats_rows: Vec<(&'static str, String)>,
    pub menu_labels: MenuLabels,
}

pub struct MenuLabels {
    pub weather: String,
    pub time: String,
    pub mode: &'static str,
    pub wand: &'static str,
    pub snap: &'static str,
    pub reset: &'static str,
}

impl HudLayoutSnapshot {
    pub fn contains_point(&self, pt: Vec2) -> bool {
        if self.top_left.bounds.contains(pt) || self.top_right.bounds.contains(pt) {
            return true;
        }
        if let Some(m) = &self.menu {
            if m.panel.contains(pt) {
                return true;
            }
        }
        if let Some(s) = &self.stats {
            if s.panel.contains(pt) {
                return true;
            }
        }
        if let Some(w) = &self.wand {
            if w.panel.contains(pt) {
                return true;
            }
        }
        if let Some(bg) = &self.bottom_god {
            if bg.panel.contains(pt) {
                return true;
            }
        }
        if let Some(bm) = &self.bottom_mage {
            if bm.panel.contains(pt) || bm.help_btn.contains(pt) {
                return true;
            }
        }
        if let Some(c) = &self.controls {
            if c.panel.contains(pt) {
                return true;
            }
        }
        false
    }
}

pub fn build_stats_rows(fps: i32, world: &World, agents: &Agents) -> Vec<(&'static str, String)> {
    let particles =
        world.gas.len() + world.debris.len() + agents.drops.len() + agents.active_magic_count();
    vec![
        ("FPS", format!("{fps}")),
        ("Villagers", format!("{}", agents.villagers.len())),
        ("Crop yield", format!("{}", agents.total_crops_harvested)),
        ("Wood", format!("{}", agents.wood_stored)),
        ("Food", format!("{}", agents.food_stored)),
        (
            "Active chunks",
            format!("{}/{}", world.active_chunk_count(), world.chunk_awake.len()),
        ),
        ("Particles", format!("{particles}")),
        ("Wind", wind_compass_label(world.wind)),
        ("Seed", format!("{}", world.seed)),
    ]
}

pub fn compute_hud_layout<F: Fn(&str, u16) -> f32>(
    sw: f32,
    sh: f32,
    fps: i32,
    world: &World,
    agents: &Agents,
    ui: &UiState,
    measure_w: &F,
) -> HudLayoutSnapshot {
    let fps_text = format!("{fps} FPS");
    let brush_text = format!("Brush {}", ui.brush_radius);
    let menu_labels = MenuLabels {
        weather: format!("Weather: {}", world.weather.name()),
        time: format!("Time: {} [T]", world.day_phase_name()),
        mode: if agents.mage_active {
            "Mode: Mage [Tab]"
        } else {
            "Mode: God [Tab]"
        },
        wand: "Wand Builder [B]",
        snap: if ui.snap_zoom {
            "Snap Zoom (0.25x): ON"
        } else {
            "Snap Zoom (0.25x): OFF"
        },
        reset: "Reset World [R]",
    };

    let top_left = layout_top_left(sw, sh, measure_w);
    let top_right = layout_top_right(sw, sh, &fps_text, measure_w);

    let menu = if ui.menu_open {
        let label_refs = [
            menu_labels.weather.as_str(),
            menu_labels.time.as_str(),
            menu_labels.mode,
            menu_labels.wand,
            menu_labels.snap,
            menu_labels.reset,
        ];
        Some(layout_menu_panel(
            sw,
            sh,
            &top_right,
            &label_refs,
            measure_w,
        ))
    } else {
        None
    };

    let stats_rows = if ui.stats_open {
        build_stats_rows(fps, world, agents)
    } else {
        Vec::new()
    };
    let stats = if ui.stats_open {
        Some(layout_stats_panel(
            sw,
            sh,
            &top_right,
            menu.as_ref().map(|m| m.panel),
            &stats_rows,
            measure_w,
        ))
    } else {
        None
    };

    let wand = if ui.wand_open {
        Some(layout_wand_drawer(sw, sh, &top_left, measure_w))
    } else {
        None
    };

    let (bottom_god, bottom_mage) = if agents.mage_active {
        (
            None,
            Some(layout_bottom_mage(
                sw,
                sh,
                agents.mage.wand.payload.short_name(),
                measure_w,
            )),
        )
    } else {
        (
            Some(layout_bottom_god(
                sw,
                sh,
                ui.active_tab,
                &brush_text,
                measure_w,
            )),
            None,
        )
    };

    let controls = if ui.controls_open {
        Some(layout_controls_modal(sw, sh, measure_w))
    } else {
        None
    };

    HudLayoutSnapshot {
        top_left,
        top_right,
        menu,
        stats,
        wand,
        bottom_god,
        bottom_mage,
        controls,
        fps_text,
        brush_text,
        stats_rows,
        menu_labels,
    }
}

pub fn draw_hud(
    snap: &HudLayoutSnapshot,
    speed: &mut SpeedMode,
    world: &World,
    agents: &mut Agents,
    ui: &mut UiState,
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let mouse = Vec2::from(mouse_position());
    let clicked = is_mouse_button_pressed(MouseButton::Left);

    // Top-left: Play/Pause + 1x/3x/5x segmented control
    let paused = *speed == SpeedMode::Pause;
    let play_label = if paused { "Play" } else { "Pause" };
    if draw_ui_button(snap.top_left.play_pause, play_label, FONT_SM, paused) {
        if paused {
            *speed = ui.last_run_speed;
        } else {
            ui.last_run_speed = *speed;
            *speed = SpeedMode::Pause;
        }
    }
    for (s, rect) in snap.top_left.speeds {
        if draw_ui_button(rect, s.label(), FONT_SM, *speed == s) {
            *speed = s;
            ui.last_run_speed = s;
        }
    }

    // Top-right: FPS readout + Menu button
    draw_panel(snap.top_right.fps_box, COL_BORDER);
    draw_ui_text_right(
        &snap.fps_text,
        snap.top_right.fps_box.right() - PAD,
        button_text_baseline_y(snap.top_right.fps_box, &snap.fps_text, FONT_SM),
        FONT_SM,
        COL_TEXT,
    );
    if draw_ui_button(snap.top_right.menu_btn, "Menu", FONT_SM, ui.menu_open) {
        ui.menu_open = !ui.menu_open;
    }

    // F3 full stats panel (right-aligned values with PAD padding)
    if let Some(st) = &snap.stats {
        draw_panel(st.panel, COL_BORDER);
        draw_ui_text(
            "STATS [F3]",
            st.panel.x + PAD,
            st.header_y,
            FONT_MD,
            COL_ACCENT,
        );
        for (idx, (label, value)) in snap.stats_rows.iter().enumerate() {
            let y = st.first_row_y + idx as f32 * st.row_step;
            draw_ui_text(label, st.panel.x + PAD, y, FONT_SM, COL_TEXT);
            draw_ui_text_right(value, st.panel.right() - PAD, y, FONT_SM, COL_ACCENT);
        }
    }

    // Menu panel (Preset, Weather, Time of day, Mode, Wand Builder, Snap Zoom, Reset)
    if let Some(m) = &snap.menu {
        draw_panel(m.panel, COL_ACCENT);
        draw_ui_text(
            "Preset",
            m.panel.x + PAD,
            m.preset_label_y,
            FONT_SM,
            COL_ACCENT,
        );
        for (preset, rect) in m.presets {
            if draw_ui_button(rect, preset.name(), FONT_SM, world.preset == preset) {
                actions.push(UiAction::SelectPreset(preset));
                ui.menu_open = false;
            }
        }
        if draw_ui_button(m.weather_btn, &snap.menu_labels.weather, FONT_SM, false) {
            actions.push(UiAction::CycleWeather);
        }
        if draw_ui_button(
            m.time_btn,
            &snap.menu_labels.time,
            FONT_SM,
            world.is_night(),
        ) {
            actions.push(UiAction::AdvanceTime);
        }
        if draw_ui_button(
            m.mode_btn,
            snap.menu_labels.mode,
            FONT_SM,
            agents.mage_active,
        ) {
            actions.push(UiAction::ToggleMode);
            ui.menu_open = false;
        }
        if draw_ui_button(m.wand_btn, snap.menu_labels.wand, FONT_SM, ui.wand_open) {
            actions.push(UiAction::ToggleWandBuilder);
            ui.menu_open = false;
        }
        if draw_ui_button(m.snap_btn, snap.menu_labels.snap, FONT_SM, ui.snap_zoom) {
            actions.push(UiAction::ToggleSnapZoom);
        }
        if draw_ui_button(m.reset_btn, snap.menu_labels.reset, FONT_SM, false) {
            actions.push(UiAction::Reset);
            ui.menu_open = false;
        }

        if clicked && !m.panel.contains(mouse) && !snap.top_right.menu_btn.contains(mouse) {
            ui.menu_open = false;
        }
    }

    // Wand Builder drawer (docked left)
    if let Some(w) = &snap.wand {
        if draw_wand_drawer(w, &mut agents.mage.wand, &mut ui.selected_rune_slot) {
            ui.wand_open = false;
        }
    }

    // Bottom HUD: God mode category tabs + color-filled buttons OR Mage mode mana + wand slots
    if let Some(bg) = &snap.bottom_god {
        draw_panel(bg.panel, COL_BORDER);

        for &(cat, rect) in &bg.tabs {
            if draw_ui_button(rect, cat.label(), FONT_SM, ui.active_tab == cat) {
                ui.active_tab = cat;
            }
        }

        // Brush readout box + clickable [-] [+] buttons: "Brush 3 [-] [+]"
        draw_panel(bg.brush_rect, COL_BORDER);
        draw_ui_text_right(
            &snap.brush_text,
            bg.brush_rect.right() - PAD,
            button_text_baseline_y(bg.brush_rect, &snap.brush_text, FONT_SM),
            FONT_SM,
            COL_TEXT,
        );
        if draw_ui_button(bg.brush_minus_btn, "-", FONT_SM, false) {
            ui.brush_radius = (ui.brush_radius - 1).clamp(1, 8);
        }
        if draw_ui_button(bg.brush_plus_btn, "+", FONT_SM, false) {
            ui.brush_radius = (ui.brush_radius + 1).clamp(1, 8);
        }

        if draw_ui_button(bg.help_btn, "?", FONT_SM, ui.controls_open) {
            ui.controls_open = !ui.controls_open;
        }

        for &(entry, rect) in &bg.entries {
            let selected = match entry {
                PaletteEntry::Tool(t) => ui.selected_tool == t,
                PaletteEntry::Spell(p) => agents.mage.wand.payload == p,
            };
            if draw_color_button(rect, entry.label(), entry.fill_color(), selected) {
                match entry {
                    PaletteEntry::Tool(t) => {
                        ui.selected_tool = t;
                    }
                    PaletteEntry::Spell(p) => {
                        actions.push(UiAction::SelectSpell(p));
                    }
                }
            }
        }
    } else if let Some(bm) = &snap.bottom_mage {
        draw_panel(bm.panel, COL_ACCENT);

        let wand = &mut agents.mage.wand;
        if draw_color_button(
            bm.payload_btn,
            wand.payload.short_name(),
            wand.payload.color(),
            true,
        ) {
            actions.push(UiAction::ToggleWandBuilder);
        }

        // Mana bar: surface track + accent fill bar + crisp text above bar
        draw_rectangle(
            bm.mana_bar.x.round(),
            bm.mana_bar.y.round(),
            bm.mana_bar.w.round(),
            bm.mana_bar.h.round(),
            COL_SURFACE,
        );
        let mana_ratio = (wand.mana / wand.max_mana.max(1.0)).clamp(0.0, 1.0);
        let fill_h = 6.0;
        let fill_w = ((bm.mana_bar.w - 2.0) * mana_ratio).round();
        draw_rectangle(
            bm.mana_bar.x.round() + 1.0,
            bm.mana_bar.bottom().round() - fill_h - 1.0,
            fill_w,
            fill_h,
            COL_ACCENT,
        );
        draw_rectangle_lines(
            bm.mana_bar.x.round(),
            bm.mana_bar.y.round(),
            bm.mana_bar.w.round(),
            bm.mana_bar.h.round(),
            1.0,
            COL_BORDER,
        );
        let mana_txt = format!("Mana {:.0}/{:.0}", wand.mana, wand.max_mana);
        let m_w = measure_ui_width(&mana_txt, FONT_SM);
        draw_ui_text(
            &mana_txt,
            bm.mana_bar.x + ((bm.mana_bar.w - m_w) * 0.5).max(4.0),
            bm.mana_bar.y + 14.0,
            FONT_SM,
            COL_TEXT,
        );

        if draw_ui_button(bm.wand_btn, "Wand [B]", FONT_SM, ui.wand_open) {
            actions.push(UiAction::ToggleWandBuilder);
        }

        // 3 Wand rune slots at bottom center
        for (slot_idx, slot_rect) in bm.slots.iter().copied().enumerate() {
            let label = match wand.runes[slot_idx] {
                Some(r) => format!("{}: {}", slot_idx + 1, r.short_name()),
                None => format!("{}: ---", slot_idx + 1),
            };
            let active = ui.wand_open && ui.selected_rune_slot == slot_idx;
            if draw_ui_button(slot_rect, &label, FONT_SM, active) {
                ui.selected_rune_slot = slot_idx;
                ui.wand_open = true;
            }
            if slot_rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Right) {
                wand.slot_rune(slot_idx, None);
                ui.selected_rune_slot = slot_idx;
            }
        }

        let (dash_txt, dash_col) = if agents.mage.dash_cooldown <= 0.0 {
            ("Dash OK", COL_ACCENT)
        } else {
            ("Dash CD", COL_TEXT)
        };
        draw_ui_text_right(dash_txt, bm.dash_right_x, bm.dash_y, FONT_SM, dash_col);

        if draw_ui_button(bm.help_btn, "?", FONT_SM, ui.controls_open) {
            ui.controls_open = !ui.controls_open;
        }
    }

    // Controls overlay modal (toggled by ? button)
    if let Some(ctrl) = &snap.controls {
        draw_panel(ctrl.panel, COL_ACCENT);
        draw_ui_text(
            "CONTROLS",
            ctrl.panel.x + PAD,
            ctrl.header_y,
            FONT_MD,
            COL_ACCENT,
        );
        if draw_ui_button(ctrl.close_btn, "X", FONT_SM, false) {
            ui.controls_open = false;
        }
        for (idx, (key, desc)) in CONTROLS_ROWS.iter().enumerate() {
            let y = ctrl.first_row_y + idx as f32 * ctrl.row_step;
            draw_ui_text(key, ctrl.key_col_x, y, FONT_SM, COL_ACCENT);
            draw_ui_text(desc, ctrl.desc_col_x, y, FONT_SM, COL_TEXT);
        }
        let clicked_help = snap
            .bottom_god
            .as_ref()
            .is_some_and(|b| b.help_btn.contains(mouse))
            || snap
                .bottom_mage
                .as_ref()
                .is_some_and(|b| b.help_btn.contains(mouse));
        if clicked && !ctrl.panel.contains(mouse) && !clicked_help {
            ui.controls_open = false;
        }
    }

    actions
}

fn draw_wand_drawer(layout: &WandDrawerLayout, wand: &mut Wand, selected_slot: &mut usize) -> bool {
    draw_panel(layout.panel, COL_ACCENT);
    draw_ui_text(
        "WAND BUILDER",
        layout.panel.x + PAD,
        layout.header_y,
        FONT_MD,
        COL_ACCENT,
    );
    let close_clicked = draw_ui_button(layout.close_btn, "X", FONT_SM, false);

    let cost_line = format!(
        "Cost: {:.0} mana   CD: {:.2}s",
        wand.mana_cost(),
        wand.cast_cooldown()
    );
    draw_ui_text(
        &cost_line,
        layout.panel.x + PAD,
        layout.cost_y,
        FONT_SM,
        COL_TEXT,
    );

    draw_ui_text(
        "Payload",
        layout.panel.x + PAD,
        layout.payload_label_y,
        FONT_SM,
        COL_ACCENT,
    );
    for &(payload, rect) in &layout.payloads {
        if draw_color_button(
            rect,
            payload.short_name(),
            payload.color(),
            wand.payload == payload,
        ) {
            wand.payload = payload;
        }
    }

    draw_ui_text(
        "Rune Slots (RClick clear)",
        layout.panel.x + PAD,
        layout.slots_label_y,
        FONT_SM,
        COL_ACCENT,
    );
    if draw_ui_button(layout.clear_btn, "Clear", FONT_SM, false) {
        wand.clear_runes();
        *selected_slot = 0;
    }

    let mouse = Vec2::from(mouse_position());
    for (idx, rect) in layout.slots.iter().copied().enumerate() {
        let label = wand.runes[idx].map_or("(Empty)", Rune::short_name);
        if draw_ui_button(rect, label, FONT_SM, *selected_slot == idx) {
            *selected_slot = idx;
        }
        if rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Right) {
            wand.slot_rune(idx, None);
            *selected_slot = idx;
        }
    }

    let mut hovered_rune: Option<Rune> = None;
    for (idx, &rune) in Rune::ALL.iter().enumerate() {
        let rect = layout.runes[idx];
        if rect.contains(mouse) {
            hovered_rune = Some(rune);
        }
        let active = wand.rune_count(rune) > 0;
        if draw_ui_button(rect, rune.short_name(), FONT_SM, active) {
            wand.slot_rune(*selected_slot, Some(rune));
            *selected_slot = (*selected_slot + 1) % 3;
        }
    }

    let rune_hdr = match hovered_rune {
        Some(r) => format!("{} (+{:.0} mana)", r.name(), r.mana_cost()),
        None => "Runes (click to slot)".to_owned(),
    };
    draw_ui_text(
        &rune_hdr,
        layout.panel.x + PAD,
        layout.runes_label_y,
        FONT_SM,
        COL_ACCENT,
    );

    let rune_tags: Vec<&str> = wand
        .runes
        .iter()
        .filter_map(|&r| r.map(Rune::short_name))
        .collect();
    let mut summary = if rune_tags.is_empty() {
        format!("Active: {}", wand.payload.name())
    } else {
        format!(
            "Active: {} [{}]",
            wand.payload.short_name(),
            rune_tags.join("+")
        )
    };
    let max_sum_w = layout.panel.w - PAD * 2.0;
    while summary.len() > 8 && measure_ui_width(&summary, FONT_SM) > max_sum_w {
        summary.pop();
    }
    draw_ui_text(
        &summary,
        layout.panel.x + PAD,
        layout.summary_y,
        FONT_SM,
        COL_TEXT,
    );

    let mut hint = wand.combo_hint().to_owned();
    while hint.len() > 8 && measure_ui_width(&hint, FONT_SM) > max_sum_w {
        hint.pop();
    }
    draw_ui_text(
        &hint,
        layout.panel.x + PAD,
        layout.hint_y,
        FONT_SM,
        COL_ACCENT,
    );

    close_clicked
}

/// Small cursor-following tooltip showing tile name, temperature, and moisture
/// (plus nearest entity tag if directly hovered). Flips left/up near edges.
pub fn draw_cursor_tooltip(
    cursor: Vec2,
    screen: Vec2,
    world: &World,
    agents: &Agents,
    mouse_world: Vec2,
    tile: (i32, i32),
) {
    let Some(cell) = world.get(tile.0, tile.1) else {
        return;
    };

    let line1 = cell.material.name().to_owned();
    let line2 = format!(
        "Temp {:.0}C  Moist {:.0}%",
        cell.temperature,
        cell.moisture * 100.0
    );
    let entity_line = agents.find_nearest_entity(mouse_world, 12.0);

    let mut line_refs: Vec<&str> = vec![line1.as_str(), line2.as_str()];
    if let Some(ent) = &entity_line {
        line_refs.push(ent.as_str());
    }

    let rect = layout_tooltip(cursor, &line_refs, screen.x, screen.y, &measure_ui_width);
    draw_panel(rect, COL_BORDER);
    draw_ui_text(&line1, rect.x + PAD, rect.y + PAD + 13.0, FONT_SM, COL_TEXT);
    draw_ui_text(
        &line2,
        rect.x + PAD,
        rect.y + PAD + 31.0,
        FONT_SM,
        COL_ACCENT,
    );
    if let Some(ent) = &entity_line {
        draw_ui_text(ent, rect.x + PAD, rect.y + PAD + 49.0, FONT_SM, COL_TEXT);
    }
}
