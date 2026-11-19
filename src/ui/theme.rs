use macroquad::prelude::*;

/// 8 px spacing grid used consistently across every HUD panel and widget.
pub const PAD: f32 = 8.0;

/// Only two font sizes are used across the entire UI.
pub const FONT_SM: u16 = 14;
pub const FONT_MD: u16 = 18;

/// Dark pixel palette: base #1e1e2e, surface #313244, text #cdd6f4, one accent #89b4fa.
pub const COL_BASE: Color = Color::from_rgba(30, 30, 46, 255);
pub const COL_SURFACE: Color = Color::from_rgba(49, 50, 68, 255);
pub const COL_SURFACE_HOVER: Color = Color::from_rgba(64, 66, 88, 255);
pub const COL_BORDER: Color = Color::from_rgba(88, 91, 112, 255);
pub const COL_TEXT: Color = Color::from_rgba(205, 214, 244, 255);
pub const COL_ACCENT: Color = Color::from_rgba(137, 180, 250, 255);
pub const COL_WHITE: Color = Color::from_rgba(255, 255, 255, 255);
pub const COL_BLACK: Color = Color::from_rgba(17, 17, 27, 255);

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Picks black or white text maximizing WCAG 2.1 contrast ratio against any fill color.
pub fn contrast_text_color(bg: Color) -> Color {
    let luma = 0.2126 * srgb_to_linear(bg.r)
        + 0.7152 * srgb_to_linear(bg.g)
        + 0.0722 * srgb_to_linear(bg.b);
    if luma > 0.1791 {
        COL_BLACK
    } else {
        COL_WHITE
    }
}

/// Clamps any panel rectangle so it stays completely inside `[pad, sw - pad] x [pad, sh - pad]`
/// with integer-snapped pixel coordinates.
pub fn clamp_rect_to_window(rect: Rect, sw: f32, sh: f32, pad: f32) -> Rect {
    let max_w = (sw - 2.0 * pad).max(1.0);
    let max_h = (sh - 2.0 * pad).max(1.0);
    let w = rect.w.clamp(1.0, max_w).round();
    let h = rect.h.clamp(1.0, max_h).round();
    let max_x = (sw - pad - w).max(pad);
    let max_y = (sh - pad - h).max(pad);
    let x = rect.x.clamp(pad, max_x).round();
    let y = rect.y.clamp(pad, max_y).round();
    Rect::new(x, y, w, h)
}

/// Places a tooltip near `cursor`, flipping to the left near the right edge and above near
/// the bottom edge, then clamping cleanly inside the window.
pub fn place_tooltip(cursor: Vec2, size: Vec2, sw: f32, sh: f32, pad: f32) -> Rect {
    let offset = PAD * 2.0;
    let mut x = cursor.x + offset;
    let mut y = cursor.y + offset;
    if x + size.x > sw - pad {
        x = cursor.x - size.x - offset;
    }
    if y + size.y > sh - pad {
        y = cursor.y - size.y - offset;
    }
    clamp_rect_to_window(Rect::new(x, y, size.x, size.y), sw, sh, pad)
}

pub fn measure_ui_text(text: &str, font_size: u16) -> TextDimensions {
    measure_text(text, None, font_size, 1.0)
}

pub fn measure_ui_width(text: &str, font_size: u16) -> f32 {
    measure_ui_text(text, font_size).width.ceil()
}

/// Fixed cap-height per allowed font size (`FONT_SM = 14`, `FONT_MD = 18`).
pub const fn fixed_font_cap_height(font_size: u16) -> f32 {
    match font_size {
        FONT_SM => 10.0,
        FONT_MD => 13.0,
        other => other as f32 * 0.72,
    }
}

/// Computes a fixed vertical baseline `y` for centering button/tab text of `font_size` inside `rect`.
/// Uses a fixed per-font cap height rather than per-string `measure_text` bounding-box height, so
/// strings with descenders (e.g. "Liquids") and without descenders (e.g. "Terrain") always share
/// the exact same baseline `y`.
pub fn button_text_baseline_y(rect: Rect, _label: &str, font_size: u16) -> f32 {
    let y = rect.y.round();
    let h = rect.h.round();
    (y + (h + fixed_font_cap_height(font_size)) * 0.5 - 1.0).round()
}

