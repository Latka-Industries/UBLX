//! Color helpers (HSL lightening / darkening, theme-aware surface shifts).

use ratatui::style::Color;

use crate::utils::Epsilon;

use super::Appearance;

const RGB_MAX: f32 = 255.0;
const HSL_SEGMENT_ANGLE: f32 = 60.0;

/// Lighten an RGB color in HSL space so it stays saturated (not dull). `pct` in 0..=1 moves lightness toward 1.0 and optionally boosts saturation. Non-RGB colors are returned unchanged.
#[must_use]
pub fn lighten_rgb(color: Color, pct: f32) -> Color {
    let Color::Rgb(red, green, blue) = color else {
        return color;
    };
    let pct_clamped = pct.clamp(0.0, 1.0);
    let (hue, sat, lit) = rgb_u8_to_hsl(red, green, blue);
    // Move lightness toward 1.0; nudge saturation up so it doesn't wash out
    let new_lit = lit + (1.0 - lit) * pct_clamped;
    let new_sat = (sat + (1.0 - sat) * pct_clamped * 0.5).min(1.0);
    let (r2, g2, b2) = hsl_to_rgb_u8(hue, new_sat, new_lit);
    Color::Rgb(r2, g2, b2)
}

/// Darken an RGB color in HSL space (mirror of [`lighten_rgb`]): move lightness toward 0. Non-RGB colors are returned unchanged.
#[must_use]
pub fn darken_rgb(color: Color, pct: f32) -> Color {
    let Color::Rgb(red, green, blue) = color else {
        return color;
    };
    let pct_clamped = pct.clamp(0.0, 1.0);
    let (hue, sat, lit) = rgb_u8_to_hsl(red, green, blue);
    let new_lit = lit * (1.0 - pct_clamped);
    let new_sat = (sat + (1.0 - sat) * pct_clamped * 0.5).min(1.0);
    let (r2, g2, b2) = hsl_to_rgb_u8(hue, new_sat, new_lit);
    Color::Rgb(r2, g2, b2)
}

/// Shift a surface color away from the page background: lighten for dark themes, darken for light themes.
#[must_use]
pub fn adjust_surface_rgb(color: Color, pct: f32, appearance: Appearance) -> Color {
    match appearance {
        Appearance::Dark => lighten_rgb(color, pct),
        Appearance::Light => darken_rgb(color, pct),
    }
}

/// RGB u8 [0,255] → HSL: H in [0, 360), S and L in [0, 1].
#[must_use]
pub fn rgb_u8_to_hsl(red: u8, green: u8, blue: u8) -> (f32, f32, f32) {
    let rf = f32::from(red) / RGB_MAX;
    let gf = f32::from(green) / RGB_MAX;
    let bf = f32::from(blue) / RGB_MAX;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let lit = f32::midpoint(max, min);
    if (max - min).abs() < Epsilon::COLOR {
        return (0.0, 0.0, lit);
    }
    let sat = if lit <= 0.5 {
        (max - min) / (max + min)
    } else {
        (max - min) / (2.0 - max - min)
    };
    let hue = if (max - rf).abs() < Epsilon::COLOR {
        HSL_SEGMENT_ANGLE * ((gf - bf) / (max - min)).rem_euclid(6.0)
    } else if (max - gf).abs() < Epsilon::COLOR {
        HSL_SEGMENT_ANGLE * ((bf - rf) / (max - min) + 2.0)
    } else {
        HSL_SEGMENT_ANGLE * ((rf - gf) / (max - min) + 4.0)
    };
    (hue, sat, lit)
}

/// HSL (H in [0, 360), S and L in [0, 1]) → RGB u8.
fn hsl_to_rgb_u8(hue: f32, sat: f32, light: f32) -> (u8, u8, u8) {
    if sat <= Epsilon::COLOR {
        let gray = (light * RGB_MAX).round() as u8;
        return (gray, gray, gray);
    }
    let chroma = (1.0 - (2.0 * light - 1.0).abs()) * sat;
    let chroma_x = chroma * (1.0 - ((hue / HSL_SEGMENT_ANGLE) % 2.0 - 1.0).abs());
    let light_floor = light - chroma / 2.0;
    let (red_f, green_f, blue_f) = match (hue / 60.0) as u32 % 6 {
        0 => (chroma, chroma_x, 0.0),
        1 => (chroma_x, chroma, 0.0),
        2 => (0.0, chroma, chroma_x),
        3 => (0.0, chroma_x, chroma),
        4 => (chroma_x, 0.0, chroma),
        _ => (chroma, 0.0, chroma_x),
    };
    (
        ((red_f + light_floor) * RGB_MAX)
            .round()
            .clamp(0.0, RGB_MAX) as u8,
        ((green_f + light_floor) * RGB_MAX)
            .round()
            .clamp(0.0, RGB_MAX) as u8,
        ((blue_f + light_floor) * RGB_MAX)
            .round()
            .clamp(0.0, RGB_MAX) as u8,
    )
}

