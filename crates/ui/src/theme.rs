//! Glass chrome palette.
//!
//! The window itself is translucent and the platform blurs the desktop behind
//! it (macOS vibrancy), so every chrome surface paints a tint over that blur
//! instead of an opaque fill. Only the PDF page itself stays fully opaque.

use gpui::{Hsla, Rgba, rgb, rgba};
use gpui_component::{Theme, ThemeMode};

/// Frost tint painted over the blurred desktop. Everything else layers on top.
/// Heavy enough to keep page chrome readable over an arbitrary backdrop, thin
/// enough that the blur still reads as glass rather than a dark slab.
pub const WINDOW_FROST: u32 = 0x0b0d_10bf;
/// Chrome strips (title bar, tool bar, status bar): a thin lift off the frost.
pub const CHROME_TINT: u32 = 0xffff_ff0a;
/// Side panels (thumbnails, properties). Heavier than the strips so their
/// content stays legible over whatever sits behind the window.
pub const PANEL_TINT: u32 = 0x0f11_1566;
/// Floating cards (page/zoom pill, search field).
pub const FLOAT_TINT: u32 = 0x1416_1ad9;
/// Inset wells inside panels (list rows, read-only text boxes).
pub const WELL_TINT: u32 = 0xffff_ff0d;

/// Hairline between chrome regions.
pub const BORDER: u32 = 0xffff_ff1a;
/// Border for controls that need to read as an edge, not a divider.
pub const BORDER_STRONG: u32 = 0xffff_ff26;
/// Hover wash for chrome sitting on glass.
pub const HOVER: u32 = 0xffff_ff14;
/// Selected or active wash: same family as the hover wash, one step up.
pub const ACTIVE: u32 = 0xffff_ff1f;

pub const TEXT: u32 = 0x00f2_f4f7;
/// Ink for text drawn on the white PDF page (form values, added text,
/// comments). The chrome palette is built for dark glass, so page content
/// needs its own near-black.
pub const PAGE_TEXT: u32 = 0x001f_2937;
pub const TEXT_MUTED: u32 = 0xffff_ffa3;
pub const TEXT_FAINT: u32 = 0xffff_ff70;
pub const ACCENT: u32 = 0x004e_9cff;
pub const ACCENT_SOFT: u32 = 0x4e9c_ff33;
pub const DANGER: u32 = 0x00ff_6b6b;
pub const DANGER_TINT: u32 = 0xff6b_6b1f;
pub const DANGER_TEXT: u32 = 0x00ff_a4a4;

/// Opaque color from an 0xRRGGBB literal.
pub fn solid(color: u32) -> Hsla {
    rgb(color).into()
}

/// Translucent color from an 0xRRGGBBAA literal.
pub fn tint(color: u32) -> Hsla {
    let color: Rgba = rgba(color);
    color.into()
}

/// Make the shared component theme glass-aware.
///
/// `gpui_component`'s root view fills the window with `theme.background` and
/// its controls paint `secondary`/`popover` fills. Left at their defaults
/// those are opaque and would hide the desktop blur entirely, so the surface
/// tones are replaced with the same tints the app's own chrome uses.
pub fn apply_glass(cx: &mut gpui::App) {
    Theme::change(ThemeMode::Dark, None, cx);
    let theme = Theme::global_mut(cx);
    theme.background = tint(WINDOW_FROST);
    theme.title_bar = tint(CHROME_TINT);
    theme.title_bar_border = tint(BORDER);
    theme.border = tint(BORDER);
    theme.input = tint(BORDER_STRONG);
    theme.popover = tint(FLOAT_TINT);
    theme.muted = tint(WELL_TINT);
    theme.muted_foreground = tint(TEXT_MUTED);
    theme.secondary = tint(WELL_TINT);
    theme.secondary_hover = tint(HOVER);
    theme.secondary_active = tint(ACTIVE);
    theme.foreground = solid(TEXT);
    theme.secondary_foreground = solid(TEXT);
    theme.accent = tint(HOVER);
    theme.accent_foreground = solid(TEXT);
    theme.shadow = false;
}
