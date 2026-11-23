use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::settings::AppSettings;

pub const USER_AGENT: &str = "AuraWave/1.0";

#[derive(Deserialize)]
#[serde(tag = "type")]
enum IpcRequest {
    #[serde(rename = "getSettings")]
    GetSettings,
    #[serde(rename = "saveSettings")]
    SaveSettings {
        lyrics_provider: Option<String>,
        yandex_music_token: Option<String>,
        prefer_hq_audio: Option<bool>,
        #[serde(default)]
        clear_token: bool,
    },
    #[serde(rename = "openExternal")]
    OpenExternal { url: String },
    #[serde(rename = "lyrics")]
    Lyrics {
        id: u64,
        artist: String,
        title: String,
        #[serde(default)]
        dur: f64,
    },
}

#[derive(Deserialize)]
struct UntypedLyricsReq {
    id: u64,
    artist: String,
    title: String,
    #[serde(default)]
    dur: f64,
}

#[derive(Deserialize, Default, Clone)]
pub struct Rec {
    #[serde(default, rename = "syncedLyrics")]
    pub synced: Option<String>,
    #[serde(default, rename = "plainLyrics")]
    pub plain: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
}

#[derive(Serialize)]
struct Line {
    t: f64,
    s: String,
}

#[derive(Serialize)]
struct Resp {
    #[serde(rename = "type")]
    kind: &'static str,
    id: u64,
    source: &'static str,
    synced: Vec<Line>,
    plain: String,
}

#[derive(Serialize)]
struct SettingsResp {
    #[serde(rename = "type")]
    kind: &'static str,
    lyrics_provider: String,
    has_yandex_token: bool,
    yandex_token_from_env: bool,
    prefer_hq_audio: bool,
}

/// Parse a single LRC timestamp tag like `mm:ss`, `mm:ss.xx`, `mm:ss.xxx`, or `mm:ss:xx`.
fn parse_lrc_tag(tag: &str) -> Option<f64> {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<&str> = trimmed.split(':').collect();
    let (m_str, s_val) = match parts.as_slice() {
        [m, s] => {
            if !m.chars().all(|c| c.is_ascii_digit()) || m.is_empty() {
                return None;
            }
            if !s.chars().all(|c| c.is_ascii_digit() || c == '.') {
                return None;
            }
            let s: f64 = s.parse().ok()?;
            (*m, s)
        }
        [m, s, frac] => {
            if !m.chars().all(|c| c.is_ascii_digit())
                || m.is_empty()
                || !s.chars().all(|c| c.is_ascii_digit())
                || s.is_empty()
                || !frac.chars().all(|c| c.is_ascii_digit())
                || frac.is_empty()
            {
                return None;
            }
            let sec: f64 = s.parse().ok()?;
            let frac_num: f64 = frac.parse().ok()?;
            let divisor = 10_f64.powi(frac.len().min(9) as i32);
            (*m, sec + frac_num / divisor)
        }
        _ => return None,
    };

    let m: f64 = m_str.parse().ok()?;
    let total = m * 60.0 + s_val;
    if total.is_finite() && total >= 0.0 {
        Some(total)
    } else {
        None
    }
}

/// Parse LRC text into `(seconds, text)` lines, sorted by timestamp.
pub fn parse_lrc(lrc: &str) -> Vec<(f64, String)> {
    let mut out = Vec::new();
    for raw in lrc.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while rest.starts_with('[') {
            let Some(end) = rest.find(']') else { break };
            let tag = &rest[1..end];
            let Some(t) = parse_lrc_tag(tag) else {
                break;
            };
            times.push(t);
            rest = rest[end + 1..].trim_start();
        }
        for t in times {
            out.push((t, rest.to_string()));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

fn enc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

fn has_synced(r: &Rec) -> bool {
    r.synced.as_deref().map_or(false, |s| !s.trim().is_empty())
}

fn has_any(r: &Rec) -> bool {
    has_synced(r) || r.plain.as_deref().map_or(false, |s| !s.trim().is_empty())
}

fn get_json<T: for<'a> Deserialize<'a>>(url: &str) -> Option<T> {
    ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(10))
        .call()
        .ok()?
        .into_json()
        .ok()
}

pub trait LyricsProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn fetch(&self, artist: &str, title: &str, dur: f64) -> Rec;
}