/// Space-separated HSL for shadcn / CSS custom properties: `"H S% L%"` (no `hsl()` wrapper).
#[must_use]
pub fn rgb_to_hsl_token(r: u8, g: u8, b: u8) -> String {
    let (hue, sat, lit) = rgb_u8_to_hsl(r, g, b);
    format!(
        "{:.0} {:.0}% {:.0}%",
        hue,
        (sat * 100.0).round(),
        (lit * 100.0).round()
    )
}

/// [`Color::Rgb`] → shadcn HSL token; other variants → [`None`].
#[must_use]
pub fn color_to_hsl_token(color: Color) -> Option<String> {
    let Color::Rgb(r, g, b) = color else {
        return None;
    };
    Some(rgb_to_hsl_token(r, g, b))
}

/// `#RRGGBB` (uppercase) for an sRGB triple.
#[must_use]
pub fn rgb_to_hex6(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// Alias of [`color_rgb_to_hex6`] (docs / WEB_UI naming).
#[must_use]
pub fn color_to_hex6(color: Color) -> Option<String> {
    color_rgb_to_hex6(color)
}

/// [`Color::Rgb`] → `#RRGGBB`; other variants → [`None`].
#[must_use]
pub fn color_rgb_to_hex6(color: Color) -> Option<String> {
    let Color::Rgb(r, g, b) = color else {
        return None;
    };
    Some(rgb_to_hex6(r, g, b))
}

/// `#RRGGBBAA` for OSC 11 (some emulators). `opacity` 0.0–1.0 → alpha byte `round(opacity * 255)`.
#[must_use]
pub fn rgb_to_osc11_hex8(r: u8, g: u8, b: u8, opacity: f32) -> String {
    let a = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("{}{a:02X}", rgb_to_hex6(r, g, b))
}

/// [`Color::Rgb`] + opacity → `#RRGGBBAA` for OSC 11; other color variants → [`None`].
#[must_use]
pub fn color_to_osc11_hex8(color: Color, opacity: f32) -> Option<String> {
    let Color::Rgb(r, g, b) = color else {
        return None;
    };
    Some(rgb_to_osc11_hex8(r, g, b, opacity))
}

/// `rgba(r,g,b,opacity)` payload for OSC 11 (`WezTerm` and others; `#RRGGBBAA` is often ignored there).
#[must_use]
pub fn rgb_to_osc11_rgba_payload(r: u8, g: u8, b: u8, opacity: f32) -> String {
    let a = opacity.clamp(0.0, 1.0);
    format!("rgba({r},{g},{b},{a})")
}

/// [`Color::Rgb`] → OSC 11 `rgba(...)` payload; other variants → [`None`].
#[must_use]
pub fn color_to_osc11_rgba_payload(color: Color, opacity: f32) -> Option<String> {
    let Color::Rgb(r, g, b) = color else {
        return None;
    };
    Some(rgb_to_osc11_rgba_payload(r, g, b, opacity))
}

/// Squared Euclidean distance in sRGB 0–255. [`None`] if either color is not [`Color::Rgb`].
#[must_use]
pub fn rgb_euclidean_sq(a: Color, b: Color) -> Option<u32> {
    let Color::Rgb(ar, ag, ab) = a else {
        return None;
    };
    let Color::Rgb(br, bg, bb) = b else {
        return None;
    };
    let dr = i32::from(ar) - i32::from(br);
    let dg = i32::from(ag) - i32::from(bg);
    let db = i32::from(ab) - i32::from(bb);
    Some((dr * dr + dg * dg + db * db) as u32)
}
