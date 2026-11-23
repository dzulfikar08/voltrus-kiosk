//! The kiosk's look: a near-black surface with the Voltrus green, applied to
//! gpui-kit's theme so its inputs and buttons match the hand-drawn parts.

use gpui_kit::component::Theme;
use gpui_kit::{App, AssetSource, FontFeatures, Hsla, Result, SharedString, px, rgb};
use std::borrow::Cow;
use std::sync::Arc;

pub fn hex(v: u32) -> Hsla {
    rgb(v).into()
}

pub struct Palette;

impl Palette {
    pub fn bg() -> Hsla {
        hex(0x07090B)
    }
    pub fn surface() -> Hsla {
        hex(0x0E1114)
    }
    pub fn raised() -> Hsla {
        hex(0x151A1F)
    }
    pub fn line() -> Hsla {
        hex(0xFFFFFF).alpha(0.07)
    }
    pub fn line_strong() -> Hsla {
        hex(0xFFFFFF).alpha(0.12)
    }
    pub fn text() -> Hsla {
        hex(0xF3F5F6)
    }
    pub fn muted() -> Hsla {
        hex(0x949CA4)
    }
    pub fn dim() -> Hsla {
        hex(0x5E666E)
    }
    /// Voltrus green: check-ins, matches, the primary action.
    pub fn accent() -> Hsla {
        hex(0x00E676)
    }
    pub fn accent_fg() -> Hsla {
        hex(0x03140A)
    }
    /// Check-outs.
    pub fn out() -> Hsla {
        hex(0x5CB8FF)
    }
    pub fn warn() -> Hsla {
        hex(0xFFB547)
    }
    pub fn danger() -> Hsla {
        hex(0xFF5C70)
    }
}

/// Digits of equal width, so the clock does not jitter.
pub fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

pub fn apply_theme(cx: &mut App) {
    // Through `update`, so gpui-kit's render tokens follow the colours.
    Theme::update(cx, edit);
}

fn edit(theme: &mut Theme) {
    theme.radius = px(10.);
    theme.radius_lg = px(16.);
    theme.background = Palette::bg();
    theme.foreground = Palette::text();
    theme.muted_foreground = Palette::muted();
    theme.border = Palette::line_strong();
    theme.input = Palette::line_strong();
    theme.ring = Palette::accent().alpha(0.55);
    theme.caret = Palette::accent();
    theme.selection = Palette::accent().alpha(0.28);
    theme.secondary = Palette::raised();
    theme.secondary_hover = hex(0x1C2228);
    theme.secondary_active = hex(0x222930);
    theme.secondary_foreground = Palette::text();
    theme.button_secondary = Palette::raised();
    theme.button_secondary_hover = hex(0x1C2228);
    theme.button_secondary_active = hex(0x222930);
    theme.button_secondary_foreground = Palette::text();
    theme.primary = Palette::accent();
    theme.primary_hover = hex(0x3DEC93);
    theme.primary_active = hex(0x00C964);
    theme.primary_foreground = Palette::accent_fg();
    theme.button_primary = Palette::accent();
    theme.button_primary_hover = hex(0x3DEC93);
    theme.button_primary_active = hex(0x00C964);
    theme.button_primary_foreground = Palette::accent_fg();
    theme.switch = hex(0x2A3138);
    theme.switch_thumb = hex(0xFFFFFF);
    theme.success = Palette::accent();
    theme.danger = Palette::danger();
    theme.warning = Palette::warn();
    theme.info = Palette::out();
}

const MARK: &[u8] = include_bytes!("../assets/brand/mark.png");

/// The full Lucide icon set plus the Voltrus mark.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == "brand/mark.png" {
            return Ok(Some(Cow::Borrowed(MARK)));
        }
        gpui_kit::assets::AllAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_kit::assets::AllAssets.list(path)
    }
}