/// Deterministic conservative advance-width estimator used by headless layout unit tests
/// where macroquad's GL font context is unavailable.
#[cfg(test)]
pub fn estimate_text_width(text: &str, font_size: u16) -> f32 {
    let char_w = (font_size as f32 * 0.58).ceil();
    text.chars().count() as f32 * char_w
}

/// Draws crisp integer-aligned UI text at the exact requested logical font size
/// (macroquad internally scales font atlas rasterization by `screen_dpi_scale()`).
pub fn draw_ui_text(text: &str, x: f32, y: f32, font_size: u16, color: Color) {
    draw_text_ex(
        text,
        x.round(),
        y.round(),
        TextParams {
            font: None,
            font_size,
            font_scale: 1.0,
            font_scale_aspect: 1.0,
            rotation: 0.0,
            color,
        },
    );
}

/// Draws text right-aligned so its right edge lands at `right_x`.
pub fn draw_ui_text_right(text: &str, right_x: f32, y: f32, font_size: u16, color: Color) {
    let w = measure_ui_width(text, font_size);
    draw_ui_text(text, right_x - w, y, font_size, color);
}

pub fn draw_panel(rect: Rect, border: Color) {
    let x = rect.x.round();
    let y = rect.y.round();
    let w = rect.w.round();
    let h = rect.h.round();
    draw_rectangle(x, y, w, h, COL_BASE);
    draw_rectangle_lines(x, y, w, h, 1.0, border);
}

pub fn draw_ui_button(rect: Rect, label: &str, font_size: u16, active: bool) -> bool {
    let mouse = Vec2::from(mouse_position());
    let hovered = rect.contains(mouse);
    let clicked = hovered && is_mouse_button_pressed(MouseButton::Left);

    let x = rect.x.round();
    let y = rect.y.round();
    let w = rect.w.round();
    let h = rect.h.round();

    let (bg, border, fg) = if active {
        (COL_ACCENT, COL_ACCENT, contrast_text_color(COL_ACCENT))
    } else if hovered {
        (COL_SURFACE_HOVER, COL_ACCENT, COL_TEXT)
    } else {
        (COL_SURFACE, COL_BORDER, COL_TEXT)
    };

    draw_rectangle(x, y, w, h, bg);
    draw_rectangle_lines(x, y, w, h, 1.0, border);

    let text_w = measure_ui_width(label, font_size);
    let tx = x + ((w - text_w) * 0.5).max(2.0);
    let ty = button_text_baseline_y(rect, label, font_size);
    draw_ui_text(label, tx, ty, font_size, fg);
    clicked
}

/// Draws a flat color-filled element/spell button with automatic black/white contrast text
/// and a crisp 2px white border when selected.
pub fn draw_color_button(rect: Rect, label: &str, fill: Color, selected: bool) -> bool {
    let mouse = Vec2::from(mouse_position());
    let hovered = rect.contains(mouse);
    let clicked = hovered && is_mouse_button_pressed(MouseButton::Left);

    let x = rect.x.round();
    let y = rect.y.round();
    let w = rect.w.round();
    let h = rect.h.round();

    draw_rectangle(x, y, w, h, fill);

    let (border_w, border_col) = if selected {
        (2.0, COL_WHITE)
    } else if hovered {
        (1.0, COL_ACCENT)
    } else {
        (1.0, COL_BASE)
    };
    draw_rectangle_lines(x, y, w, h, border_w, border_col);

    let text_w = measure_ui_width(label, FONT_SM);
    let tx = x + ((w - text_w) * 0.5).max(2.0);
    let ty = button_text_baseline_y(rect, label, FONT_SM);
    draw_ui_text(label, tx, ty, FONT_SM, contrast_text_color(fill));
    clicked
}