pub struct LrclibProvider;

impl LrclibProvider {
    fn pick_best(list: Vec<Rec>, dur: f64) -> Rec {
        let mut best: Option<(f64, Rec)> = None;
        for (idx, r) in list.into_iter().enumerate() {
            if !has_any(&r) {
                continue;
            }
            let mut score = if has_synced(&r) { 1000.0 } else { 100.0 };
            if dur > 0.0 {
                if let Some(rd) = r.duration {
                    if rd.is_finite() && rd > 0.0 {
                        score -= (rd - dur).abs().min(300.0);
                    }
                }
            }
            score -= (idx as f64) * 0.5;
            if best.as_ref().map_or(true, |(bs, _)| score > *bs) {
                best = Some((score, r));
            }
        }
        best.map(|(_, r)| r).unwrap_or_default()
    }

    fn query(&self, artist: &str, title: &str, dur: f64) -> Rec {
        if !artist.trim().is_empty() && !title.trim().is_empty() {
            let mut url = format!(
                "https://lrclib.net/api/get?artist_name={}&track_name={}",
                enc(artist),
                enc(title)
            );
            if dur > 0.0 {
                url.push_str(&format!("&duration={}", dur.round() as i64));
            }
            if let Some(r) = get_json::<Rec>(&url) {
                if has_synced(&r) {
                    return r;
                }
                if has_any(&r) {
                    let search_url = format!(
                        "https://lrclib.net/api/search?track_name={}&artist_name={}",
                        enc(title),
                        enc(artist)
                    );
                    if let Some(list) = get_json::<Vec<Rec>>(&search_url) {
                        let searched = Self::pick_best(list, dur);
                        if has_synced(&searched) {
                            return searched;
                        }
                    }
                    return r;
                }
            }

            let url = format!(
                "https://lrclib.net/api/search?track_name={}&artist_name={}",
                enc(title),
                enc(artist)
            );
            if let Some(list) = get_json::<Vec<Rec>>(&url) {
                let picked = Self::pick_best(list, dur);
                if has_any(&picked) {
                    return picked;
                }
            }
        }

        let q = format!("{} {}", artist, title).trim().to_string();
        if q.is_empty() {
            return Rec::default();
        }
        let url = format!("https://lrclib.net/api/search?q={}", enc(&q));
        let list: Vec<Rec> = get_json(&url).unwrap_or_default();
        Self::pick_best(list, dur)
    }
}

impl LyricsProvider for LrclibProvider {
    fn name(&self) -> &'static str {
        "LRCLIB"
    }

    fn fetch(&self, artist: &str, title: &str, dur: f64) -> Rec {
        let primary = self.query(artist, title, dur);
        if has_synced(&primary) {
            return primary;
        }
        if let Some((split_artist, split_title)) = split_artist_title(title) {
            let split = self.query(split_artist, split_title, dur);
            if has_synced(&split) || (!has_any(&primary) && has_any(&split)) {
                return split;
            }
        }
        primary
    }
}

pub struct YandexProvider;

const YANDEX_SIGN_KEY: &[u8] = b"p93jhgh689SBReK6ghtw62";

