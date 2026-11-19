use macroquad::prelude::*;

use crate::agents::{Agents, Payload};
use crate::world::{Material, Preset, World};

use super::hud::compute_hud_layout;
use super::layout::{layout_bottom_god, layout_tooltip, CONTROLS_ROWS};
use super::theme::{
    button_text_baseline_y, clamp_rect_to_window, contrast_text_color, estimate_text_width,
    place_tooltip, COL_BLACK, COL_WHITE, FONT_MD, FONT_SM, PAD,
};
use super::{BrushTool, PaletteCategory, PaletteEntry, UiState};

fn assert_rect_inside_window(name: &str, r: Rect, sw: f32, sh: f32) {
    assert!(
        r.x >= PAD - 0.5
            && r.y >= PAD - 0.5
            && r.right() <= sw - PAD + 0.5
            && r.bottom() <= sh - PAD + 0.5,
        "{name} clipped outside [{PAD}, {}] x [{PAD}, {}]: got ({}, {}, w={}, h={}, r={}, b={})",
        sw - PAD,
        sh - PAD,
        r.x,
        r.y,
        r.w,
        r.h,
        r.right(),
        r.bottom()
    );
    assert!(
        r.w > 0.0 && r.h > 0.0,
        "{name} has non-positive size: {r:?}"
    );
}

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.x < b.right() - 0.5
        && a.right() > b.x + 0.5
        && a.y < b.bottom() - 0.5
        && a.bottom() > b.y + 0.5
}

#[test]
fn clamp_rect_keeps_panels_inside_window_bounds() {
    let sw = 960.0;
    let sh = 600.0;

    let off_left_top = clamp_rect_to_window(Rect::new(-40.0, -25.0, 200.0, 120.0), sw, sh, PAD);
    assert_eq!(off_left_top.x, PAD);
    assert_eq!(off_left_top.y, PAD);
    assert_eq!(off_left_top.w, 200.0);
    assert_eq!(off_left_top.h, 120.0);

    let off_right_bot = clamp_rect_to_window(Rect::new(900.0, 560.0, 220.0, 180.0), sw, sh, PAD);
    assert_eq!(off_right_bot.right(), sw - PAD);
    assert_eq!(off_right_bot.bottom(), sh - PAD);

    let oversized = clamp_rect_to_window(Rect::new(50.0, 50.0, 2000.0, 1500.0), sw, sh, PAD);
    assert_eq!(oversized.x, PAD);
    assert_eq!(oversized.y, PAD);
    assert_eq!(oversized.w, sw - 2.0 * PAD);
    assert_eq!(oversized.h, sh - 2.0 * PAD);
}

#[test]
fn tooltip_flips_left_near_right_edge_and_up_near_bottom() {
    let sw = 960.0;
    let sh = 600.0;
    let size = vec2(180.0, 52.0);

    // Center: stays bottom-right of cursor
    let center_cur = vec2(400.0, 300.0);
    let r_center = place_tooltip(center_cur, size, sw, sh, PAD);
    assert!(r_center.x > center_cur.x);
    assert!(r_center.y > center_cur.y);
    assert_rect_inside_window("tooltip_center", r_center, sw, sh);

    // Near right edge: flips cleanly to the left of cursor
    let right_cur = vec2(sw - 30.0, 240.0);
    let r_right = place_tooltip(right_cur, size, sw, sh, PAD);
    assert!(
        r_right.right() < right_cur.x,
        "tooltip near right edge should flip left of cursor: rect={r_right:?}, cursor={right_cur:?}"
    );
    assert_rect_inside_window("tooltip_right", r_right, sw, sh);

    // Near bottom-right corner: flips both left and up
    let br_cur = vec2(sw - 20.0, sh - 20.0);
    let r_br = place_tooltip(br_cur, size, sw, sh, PAD);
    assert!(r_br.right() < br_cur.x);
    assert!(r_br.bottom() < br_cur.y);
    assert_rect_inside_window("tooltip_br", r_br, sw, sh);
}

#[test]
fn contrast_text_picks_black_on_light_and_white_on_dark() {
    assert_eq!(contrast_text_color(Material::DeepWater.color()), COL_WHITE);
    assert_eq!(contrast_text_color(Material::Oil.color()), COL_WHITE);
    assert_eq!(contrast_text_color(Material::Gunpowder.color()), COL_WHITE);
    assert_eq!(contrast_text_color(Material::Dirt.color()), COL_WHITE);
    assert_eq!(contrast_text_color(Material::Tree.color()), COL_WHITE);
    assert_eq!(contrast_text_color(Material::Stone.color()), COL_BLACK);
    assert_eq!(contrast_text_color(Material::Sand.color()), COL_BLACK);
    assert_eq!(contrast_text_color(Material::Ice.color()), COL_BLACK);
    assert_eq!(contrast_text_color(Material::Steam.color()), COL_BLACK);
    assert_eq!(contrast_text_color(Material::Acid.color()), COL_BLACK);
}

