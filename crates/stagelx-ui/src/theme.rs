use bevy_egui::egui::{Color32, RichText};

// ═══════════════════════════════════════════════════════════════════════════════
// Design Tokens — oklch values pre-converted to sRGB
// ═══════════════════════════════════════════════════════════════════════════════

// ── Surfaces ───────────────────────────────────────────────────────────────────
pub const BG_APP: Color32 = Color32::from_rgb(11, 13, 17); // oklch(0.135 0.004 240)
pub const BG_CHROME: Color32 = Color32::from_rgb(19, 22, 26); // oklch(0.165 0.004 240)
pub const BG_PANEL: Color32 = Color32::from_rgb(18, 20, 22); // oklch(0.190 0.005 240)
pub const BG_RAISED: Color32 = Color32::from_rgb(26, 28, 30); // oklch(0.225 0.005 240)
pub const BG_INPUT: Color32 = Color32::from_rgb(10, 11, 13); // oklch(0.150 0.004 240)
pub const BG_HOVER: Color32 = Color32::from_rgb(30, 33, 35); // oklch(0.245 0.005 240)
pub const BG_INPUT_FOCUS: Color32 = Color32::from_rgb(10, 13, 14); // oklch(0.155 0.005 240)

// ── Borders ────────────────────────────────────────────────────────────────────
pub const BORDER: Color32 = Color32::from_rgb(42, 45, 47); // oklch(0.295 0.005 240)
pub const BORDER_SOFT: Color32 = Color32::from_rgb(30, 33, 35); // oklch(0.245 0.005 240)
pub const BORDER_STRONG: Color32 = Color32::from_rgb(65, 68, 70); // oklch(0.385 0.005 240)

// ── Text ───────────────────────────────────────────────────────────────────────
pub const FG: Color32 = Color32::from_rgb(232, 236, 238); // oklch(0.940 0.005 240)
pub const FG_SECONDARY: Color32 = Color32::from_rgb(156, 159, 161); // oklch(0.700 0.005 240)
pub const FG_MUTED: Color32 = Color32::from_rgb(102, 105, 107); // oklch(0.520 0.005 240)
pub const FG_FAINT: Color32 = Color32::from_rgb(69, 72, 74); // oklch(0.400 0.005 240)

// ── Accents ────────────────────────────────────────────────────────────────────
pub const ACCENT: Color32 = Color32::from_rgb(44, 204, 235); // oklch(0.78 0.13 215)
pub const ACCENT_DIM: Color32 = Color32::from_rgb(28, 143, 165); // oklch(0.60 0.10 215)
pub const ACCENT_BG: Color32 = Color32::from_rgb(0, 53, 64); // oklch(0.30 0.06 215)
pub const ACCENT_BG_HOVER: Color32 = Color32::from_rgb(0, 64, 78); // oklch(0.34 0.07 215)

// ── Semantic ───────────────────────────────────────────────────────────────────
pub const WARNING: Color32 = Color32::from_rgb(230, 181, 93); // oklch(0.80 0.12 80)
pub const WARNING_DIM: Color32 = Color32::from_rgb(166, 127, 56); // oklch(0.62 0.10 80)
pub const WARNING_BG: Color32 = Color32::from_rgb(39, 23, 0); // oklch(0.22 0.05 80)
pub const ERROR: Color32 = Color32::from_rgb(236, 92, 80); // oklch(0.66 0.18 28)
pub const RX: Color32 = Color32::from_rgb(124, 213, 144); // oklch(0.80 0.13 150)
pub const IDLE: Color32 = Color32::from_rgb(97, 100, 102); // oklch(0.50 0.005 240)

// ── Derived ────────────────────────────────────────────────────────────────────
pub const ROW_SELECTED: Color32 = Color32::from_rgb(2, 36, 43); // oklch(0.24 0.04 215)
pub const STRIPE_ODD: Color32 = Color32::from_rgba_premultiplied(12, 15, 16, 128);
pub const ROW_BORDER: Color32 = Color32::from_rgb(16, 18, 20); // oklch(0.18 0.005 240)
pub const PILL_LIVE_BG: Color32 = Color32::from_rgb(16, 31, 19); // oklch(0.22 0.03 150)
pub const PILL_LIVE_BORDER: Color32 = Color32::from_rgb(35, 84, 48); // oklch(0.40 0.08 150)
pub const FADER_GRADIENT_BOTTOM: Color32 = Color32::from_rgb(0, 128, 150); // oklch(0.55 0.10 215)
pub const CAP_BG_TOP: Color32 = Color32::from_rgb(75, 77, 79); // oklch(0.42 0.005 240)
pub const CAP_BG_BOTTOM: Color32 = Color32::from_rgb(44, 46, 48); // oklch(0.30 0.005 240)
pub const GLOW_RX: Color32 = Color32::from_rgba_premultiplied(124, 213, 144, 153);
pub const GLOW_TX: Color32 = Color32::from_rgba_premultiplied(44, 204, 235, 128);

// ═══════════════════════════════════════════════════════════════════════════════
// Typography helpers
// ═══════════════════════════════════════════════════════════════════════════════

pub fn wordmark(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(14.0)
        .color(FG)
        .strong()
}

pub fn wordmark_accent(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(14.0)
        .color(ACCENT)
        .strong()
}

pub fn encoder_readout(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(18.0)
        .color(FG)
}

pub fn fader_readout(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(14.0)
        .color(FG)
}

pub fn big_counter(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(16.0)
        .color(FG)
}

pub fn panel_title(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(11.0)
        .color(FG)
        .strong()
}

pub fn mode_tab(text: impl Into<String>, active: bool) -> RichText {
    let mut t = RichText::new(text.into())
        .size(11.0)
        .color(if active { FG } else { FG_SECONDARY });
    if active {
        t = t.strong();
    }
    t
}

pub fn body_row(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(11.0)
        .color(FG)
}

pub fn body_row_secondary(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(11.0)
        .color(FG_SECONDARY)
}

pub fn address_mono(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(11.0)
        .color(FG)
        .monospace()
}

pub fn field_label(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(10.0)
        .color(FG_MUTED)
        .strong()
}

pub fn eyebrow(text: impl Into<String>) -> RichText {
    RichText::new(text.into().to_uppercase())
        .size(9.0)
        .color(FG_MUTED)
        .monospace()
        .strong()
}

pub fn hint(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(9.0)
        .color(FG_FAINT)
        .monospace()
}

pub fn hint_secondary(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(10.0)
        .color(FG_MUTED)
        .monospace()
}

pub fn version_chip(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(9.0)
        .color(FG_FAINT)
        .monospace()
}

pub fn status_bar_text(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(10.0)
        .color(FG_MUTED)
        .monospace()
}

pub fn error_text(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(11.0)
        .color(ERROR)
        .monospace()
}

pub fn viewport_label(text: impl Into<String>) -> RichText {
    RichText::new(text.into().to_uppercase())
        .size(9.0)
        .color(ACCENT)
        .strong()
}

pub fn viewport_sub(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(9.0)
        .color(FG_MUTED)
        .monospace()
}