pub fn yandex_sign(track_id: &str, ts: u64) -> String {
    use base64::Engine;
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(YANDEX_SIGN_KEY)
        .expect("HMAC key length is valid");
    mac.update(format!("{}{}", track_id, ts).as_bytes());
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

fn resolve_yandex_token() -> (Option<String>, bool) {
    if let Ok(env_tok) = std::env::var("YANDEX_MUSIC_TOKEN") {
        let trimmed = env_tok.trim();
        if !trimmed.is_empty() {
            return (Some(trimmed.to_string()), true);
        }
    }
    let cfg_tok = AppSettings::load().yandex_music_token;
    let trimmed = cfg_tok.trim();
    if !trimmed.is_empty() {
        return (Some(trimmed.to_string()), false);
    }
    (None, false)
}

fn yandex_get_json(url: &str, token: &str) -> Option<serde_json::Value> {
    ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .set("Authorization", &format!("OAuth {}", token))
        .set("X-Yandex-Music-Client", "YandexMusicAndroid/24023231")
        .timeout(Duration::from_secs(10))
        .call()
        .ok()?
        .into_json()
        .ok()
}

fn pick_yandex_track_id(results: &[serde_json::Value], dur: f64) -> Option<String> {
    let mut best: Option<(f64, String)> = None;
    for (idx, item) in results.iter().enumerate() {
        let id = item["id"]
            .as_i64()
            .map(|i| i.to_string())
            .or_else(|| item["id"].as_str().map(String::from));
        let Some(id) = id else { continue };

        let has_sync = item["lyricsInfo"]["hasAvailableSyncLyrics"]
            .as_bool()
            .unwrap_or(false);
        let has_text = item["lyricsInfo"]["hasAvailableTextLyrics"]
            .as_bool()
            .unwrap_or(false);

        let mut score = if has_sync {
            1000.0
        } else if has_text {
            200.0
        } else {
            0.0
        };

        if dur > 0.0 {
            if let Some(ms) = item["durationMs"].as_f64() {
                if ms.is_finite() && ms > 0.0 {
                    score -= (ms / 1000.0 - dur).abs().min(300.0);
                }
            }
        }
        score -= (idx as f64) * 0.5;

        if best.as_ref().map_or(true, |(bs, _)| score > *bs) {
            best = Some((score, id));
        }
    }
    best.map(|(_, id)| id)
}

impl LyricsProvider for YandexProvider {
    fn name(&self) -> &'static str {
        "Yandex Music"
    }

    fn fetch(&self, artist: &str, title: &str, dur: f64) -> Rec {
        let (Some(token), _) = resolve_yandex_token() else {
            return Rec::default();
        };
        let q = format!("{} {}", artist, title).trim().to_string();
        if q.is_empty() {
            return Rec::default();
        }

        let search_url = format!(
            "https://api.music.yandex.net/search?text={}&type=track&page=0",
            enc(&q)
        );
        let Some(search_json) = yandex_get_json(&search_url, &token) else {
            return Rec::default();
        };
        let Some(results) = search_json["result"]["tracks"]["results"].as_array() else {
            return Rec::default();
        };
        let Some(id) = pick_yandex_track_id(results, dur) else {
            return Rec::default();
        };

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let sign = yandex_sign(&id, ts);

        let lrc_url = format!(
            "https://api.music.yandex.net/tracks/{}/lyrics?format=LRC&timeStamp={}&sign={}",
            id,
            ts,
            enc(&sign)
        );
        if let Some(v) = yandex_get_json(&lrc_url, &token) {
            if let Some(dl) = v["result"]["downloadUrl"].as_str() {
                if let Ok(r) = ureq::get(dl)
                    .set("User-Agent", USER_AGENT)
                    .timeout(Duration::from_secs(10))
                    .call()
                {
                    let text = r.into_string().unwrap_or_default();
                    if !parse_lrc(&text).is_empty() {
                        return Rec {
                            synced: Some(text),
                            plain: None,
                            duration: None,
                        };
                    }
                    if !text.trim().is_empty() {
                        return Rec {
                            synced: None,
                            plain: Some(text),
                            duration: None,
                        };
                    }
                }
            }
        }

        let text_url = format!(
            "https://api.music.yandex.net/tracks/{}/lyrics?format=TEXT&timeStamp={}&sign={}",
            id,
            ts,
            enc(&sign)
        );
        if let Some(v) = yandex_get_json(&text_url, &token) {
            if let Some(dl) = v["result"]["downloadUrl"].as_str() {
                if let Ok(r) = ureq::get(dl)
                    .set("User-Agent", USER_AGENT)
                    .timeout(Duration::from_secs(10))
                    .call()
                {
                    let text = r.into_string().unwrap_or_default();
                    if !text.trim().is_empty() {
                        return Rec {
                            synced: None,
                            plain: Some(text),
                            duration: None,
                        };
                    }
                }
            }
        }

        let supp_url = format!("https://api.music.yandex.net/tracks/{}/supplement", id);
        if let Some(v) = yandex_get_json(&supp_url, &token) {
            if let Some(full) = v["result"]["lyrics"]["fullLyrics"].as_str() {
                if !full.trim().is_empty() {
                    return Rec {
                        synced: None,
                        plain: Some(full.to_string()),
                        duration: None,
                    };
                }
            }
        }

        Rec::default()
    }
}