#[test]
fn all_materials_and_spells_covered_by_bottom_categories() {
    for &mat in &Material::PALETTE {
        let found = PaletteCategory::ALL.iter().any(|cat| {
            cat.entries()
                .contains(&PaletteEntry::Tool(BrushTool::Element(mat)))
        });
        assert!(
            found,
            "Material {:?} missing from bottom category tabs",
            mat
        );
    }
    for &payload in &Payload::ALL {
        let found = PaletteCategory::Spells
            .entries()
            .contains(&PaletteEntry::Spell(payload));
        assert!(found, "Payload {:?} missing from Spells tab", payload);
    }
}

#[test]
fn all_hud_panels_fit_without_clipping_at_960x600_and_1280x800() {
    let world = World::new(Preset::LushRiverIsland, 42);
    let mut agents = Agents::new(42);
    agents.populate_for_world(&world);

    for (sw, sh) in [(960.0_f32, 600.0_f32), (1280.0_f32, 800.0_f32)] {
        // Test God mode across every bottom category tab with all panels open
        agents.mage_active = false;
        for &tab in &PaletteCategory::ALL {
            let ui = UiState {
                menu_open: true,
                stats_open: true,
                controls_open: true,
                wand_open: true,
                active_tab: tab,
                ..UiState::default()
            };
            let snap = compute_hud_layout(sw, sh, 60, &world, &agents, &ui, &estimate_text_width);

            assert_rect_inside_window("top_left", snap.top_left.bounds, sw, sh);
            assert_rect_inside_window("top_right", snap.top_right.bounds, sw, sh);
            assert!(
                !rects_overlap(snap.top_left.bounds, snap.top_right.bounds),
                "top_left overlaps top_right at {sw}x{sh}"
            );

            let menu = snap.menu.as_ref().expect("menu open");
            let stats = snap.stats.as_ref().expect("stats open");
            let wand = snap.wand.as_ref().expect("wand open");
            let bg = snap.bottom_god.as_ref().expect("bottom god");
            let ctrl = snap.controls.as_ref().expect("controls open");

            assert_rect_inside_window("menu", menu.panel, sw, sh);
            assert_rect_inside_window("stats", stats.panel, sw, sh);
            assert_rect_inside_window("wand", wand.panel, sw, sh);
            assert_rect_inside_window("bottom_god", bg.panel, sw, sh);
            assert_rect_inside_window("controls", ctrl.panel, sw, sh);

            for &(_, r) in &bg.tabs {
                assert_rect_inside_window("tab_btn", r, sw, sh);
                assert!(
                    !rects_overlap(r, bg.brush_rect),
                    "tab overlaps brush readout at {sw}x{sh}"
                );
                assert!(
                    !rects_overlap(r, bg.brush_minus_btn),
                    "tab overlaps brush [-] btn at {sw}x{sh}"
                );
                assert!(
                    !rects_overlap(r, bg.brush_plus_btn),
                    "tab overlaps brush [+] btn at {sw}x{sh}"
                );
                assert!(
                    !rects_overlap(r, bg.help_btn),
                    "tab overlaps help btn at {sw}x{sh}"
                );
            }
            for &(_, r) in &bg.entries {
                assert_rect_inside_window("entry_btn", r, sw, sh);
            }
            assert_rect_inside_window("brush_rect", bg.brush_rect, sw, sh);
            assert_rect_inside_window("brush_minus_btn", bg.brush_minus_btn, sw, sh);
            assert_rect_inside_window("brush_plus_btn", bg.brush_plus_btn, sw, sh);
            assert_rect_inside_window("help_btn", bg.help_btn, sw, sh);
            assert!(!rects_overlap(bg.brush_rect, bg.brush_minus_btn));
            assert!(!rects_overlap(bg.brush_minus_btn, bg.brush_plus_btn));
            assert!(!rects_overlap(bg.brush_plus_btn, bg.help_btn));

            assert!(
                !rects_overlap(menu.panel, stats.panel),
                "menu overlaps stats at {sw}x{sh}"
            );
            assert!(
                !rects_overlap(wand.panel, menu.panel),
                "wand overlaps menu at {sw}x{sh}"
            );
            assert!(
                !rects_overlap(wand.panel, stats.panel),
                "wand overlaps stats at {sw}x{sh}"
            );
            assert!(
                !rects_overlap(wand.panel, bg.panel),
                "wand overlaps bottom bar at {sw}x{sh} (tab {tab:?})"
            );
            assert!(
                !rects_overlap(menu.panel, bg.panel),
                "menu overlaps bottom bar at {sw}x{sh}"
            );
        }

        // Test Mage mode with all panels open
        agents.mage_active = true;
        let ui = UiState {
            menu_open: true,
            stats_open: true,
            controls_open: true,
            wand_open: true,
            ..UiState::default()
        };
        let snap = compute_hud_layout(sw, sh, 60, &world, &agents, &ui, &estimate_text_width);
        let bm = snap.bottom_mage.as_ref().expect("bottom mage");
        let wand = snap.wand.as_ref().expect("wand open");
        let menu = snap.menu.as_ref().expect("menu open");
        assert_rect_inside_window("bottom_mage", bm.panel, sw, sh);
        assert_rect_inside_window("mage_help_btn", bm.help_btn, sw, sh);
        assert!(
            !rects_overlap(bm.panel, bm.help_btn),
            "mage panel overlaps ? button at {sw}x{sh}"
        );
        assert!(
            !rects_overlap(wand.panel, bm.panel),
            "wand overlaps mage bottom bar at {sw}x{sh}"
        );
        assert!(
            !rects_overlap(menu.panel, bm.panel),
            "menu overlaps mage bottom bar at {sw}x{sh}"
        );

        // Tooltip near all edges
        let lines = ["Shallow Water", "Temp 21C  Moist 100%", "Villager: Farm"];
        for cur in [
            vec2(12.0, 12.0),
            vec2(sw - 12.0, 200.0),
            vec2(sw - 12.0, sh - 12.0),
            vec2(12.0, sh - 12.0),
        ] {
            let r = layout_tooltip(cur, &lines, sw, sh, &estimate_text_width);
            assert_rect_inside_window("tooltip_edge", r, sw, sh);
        }
    }
}

