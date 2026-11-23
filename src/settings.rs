use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub home_url: String,
    pub theme_id: String,
    pub show_toolbar: bool,
    pub always_on_top: bool,
    pub close_to_tray: bool,
    pub block_ads: bool,
    pub hide_upsell_banners: bool,
    pub prefer_hq_audio: bool,
    pub zoom_factor: f64,
    pub window_x: Option<i32>,
    pub window_y: Option<i32>,
    pub window_width: i32,
    pub window_height: i32,
    pub is_maximized: bool,
    pub lyrics_provider: String,
    pub yandex_music_token: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            home_url: "https://soundcloud.com/discover".to_string(),
            theme_id: "aurawave".to_string(),
            show_toolbar: true,
            always_on_top: false,
            close_to_tray: true,
            block_ads: true,
            hide_upsell_banners: true,
            prefer_hq_audio: true,
            zoom_factor: 1.0,
            window_x: None,
            window_y: None,
            window_width: 1280,
            window_height: 820,
            is_maximized: false,
            lyrics_provider: "lrclib".to_string(),
            yandex_music_token: String::new(),
        }
    }
}

impl AppSettings {
    pub fn app_data_folder() -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        base.join("AuraWave")
    }

    pub fn webview_data_folder() -> PathBuf {
        Self::app_data_folder().join("WebView2")
    }

    pub fn settings_path() -> PathBuf {
        Self::app_data_folder().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::settings_path();
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(mut parsed) = serde_json::from_str::<AppSettings>(&raw) {
                if !is_valid_soundcloud_url(&parsed.home_url) {
                    parsed.home_url = "https://soundcloud.com/discover".to_string();
                }
                parsed.zoom_factor = parsed.zoom_factor.clamp(0.5, 2.0);
                if parsed.window_width < 420 {
                    parsed.window_width = 1280;
                }
                if parsed.window_height < 140 {
                    parsed.window_height = 820;
                }
                if !parsed.lyrics_provider.eq_ignore_ascii_case("yandex") {
                    parsed.lyrics_provider = "lrclib".to_string();
                } else {
                    parsed.lyrics_provider = "yandex".to_string();
                }
                return parsed;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let dir = Self::app_data_folder();
        let _ = fs::create_dir_all(&dir);
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(Self::settings_path(), json);
        }
    }
}

pub fn is_valid_soundcloud_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("https://") && !lower.starts_with("http://") {
        return false;
    }
    let after_scheme = trimmed.split("://").nth(1).unwrap_or("");
    let host_port = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    let host = host_port.split(':').next().unwrap_or("").to_ascii_lowercase();
    host == "soundcloud.com" || host.ends_with(".soundcloud.com") || host == "on.soundcloud.com"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aurawave_isolation_and_defaults() {
        assert!(AppSettings::app_data_folder().ends_with("AuraWave"));
        assert!(AppSettings::webview_data_folder().ends_with("WebView2"));
        assert!(AppSettings::settings_path().ends_with("settings.json"));
        let def = AppSettings::default();
        assert_eq!(def.theme_id, "aurawave");
        assert_eq!(def.lyrics_provider, "lrclib");
        assert!(def.prefer_hq_audio);
    }

    #[test]
    fn soundcloud_url_validation() {
        assert!(is_valid_soundcloud_url("https://soundcloud.com/discover"));
        assert!(is_valid_soundcloud_url("https://m.soundcloud.com/artist/track"));
        assert!(is_valid_soundcloud_url("https://on.soundcloud.com/abc123"));
        assert!(!is_valid_soundcloud_url("https://evil.com/soundcloud.com"));
        assert!(!is_valid_soundcloud_url("javascript:alert(1)"));
        assert!(!is_valid_soundcloud_url(""));
    }
}