fn split_artist_title(title: &str) -> Option<(&str, &str)> {
    let (a, t) = title.split_once(" - ")?;
    let a = a.trim();
    let t = t.trim();
    if a.is_empty() || t.is_empty() {
        None
    } else {
        Some((a, t))
    }
}

fn fetch_with_providers(
    artist: &str,
    title: &str,
    dur: f64,
    preferred: &str,
) -> (Rec, &'static str) {
    let lrclib = LrclibProvider;
    let yandex = YandexProvider;
    let (primary, fallback): (&dyn LyricsProvider, &dyn LyricsProvider) =
        if preferred.eq_ignore_ascii_case("yandex") {
            (&yandex, &lrclib)
        } else {
            (&lrclib, &yandex)
        };

    let p_rec = primary.fetch(artist, title, dur);
    if has_synced(&p_rec) {
        return (p_rec, primary.name());
    }

    let f_rec = fallback.fetch(artist, title, dur);
    if has_synced(&f_rec) {
        return (f_rec, fallback.name());
    }

    if has_any(&p_rec) {
        return (p_rec, primary.name());
    }
    if has_any(&f_rec) {
        return (f_rec, fallback.name());
    }

    (Rec::default(), "")
}

fn settings_response() -> Option<String> {
    let s = AppSettings::load();
    let (tok, from_env) = resolve_yandex_token();
    serde_json::to_string(&SettingsResp {
        kind: "settings",
        lyrics_provider: s.lyrics_provider,
        has_yandex_token: tok.is_some(),
        yandex_token_from_env: from_env,
        prefer_hq_audio: s.prefer_hq_audio,
    })
    .ok()
}

/// Opens a strictly allowlisted `https://genius.com/...` URL in the user's default browser.
pub fn open_external_genius(raw_url: &str) -> bool {
    let trimmed = raw_url.trim();
    if !trimmed.starts_with("https://genius.com/") {
        return false;
    }
    let wide_op: Vec<u16> = "open\0".encode_utf16().collect();
    let wide_url: Vec<u16> = trimmed.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let res = windows::Win32::UI::Shell::ShellExecuteW(
            None,
            windows::core::PCWSTR(wide_op.as_ptr()),
            windows::core::PCWSTR(wide_url.as_ptr()),
            None,
            None,
            windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
        (res.0 as isize) > 32
    }
}