#[test]
fn liquids_and_terrain_tabs_share_exact_same_baseline_y() {
    let bg = layout_bottom_god(
        1280.0,
        800.0,
        PaletteCategory::Terrain,
        "Brush 3",
        &estimate_text_width,
    );
    let terrain_rect = bg
        .tabs
        .iter()
        .find(|&&(cat, _)| cat == PaletteCategory::Terrain)
        .expect("Terrain tab")
        .1;
    let liquids_rect = bg
        .tabs
        .iter()
        .find(|&&(cat, _)| cat == PaletteCategory::Liquids)
        .expect("Liquids tab")
        .1;

    let terrain_y = button_text_baseline_y(terrain_rect, "Terrain", FONT_SM);
    let liquids_y = button_text_baseline_y(liquids_rect, "Liquids", FONT_SM);
    assert_eq!(
        terrain_y, liquids_y,
        "\"Liquids\" (with descender q) and \"Terrain\" must share the exact same baseline y"
    );

    // Every tab on row 1 and the brush / [-] / [+] / [?] buttons share the same baseline y
    for &(cat, rect) in &bg.tabs {
        assert_eq!(
            button_text_baseline_y(rect, cat.label(), FONT_SM),
            terrain_y
        );
    }
    assert_eq!(
        button_text_baseline_y(bg.brush_rect, "Brush 3", FONT_SM),
        terrain_y
    );
    assert_eq!(
        button_text_baseline_y(bg.brush_minus_btn, "-", FONT_SM),
        terrain_y
    );
    assert_eq!(
        button_text_baseline_y(bg.brush_plus_btn, "+", FONT_SM),
        terrain_y
    );
    assert_eq!(button_text_baseline_y(bg.help_btn, "?", FONT_SM), terrain_y);

    // Also verify FONT_MD baseline is independent of descenders
    let hdr_rect = Rect::new(16.0, 16.0, 180.0, 28.0);
    assert_eq!(
        button_text_baseline_y(hdr_rect, "Liquids", FONT_MD),
        button_text_baseline_y(hdr_rect, "Terrain", FONT_MD)
    );

    // Verify [ / ] keys are mentioned in the ? controls overlay
    assert!(
        CONTROLS_ROWS
            .iter()
            .any(|&(k, _)| k.contains('[') && k.contains(']')),
        "? controls overlay must mention [ ] keys"
    );
}
