use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use wry::dpi::{PhysicalPosition, PhysicalSize};
use wry::Rect;

use crate::themes::{Rgb, ThemeDefinition};

pub const TOOLBAR_HEIGHT_LOGICAL: f64 = 34.0;
pub const TOOLBAR_HTML: &str = include_str!("toolbar.html");
pub const DESKTOP_BRIDGE_JS: &str = include_str!("desktop_bridge.js");

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WebViewBounds {
    pub toolbar: Rect,
    pub content: Rect,
    pub show_toolbar: bool,
    pub toolbar_px: u32,
}

/// Compute exact physical pixel bounds for the top 34px toolbar and bottom content WebView.
pub fn compute_webview_bounds(
    win_width: u32,
    win_height: u32,
    scale_factor: f64,
    toolbar_visible: bool,
) -> WebViewBounds {
    let scale = if scale_factor.is_finite() && scale_factor > 0.25 {
        scale_factor
    } else {
        1.0
    };
    let toolbar_px = if toolbar_visible {
        ((TOOLBAR_HEIGHT_LOGICAL * scale).round() as u32).min(win_height)
    } else {
        0
    };
    let content_h = win_height.saturating_sub(toolbar_px);
    WebViewBounds {
        toolbar: Rect {
            position: PhysicalPosition::new(0i32, 0i32).into(),
            size: PhysicalSize::new(win_width, toolbar_px).into(),
        },
        content: Rect {
            position: PhysicalPosition::new(0i32, toolbar_px as i32).into(),
            size: PhysicalSize::new(win_width, content_h).into(),
        },
        show_toolbar: toolbar_visible && toolbar_px > 0,
        toolbar_px,
    }
}

fn rgb_hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolbarTheme {
    pub toolbar_bg: String,
    pub border_color: String,
    pub text_primary: String,
    pub text_muted: String,
    pub accent_color: String,
    pub button_hover: String,
    pub button_down: String,
}

impl From<&ThemeDefinition> for ToolbarTheme {
    fn from(t: &ThemeDefinition) -> Self {
        Self {
            toolbar_bg: rgb_hex(t.toolbar_bg),
            border_color: rgb_hex(t.border_color),
            text_primary: rgb_hex(t.text_primary),
            text_muted: rgb_hex(t.text_muted),
            accent_color: rgb_hex(t.accent_color),
            button_hover: rgb_hex(t.button_hover),
            button_down: rgb_hex(t.button_down),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolbarState {
    pub theme: ToolbarTheme,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub playing: bool,
    pub track_label: String,
    pub track_url: String,
    pub lyrics_open: bool,
    pub mini_mode: bool,
    pub always_on_top: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "action")]
pub enum ToolbarAction {
    #[serde(rename = "back")]
    Back,
    #[serde(rename = "forward")]
    Forward,
    #[serde(rename = "reload")]
    Reload,
    #[serde(rename = "navigate")]
    Navigate { url: String },
    #[serde(rename = "copyTrackUrl")]
    CopyTrackUrl,
    #[serde(rename = "prev")]
    Prev,
    #[serde(rename = "playPause")]
    PlayPause,
    #[serde(rename = "next")]
    Next,
    #[serde(rename = "toggleLyrics")]
    ToggleLyrics,
    #[serde(rename = "openLyricsPage")]
    OpenLyricsPage,
    #[serde(rename = "showThemeMenu")]
    ShowThemeMenu { x: f64, y: f64 },
    #[serde(rename = "toggleMini")]
    ToggleMini,
    #[serde(rename = "togglePin")]
    TogglePin,
    #[serde(rename = "showSettingsMenu")]
    ShowSettingsMenu { x: f64, y: f64 },
    #[serde(rename = "shortcut")]
    Shortcut { name: String },
}

pub fn parse_toolbar_action(json: &str) -> Option<ToolbarAction> {
    serde_json::from_str(json).ok()
}

/// Copy UTF-8 text to the Windows clipboard as `CF_UNICODETEXT` (format 13).
pub fn copy_to_clipboard(hwnd: isize, text: &str) -> bool {
    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wide.len() * std::mem::size_of::<u16>();

    unsafe {
        let owner = HWND(hwnd as *mut _);
        if OpenClipboard(Some(owner)).is_err() {
            return false;
        }
        if EmptyClipboard().is_err() {
            let _ = CloseClipboard();
            return false;
        }
        let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, byte_len) else {
            let _ = CloseClipboard();
            return false;
        };
        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            let _ = GlobalFree(Some(HGLOBAL(hmem.0)));
            let _ = CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        let _ = GlobalUnlock(hmem);

        let ok = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(hmem.0))).is_ok();
        if !ok {
            let _ = GlobalFree(Some(HGLOBAL(hmem.0)));
        }
        let _ = CloseClipboard();
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::get_theme;
    use wry::dpi::{Position, Size};

    #[test]
    fn bounds_math_scales_cleanly_and_sums_to_window_height() {
        let b1 = compute_webview_bounds(1280, 820, 1.0, true);
        assert!(b1.show_toolbar);
        assert_eq!(b1.toolbar_px, 34);
        assert_eq!(
            b1.toolbar.size,
            Size::Physical(PhysicalSize::new(1280, 34))
        );
        assert_eq!(
            b1.content.position,
            Position::Physical(PhysicalPosition::new(0, 34))
        );
        assert_eq!(
            b1.content.size,
            Size::Physical(PhysicalSize::new(1280, 786))
        );

        let b_hidpi = compute_webview_bounds(1920, 1080, 1.5, true);
        assert_eq!(b_hidpi.toolbar_px, 51);
        assert_eq!(
            b_hidpi.content.size,
            Size::Physical(PhysicalSize::new(1920, 1029))
        );

        let b_hidden = compute_webview_bounds(1280, 820, 1.5, false);
        assert!(!b_hidden.show_toolbar);
        assert_eq!(b_hidden.toolbar_px, 0);
        assert_eq!(
            b_hidden.content.position,
            Position::Physical(PhysicalPosition::new(0, 0))
        );
        assert_eq!(
            b_hidden.content.size,
            Size::Physical(PhysicalSize::new(1280, 820))
        );
    }

    #[test]
    fn toolbar_action_and_state_serde_roundtrip() {
        assert_eq!(
            parse_toolbar_action(r#"{"action":"navigate","url":"https://soundcloud.com/you/likes"}"#),
            Some(ToolbarAction::Navigate {
                url: "https://soundcloud.com/you/likes".to_string()
            })
        );
        assert_eq!(
            parse_toolbar_action(r#"{"action":"showThemeMenu","x":912.5,"y":30}"#),
            Some(ToolbarAction::ShowThemeMenu { x: 912.5, y: 30.0 })
        );
        assert_eq!(
            parse_toolbar_action(r#"{"action":"toggleLyrics"}"#),
            Some(ToolbarAction::ToggleLyrics)
        );
        assert!(parse_toolbar_action("not-json").is_none());

        let state = ToolbarState {
            theme: ToolbarTheme::from(get_theme("aurawave")),
            can_go_back: true,
            can_go_forward: false,
            playing: true,
            track_label: "Artist — Track".to_string(),
            track_url: "https://soundcloud.com/a/b".to_string(),
            lyrics_open: true,
            mini_mode: false,
            always_on_top: false,
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("#a855f7"));
        assert!(json.contains("Artist — Track"));
    }
}