/// Handles a JS IPC request and returns a JSON response string.
pub fn handle_request(body: &str) -> Option<String> {
    if let Ok(req) = serde_json::from_str::<IpcRequest>(body) {
        match req {
            IpcRequest::OpenExternal { url } => {
                let _ = open_external_genius(&url);
                return None;
            }
            IpcRequest::GetSettings => return settings_response(),
            IpcRequest::SaveSettings {
                lyrics_provider,
                yandex_music_token,
                prefer_hq_audio,
                clear_token,
            } => {
                let mut s = AppSettings::load();
                if let Some(p) = lyrics_provider {
                    s.lyrics_provider = if p.eq_ignore_ascii_case("yandex") {
                        "yandex".to_string()
                    } else {
                        "lrclib".to_string()
                    };
                }
                if let Some(hq) = prefer_hq_audio {
                    s.prefer_hq_audio = hq;
                }
                if clear_token {
                    s.yandex_music_token.clear();
                } else if let Some(tok) = yandex_music_token {
                    let trimmed = tok.trim();
                    if !trimmed.is_empty() {
                        s.yandex_music_token = trimmed.to_string();
                    }
                }
                s.save();
                return settings_response();
            }
            IpcRequest::Lyrics {
                id,
                artist,
                title,
                dur,
            } => {
                let pref = AppSettings::load().lyrics_provider;
                let (rec, source) = fetch_with_providers(&artist, &title, dur, &pref);
                let synced = rec
                    .synced
                    .as_deref()
                    .map(parse_lrc)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(t, s)| Line { t, s })
                    .collect();
                return serde_json::to_string(&Resp {
                    kind: "lyrics",
                    id,
                    source,
                    synced,
                    plain: rec.plain.unwrap_or_default(),
                })
                .ok();
            }
        }
    }

    let req: UntypedLyricsReq = serde_json::from_str(body).ok()?;
    let pref = AppSettings::load().lyrics_provider;
    let (rec, source) = fetch_with_providers(&req.artist, &req.title, req.dur, &pref);
    let synced = rec
        .synced
        .as_deref()
        .map(parse_lrc)
        .unwrap_or_default()
        .into_iter()
        .map(|(t, s)| Line { t, s })
        .collect();
    serde_json::to_string(&Resp {
        kind: "lyrics",
        id: req.id,
        source,
        synced,
        plain: rec.plain.unwrap_or_default(),
    })
    .ok()
}

pub const OVERLAY_JS: &str = include_str!("overlay.js");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lrc_formats_and_sorting() {
        let lrc = "\
[ar:Test Artist]
[00:12.500]Line two
[00:05.25]Line one
[00:20:50]Colon centis
[00:01]Whole seconds
";
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed.len(), 4);
        assert!((parsed[0].0 - 1.0).abs() < 1e-6);
        assert_eq!(parsed[0].1, "Whole seconds");
        assert!((parsed[1].0 - 5.25).abs() < 1e-6);
        assert_eq!(parsed[1].1, "Line one");
        assert!((parsed[2].0 - 12.5).abs() < 1e-6);
        assert_eq!(parsed[2].1, "Line two");
        assert!((parsed[3].0 - 20.5).abs() < 1e-6);
        assert_eq!(parsed[3].1, "Colon centis");
    }

    #[test]
    fn parse_lrc_multi_tag_empty_and_bracketed_text() {
        let lrc = "\
[00:10.00][00:40.00]Chorus line
[00:15.00][Chorus] Keep brackets
[00:18.00]
";
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed.len(), 4);
        assert_eq!(parsed[0], (10.0, "Chorus line".to_string()));
        assert_eq!(parsed[1], (15.0, "[Chorus] Keep brackets".to_string()));
        assert_eq!(parsed[2], (18.0, "".to_string()));
        assert_eq!(parsed[3], (40.0, "Chorus line".to_string()));
    }

    #[test]
    fn parse_lrc_rejects_nan_inf_negative_without_panic() {
        let lrc = "\
[NaN:00]Bad nan
[inf:10]Bad inf
[-01:00]Bad neg
[00:NaN]Bad sec nan
[00:02.00]Valid
";
        let parsed = parse_lrc(lrc);
        assert_eq!(parsed, vec![(2.0, "Valid".to_string())]);
    }

    #[test]
    fn yandex_hmac_sha256_sign_matches_expected() {
        use base64::Engine;
        use hmac::{Hmac, Mac};
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(b"p93jhgh689SBReK6ghtw62").unwrap();
        mac.update(b"1234561700000000");
        let expected = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
        assert_eq!(yandex_sign("123456", 1700000000), expected);
        assert_eq!(USER_AGENT, "AuraWave/1.0");
    }

    #[test]
    fn overlay_js_contains_dedicated_lyrics_page_and_button() {
        assert!(OVERLAY_JS.contains("#scl-page-btn"));
        assert!(OVERLAY_JS.contains("#scl-page"));
        assert!(OVERLAY_JS.contains("__scLyricsOpenPage"));
    }
}
