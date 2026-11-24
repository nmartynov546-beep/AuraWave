#![windows_subsystem = "windows"]
mod adblock;
mod ipc_instance;
mod lyrics;
mod settings;
mod themes;
mod toolbar;

use muda::{CheckMenuItem, ContextMenu, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::platform::windows::WindowExtWindows;
use tao::window::{Fullscreen as TaoFullscreen, Icon as WindowIcon, Theme as TaoTheme, Window, WindowBuilder};
use tray_icon::{MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};
use webview2_com::HistoryChangedEventHandler;
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWINDOWATTRIBUTE, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
};
use windows::Win32::System::Com::CoTaskMemFree;
use wry::{WebContext, WebView, WebViewBuilder, WebViewExtWindows};

const ICON_RGBA_64: &[u8] = include_bytes!("../assets/aurawave_rgba.bin");
const ICON_SIZE: u32 = 64;

#[derive(Debug, Clone)]
pub enum UserEvent {
    Lyrics(String),
    PipeMessage(String),
    Tray(TrayIconEvent),
    Menu(MenuEvent),
    Toolbar(toolbar::ToolbarAction),
    HistoryChanged,
    PlayerState {
        playing: bool,
        title: String,
        artist: String,
        track_url: String,
        page_url: String,
    },
    LyricsState {
        open: bool,
    },
    Shortcut(String),
    Nav(String),
}

pub struct AppState {
    pub settings: settings::AppSettings,
    pub theme: &'static themes::ThemeDefinition,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub playing: bool,
    pub track_label: String,
    pub track_url: String,
    pub current_page_url: String,
    pub lyrics_open: bool,
    pub mini_mode: bool,
    pub fullscreen: bool,
    pub normal_bounds: Option<(i32, i32, u32, u32)>,
    pub normal_maximized: bool,
    pub fs_saved_bounds: Option<(i32, i32, u32, u32)>,
    pub fs_saved_maximized: bool,
}

impl AppState {
    pub fn new(mut s: settings::AppSettings) -> Self {
        let theme = themes::get_theme(&s.theme_id);
        s.theme_id = theme.id.to_string();
        Self {
            settings: s,
            theme,
            can_go_back: false,
            can_go_forward: false,
            playing: false,
            track_label: "SoundCloud".to_string(),
            track_url: String::new(),
            current_page_url: "https://soundcloud.com/discover".to_string(),
            lyrics_open: false,
            mini_mode: false,
            fullscreen: false,
            normal_bounds: None,
            normal_maximized: false,
            fs_saved_bounds: None,
            fs_saved_maximized: false,
        }
    }

    pub fn to_toolbar_state(&self) -> toolbar::ToolbarState {
        toolbar::ToolbarState {
            theme: toolbar::ToolbarTheme::from(self.theme),
            can_go_back: self.can_go_back,
            can_go_forward: self.can_go_forward,
            playing: self.playing,
            track_label: self.track_label.clone(),
            track_url: self.best_track_url(),
            lyrics_open: self.lyrics_open,
            mini_mode: self.mini_mode,
            always_on_top: self.settings.always_on_top,
        }
    }

    pub fn best_track_url(&self) -> String {
        if !self.track_url.is_empty() {
            self.track_url.clone()
        } else if !self.current_page_url.is_empty() {
            self.current_page_url.clone()
        } else {
            "https://soundcloud.com".to_string()
        }
    }
}

pub struct TrayState {
    pub tray: TrayIcon,
    pub show_id: MenuId,
    pub play_id: MenuId,
    pub next_id: MenuId,
    pub prev_id: MenuId,
    pub lyrics_id: MenuId,
    pub page_id: MenuId,
    pub pin_id: MenuId,
    pub close_to_tray_id: MenuId,
    pub quit_id: MenuId,
    pub theme_ids: Vec<(MenuId, &'static str)>,
    pub pin_item: CheckMenuItem,
    pub close_to_tray_item: CheckMenuItem,
    pub play_item: MenuItem,
}

fn load_window_icon() -> Option<WindowIcon> {
    WindowIcon::from_rgba(ICON_RGBA_64.to_vec(), ICON_SIZE, ICON_SIZE).ok()
}

fn load_tray_icon() -> Option<tray_icon::Icon> {
    tray_icon::Icon::from_resource(1, Some((32, 32)))
        .ok()
        .or_else(|| tray_icon::Icon::from_rgba(ICON_RGBA_64.to_vec(), ICON_SIZE, ICON_SIZE).ok())
}

fn apply_dwm_theme(window: &Window, theme: &themes::ThemeDefinition) {
    let hwnd = HWND(window.hwnd() as *mut _);
    let dark = BOOL::from(theme.dark_title_bar);
    let caption = theme.toolbar_bg.colorref();
    let text = theme.text_primary.colorref();
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(20),
            &dark as *const _ as *const _,
            std::mem::size_of::<BOOL>() as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &caption as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TEXT_COLOR,
            &text as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

fn build_theme_css_script(theme: &themes::ThemeDefinition) -> String {
    let t = theme;
    let theme_css = concat!(
        ".sc-button-cta,.sc-button-primary,.sc-button-orange,.playButton{background-color:var(--sc-accent)!important;border-color:var(--sc-accent)!important;color:#fff!important;}",
        ".sc-button-cta:hover,.sc-button-primary:hover,.playButton:hover{filter:brightness(1.08);}",
        ".sc-button-selected.sc-button-like,.sc-button-selected.sc-button-repost,.sc-button-selected.sc-button-follow,.sc-button-selected{color:var(--sc-accent)!important;border-color:var(--sc-accent)!important;}",
        ".sc-button-selected svg,.sc-button-selected svg path{fill:var(--sc-accent)!important;}",
        ".playbackTimeline__progressBar,.playbackTimeline__progressHandle,.volume__sliderProgress,.volume__sliderHandle{background-color:var(--sc-accent)!important;}",
        ".playControls__control,.repeatControl,.shuffleControl,.playControls__play{box-sizing:border-box!important;overflow:visible!important;}",
        ".shuffleControl.m-shuffling,.repeatControl.m-one,.repeatControl.m-all,.playControls__control.m-active{color:var(--sc-accent)!important;}",
        ".shuffleControl.m-shuffling svg path,.repeatControl.m-one svg path,.repeatControl.m-all svg path,.playControls__control.m-active svg path{fill:var(--sc-accent)!important;}",
        ".playControls__bg,.playControls__wrapper{background-color:var(--sc-toolbar)!important;border-top:1px solid var(--sc-border)!important;}",
        ".playControls .sc-text,.playbackSoundBadge__titleLink,.playbackTimeline__duration{color:var(--sc-text)!important;}",
        ".playbackSoundBadge__lightLink,.playbackTimeline__timePassed{color:var(--sc-muted)!important;}",
        ".playbackSoundBadge__actions{display:flex!important;align-items:center!important;gap:4px!important;}",
        ".playbackSoundBadge__actions .sc-button,.playbackSoundBadge__actions button{width:26px!important;height:26px!important;min-width:26px!important;min-height:26px!important;max-width:26px!important;max-height:26px!important;box-sizing:border-box!important;padding:0!important;border-radius:4px!important;}",
        ".playbackSoundBadge__showQueue,.playControls .playbackSoundBadge__showQueue,.playbackSoundBadge__actions .playbackSoundBadge__showQueue,.playbackSoundBadge__queueCircle,.playControls__queue,.queueControl,button.playbackSoundBadge__showQueue,button[title*='Next up' i],button[aria-label*='Next up' i],button[title*='Queue' i],button[aria-label*='Queue' i],button[title*='Очередь' i],button[aria-label*='Очередь' i]{width:26px!important;height:26px!important;min-width:26px!important;min-height:26px!important;max-width:26px!important;max-height:26px!important;box-sizing:border-box!important;padding:0!important;margin:0 2px!important;border:1px solid var(--sc-border)!important;border-radius:4px!important;background-color:transparent!important;color:var(--sc-text)!important;display:inline-flex!important;align-items:center!important;justify-content:center!important;vertical-align:middle!important;cursor:pointer!important;line-height:1!important;}",
        ".playbackSoundBadge__showQueue:hover,.playControls .playbackSoundBadge__showQueue:hover,.playbackSoundBadge__actions .playbackSoundBadge__showQueue:hover,.playbackSoundBadge__queueCircle:hover,.playControls__queue:hover,.queueControl:hover,button.playbackSoundBadge__showQueue:hover,button[title*='Next up' i]:hover,button[aria-label*='Next up' i]:hover,button[title*='Queue' i]:hover,button[aria-label*='Queue' i]:hover,button[title*='Очередь' i]:hover,button[aria-label*='Очередь' i]:hover{background-color:var(--sc-hover)!important;border-color:var(--sc-accent)!important;}",
        ".playbackSoundBadge.m-queueVisible .playbackSoundBadge__showQueue,.playbackSoundBadge__showQueue.m-active,.playbackSoundBadge__showQueue.sc-button-selected,.playControls__queue.m-active,.queueControl.m-active{background-color:var(--sc-accent)!important;border-color:var(--sc-accent)!important;color:#fff!important;}",
        ".playbackSoundBadge__showQueue svg,.playControls .playbackSoundBadge__showQueue svg,.playbackSoundBadge__actions .playbackSoundBadge__showQueue svg,.playbackSoundBadge__queueCircle svg,.playControls__queue svg,.queueControl svg,button.playbackSoundBadge__showQueue svg,button[title*='Next up' i] svg,button[aria-label*='Next up' i] svg,button[title*='Queue' i] svg,button[aria-label*='Queue' i] svg,button[title*='Очередь' i] svg,button[aria-label*='Очередь' i] svg{width:16px!important;height:16px!important;max-width:16px!important;max-height:16px!important;display:block!important;margin:auto!important;fill:currentColor!important;}",
        ".playbackSoundBadge.m-queueVisible .playbackSoundBadge__showQueue svg path,.playbackSoundBadge__showQueue.m-active svg path,.playbackSoundBadge__showQueue.sc-button-selected svg path,.playControls__queue.m-active svg path,.queueControl.m-active svg path{fill:#fff!important;}"
    );
    format!(
        r#"(function(){{
if(window.top!==window)return;
const apply=()=>{{
  const r=document.documentElement;
  if(r){{
    r.style.setProperty('--sc-toolbar','rgb({},{},{})');
    r.style.setProperty('--sc-bg','rgb({},{},{})');
    r.style.setProperty('--sc-border','rgb({},{},{})');
    r.style.setProperty('--sc-text','rgb({},{},{})');
    r.style.setProperty('--sc-muted','rgb({},{},{})');
    r.style.setProperty('--sc-accent','rgb({},{},{})');
    r.style.setProperty('--sc-hover','rgb({},{},{})');
    r.style.setProperty('--sc-down','rgb({},{},{})');
  }}
  let st=document.getElementById('aurawave-theme');
  if(!st){{st=document.createElement('style');st.id='aurawave-theme';(document.head||r).appendChild(st);}}
  st.textContent={:?};
}};
if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',apply);else apply();
}})();"#,
        t.toolbar_bg.r,
        t.toolbar_bg.g,
        t.toolbar_bg.b,
        t.body_bg.r,
        t.body_bg.g,
        t.body_bg.b,
        t.border_color.r,
        t.border_color.g,
        t.border_color.b,
        t.text_primary.r,
        t.text_primary.g,
        t.text_primary.b,
        t.text_muted.r,
        t.text_muted.g,
        t.text_muted.b,
        t.accent_color.r,
        t.accent_color.g,
        t.accent_color.b,
        t.button_hover.r,
        t.button_hover.g,
        t.button_hover.b,
        t.button_down.r,
        t.button_down.g,
        t.button_down.b,
        theme_css
    )
}

fn build_audio_quality_script(prefer_hq: bool) -> String {
    format!(
        r#"(function(){{
if(window.top!==window||window.__scAudioQualityInit)return;
window.__scAudioQualityInit=true;
window.__scHqAudio={};
const score=(t)=>{{
  if(!t||typeof t!=='object')return -1;
  if(t.snipped)return -1000;
  const mime=String((t.format&&t.format.mime_type)||'').toLowerCase();
  const proto=String((t.format&&t.format.protocol)||'').toLowerCase();
  const preset=String(t.preset||'').toLowerCase();
  let s=t.quality==='hq'?1000:0;
  if(mime.includes('opus')||preset.includes('opus'))s+=300;
  else if(mime.includes('mp4a')||preset.includes('aac'))s+=200;
  else if(mime.includes('mpeg')||preset.includes('mp3'))s+=100;
  if(proto==='progressive')s+=20;
  else if(proto==='hls')s+=10;
  return s;
}};
const prioritize=(node,depth)=>{{
  if(!node||typeof node!=='object'||depth>5)return;
  if(Array.isArray(node)){{for(let i=0;i<node.length;i++)prioritize(node[i],depth+1);return;}}
  if(node.media&&Array.isArray(node.media.transcodings)&&node.media.transcodings.length>1){{
    node.media.transcodings=[...node.media.transcodings].sort((a,b)=>score(b)-score(a));
  }}
  for(const k of Object.keys(node)){{
    if(k!=='media'&&typeof node[k]==='object')prioritize(node[k],depth+1);
  }}
}};
const origFetch=window.fetch;
if(typeof origFetch==='function'){{
  window.fetch=async function(...args){{
    const resp=await origFetch.apply(this,args);
    if(!window.__scHqAudio)return resp;
    try{{
      const url=typeof args[0]==='string'?args[0]:(args[0]&&args[0].url)||'';
      if(!url.includes('api-v2.soundcloud.com/'))return resp;
      const ct=(resp.headers.get('content-type')||'').toLowerCase();
      if(!ct.includes('application/json'))return resp;
      const clone=resp.clone();
      const data=await clone.json();
      prioritize(data,0);
      return new Response(JSON.stringify(data),{{status:resp.status,statusText:resp.statusText,headers:resp.headers}});
    }}catch(_e){{
      return resp;
    }}
  }};
}}
}})();"#,
        prefer_hq
    )
}

fn show_and_focus(window: &Window) {
    window.set_visible(true);
    window.set_minimized(false);
    window.set_focus();
}

fn save_window_geometry(window: &Window, st: &AppState) {
    let mut s = st.settings.clone();
    let max = window.is_maximized();
    s.is_maximized = max;
    if !max && !st.mini_mode && !st.fullscreen {
        if let Ok(pos) = window.outer_position() {
            s.window_x = Some(pos.x);
            s.window_y = Some(pos.y);
        }
        let size = window.inner_size();
        let scale = window.scale_factor().max(1.0);
        let w = (size.width as f64 / scale).round() as i32;
        let h = (size.height as f64 / scale).round() as i32;
        if w >= 420 && h >= 140 {
            s.window_width = w;
            s.window_height = h;
        }
    }
    s.save();
}

fn apply_webview_zoom(webview: &WebView, zoom: f64) {
    let _ = webview.zoom(zoom);
}

fn build_tray(proxy: &EventLoopProxy<UserEvent>, initial_state: &AppState) -> Option<TrayState> {
    let tray_menu = Menu::new();
    let show_item = MenuItem::new("Show AuraWave", true, None);
    let play_item = MenuItem::new(if initial_state.playing { "Pause" } else { "Play" }, true, None);
    let next_item = MenuItem::new("Next", true, None);
    let prev_item = MenuItem::new("Previous", true, None);
    let lyrics_item = MenuItem::new("Toggle Lyrics (Alt+L)", true, None);
    let page_item = MenuItem::new("Open Lyrics Page", true, None);
    let theme_sub = Submenu::new("Theme", true);
    let mut theme_ids = Vec::new();
    for theme in themes::THEMES {
        let item = MenuItem::new(theme.name, true, None);
        theme_ids.push((item.id().clone(), theme.id));
        let _ = theme_sub.append(&item);
    }
    let pin_item = CheckMenuItem::new("Always on Top", true, initial_state.settings.always_on_top, None);
    let close_to_tray_item = CheckMenuItem::new("Close to Tray", true, initial_state.settings.close_to_tray, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let _ = tray_menu.append(&show_item);
    let _ = tray_menu.append(&PredefinedMenuItem::separator());
    let _ = tray_menu.append(&play_item);
    let _ = tray_menu.append(&next_item);
    let _ = tray_menu.append(&prev_item);
    let _ = tray_menu.append(&lyrics_item);
    let _ = tray_menu.append(&page_item);
    let _ = tray_menu.append(&PredefinedMenuItem::separator());
    let _ = tray_menu.append(&theme_sub);
    let _ = tray_menu.append(&pin_item);
    let _ = tray_menu.append(&close_to_tray_item);
    let _ = tray_menu.append(&PredefinedMenuItem::separator());
    let _ = tray_menu.append(&quit_item);

    let show_id = show_item.id().clone();
    let play_id = play_item.id().clone();
    let next_id = next_item.id().clone();
    let prev_id = prev_item.id().clone();
    let lyrics_id = lyrics_item.id().clone();
    let page_id = page_item.id().clone();
    let pin_id = pin_item.id().clone();
    let close_to_tray_id = close_to_tray_item.id().clone();
    let quit_id = quit_item.id().clone();

    let p1 = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |ev| {
        let _ = p1.send_event(UserEvent::Tray(ev));
    }));
    let p2 = proxy.clone();
    MenuEvent::set_event_handler(Some(move |ev| {
        let _ = p2.send_event(UserEvent::Menu(ev));
    }));

    let icon = load_tray_icon()?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("AuraWave")
        .with_icon(icon)
        .build()
        .ok()?;

    Some(TrayState {
        tray,
        show_id,
        play_id,
        next_id,
        prev_id,
        lyrics_id,
        page_id,
        pin_id,
        close_to_tray_id,
        quit_id,
        theme_ids,
        pin_item,
        close_to_tray_item,
        play_item,
    })
}

fn build_theme_menu(active_id: &str) -> (Menu, Vec<(MenuId, &'static str)>) {
    let menu = Menu::new();
    let mut theme_ids = Vec::new();
    for theme in themes::THEMES {
        let is_checked = theme.id.eq_ignore_ascii_case(active_id);
        let item = CheckMenuItem::new(theme.name, true, is_checked, None);
        theme_ids.push((item.id().clone(), theme.id));
        let _ = menu.append(&item);
    }
    (menu, theme_ids)
}

struct SettingsMenuEntries {
    menu: Menu,
    toolbar_id: MenuId,
    mini_id: MenuId,
    fullscreen_id: MenuId,
    theme_ids: Vec<(MenuId, &'static str)>,
    pin_id: MenuId,
    clean_id: MenuId,
    close_to_tray_id: MenuId,
    lyrics_page_id: MenuId,
    lyrics_lrclib_id: MenuId,
    lyrics_yandex_id: MenuId,
    prefer_hq_id: MenuId,
    zoom_in_id: MenuId,
    zoom_out_id: MenuId,
    zoom_reset_id: MenuId,
    copy_url_id: MenuId,
    open_browser_id: MenuId,
    quit_id: MenuId,
}

fn build_settings_menu(st: &AppState) -> SettingsMenuEntries {
    let menu = Menu::new();

    let tb_item = CheckMenuItem::new("Show Toolbar (Ctrl+H)", true, st.settings.show_toolbar, None);
    let mini_item = CheckMenuItem::new("Mini-Player (Ctrl+M)", true, st.mini_mode, None);
    let fs_item = CheckMenuItem::new("Fullscreen (F11)", true, st.fullscreen, None);

    let theme_sub = Submenu::new("Theme", true);
    let mut theme_ids = Vec::new();
    for theme in themes::THEMES {
        let is_checked = theme.id.eq_ignore_ascii_case(st.theme.id);
        let item = CheckMenuItem::new(theme.name, true, is_checked, None);
        theme_ids.push((item.id().clone(), theme.id));
        let _ = theme_sub.append(&item);
    }

    let pin_item = CheckMenuItem::new("Always on Top", true, st.settings.always_on_top, None);
    let clean_item = CheckMenuItem::new(
        "Block Ads && Upsells",
        true,
        st.settings.block_ads || st.settings.hide_upsell_banners,
        None,
    );
    let tray_item = CheckMenuItem::new("Close to Tray", true, st.settings.close_to_tray, None);

    let lyrics_sub = Submenu::new("Lyrics", true);
    let lyrics_page_item = MenuItem::new("Open Lyrics Page", true, None);
    let is_yandex = st.settings.lyrics_provider.eq_ignore_ascii_case("yandex");
    let lrclib_item = CheckMenuItem::new("LRCLIB (Default, Yandex fallback)", true, !is_yandex, None);
    let yandex_item = CheckMenuItem::new("Yandex Music (LRCLIB fallback)", true, is_yandex, None);
    let _ = lyrics_sub.append(&lyrics_page_item);
    let _ = lyrics_sub.append(&PredefinedMenuItem::separator());
    let _ = lyrics_sub.append(&lrclib_item);
    let _ = lyrics_sub.append(&yandex_item);

    let prefer_hq_item = CheckMenuItem::new(
        "Prefer HQ Audio (Opus)",
        true,
        st.settings.prefer_hq_audio,
        None,
    );

    let zoom_in_item = MenuItem::new("Zoom In (Ctrl++)", true, None);
    let zoom_out_item = MenuItem::new("Zoom Out (Ctrl+-)", true, None);
    let zoom_reset_item = MenuItem::new("Reset Zoom (Ctrl+0)", true, None);

    let copy_url_item = MenuItem::new("Copy Track URL", true, None);
    let open_browser_item = MenuItem::new("Open Current Page in Browser", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let _ = menu.append(&tb_item);
    let _ = menu.append(&mini_item);
    let _ = menu.append(&fs_item);
    let _ = menu.append(&theme_sub);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&pin_item);
    let _ = menu.append(&clean_item);
    let _ = menu.append(&tray_item);
    let _ = menu.append(&lyrics_sub);
    let _ = menu.append(&prefer_hq_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&zoom_in_item);
    let _ = menu.append(&zoom_out_item);
    let _ = menu.append(&zoom_reset_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&copy_url_item);
    let _ = menu.append(&open_browser_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit_item);

    SettingsMenuEntries {
        toolbar_id: tb_item.id().clone(),
        mini_id: mini_item.id().clone(),
        fullscreen_id: fs_item.id().clone(),
        theme_ids,
        pin_id: pin_item.id().clone(),
        clean_id: clean_item.id().clone(),
        close_to_tray_id: tray_item.id().clone(),
        lyrics_page_id: lyrics_page_item.id().clone(),
        lyrics_lrclib_id: lrclib_item.id().clone(),
        lyrics_yandex_id: yandex_item.id().clone(),
        prefer_hq_id: prefer_hq_item.id().clone(),
        zoom_in_id: zoom_in_item.id().clone(),
        zoom_out_id: zoom_out_item.id().clone(),
        zoom_reset_id: zoom_reset_item.id().clone(),
        copy_url_id: copy_url_item.id().clone(),
        open_browser_id: open_browser_item.id().clone(),
        quit_id: quit_item.id().clone(),
        menu,
    }
}

fn sync_toolbar_state(toolbar_webview: &WebView, st: &AppState) {
    let tb_state = st.to_toolbar_state();
    if let Ok(json) = serde_json::to_string(&tb_state) {
        let script = format!("window.__tbUpdate&&window.__tbUpdate({});", json);
        let _ = toolbar_webview.evaluate_script(&script);
    }
}

fn update_layout(
    window: &Window,
    toolbar_webview: &WebView,
    content_webview: &WebView,
    st: &AppState,
) {
    let size = window.inner_size();
    let scale = window.scale_factor();
    let toolbar_visible = st.settings.show_toolbar && !st.mini_mode && !st.fullscreen;
    let bounds = toolbar::compute_webview_bounds(size.width, size.height, scale, toolbar_visible);

    let _ = toolbar_webview.set_bounds(bounds.toolbar);
    let _ = toolbar_webview.set_visible(bounds.show_toolbar);
    let _ = content_webview.set_bounds(bounds.content);
}

fn handle_theme_selection(
    window: &Window,
    toolbar_wv: &WebView,
    content_wv: &WebView,
    st: &mut AppState,
    theme_id: &str,
) {
    let t = themes::get_theme(theme_id);
    st.theme = t;
    st.settings.theme_id = t.id.to_string();
    st.settings.save();

    window.set_theme(Some(if t.dark_title_bar {
        TaoTheme::Dark
    } else {
        TaoTheme::Light
    }));
    apply_dwm_theme(window, t);

    let js = build_theme_css_script(t);
    let _ = content_wv.evaluate_script(&js);
    sync_toolbar_state(toolbar_wv, st);
}

fn handle_mini_toggle(
    window: &Window,
    toolbar_wv: &WebView,
    content_wv: &WebView,
    st: &mut AppState,
) {
    if st.fullscreen {
        handle_fullscreen_toggle(window, toolbar_wv, content_wv, st);
    }
    if !st.mini_mode {
        let pos = window.outer_position().unwrap_or(PhysicalPosition::new(100, 100));
        let size = window.inner_size();
        st.normal_bounds = Some((pos.x, pos.y, size.width, size.height));
        st.normal_maximized = window.is_maximized();

        st.mini_mode = true;
        if window.is_maximized() {
            window.set_maximized(false);
        }
        window.set_min_inner_size(Some(LogicalSize::new(360.0, 110.0)));
        window.set_always_on_top(true);
        window.set_inner_size(LogicalSize::new(480.0, 148.0));
        let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.setMiniMode(true);");
    } else {
        st.mini_mode = false;
        let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.setMiniMode(false);");
        window.set_min_inner_size(Some(LogicalSize::new(420.0, 140.0)));
        window.set_always_on_top(st.settings.always_on_top);

        if let Some((x, y, w, h)) = st.normal_bounds {
            window.set_outer_position(PhysicalPosition::new(x, y));
            window.set_inner_size(PhysicalSize::new(w, h));
        }
        if st.normal_maximized {
            window.set_maximized(true);
        }
    }
    update_layout(window, toolbar_wv, content_wv, st);
    sync_toolbar_state(toolbar_wv, st);
}

fn handle_fullscreen_toggle(
    window: &Window,
    toolbar_wv: &WebView,
    content_wv: &WebView,
    st: &mut AppState,
) {
    if st.mini_mode {
        handle_mini_toggle(window, toolbar_wv, content_wv, st);
    }
    if !st.fullscreen {
        let pos = window.outer_position().unwrap_or(PhysicalPosition::new(100, 100));
        let size = window.inner_size();
        st.fs_saved_bounds = Some((pos.x, pos.y, size.width, size.height));
        st.fs_saved_maximized = window.is_maximized();

        st.fullscreen = true;
        window.set_fullscreen(Some(TaoFullscreen::Borderless(None)));
    } else {
        st.fullscreen = false;
        window.set_fullscreen(None);
        if let Some((x, y, w, h)) = st.fs_saved_bounds {
            window.set_outer_position(PhysicalPosition::new(x, y));
            window.set_inner_size(PhysicalSize::new(w, h));
        }
        if st.fs_saved_maximized {
            window.set_maximized(true);
        }
    }
    update_layout(window, toolbar_wv, content_wv, st);
    sync_toolbar_state(toolbar_wv, st);
}

fn adjust_zoom(content_wv: &WebView, st: &mut AppState, delta: f64) {
    set_zoom(content_wv, st, st.settings.zoom_factor + delta);
}

fn set_zoom(content_wv: &WebView, st: &mut AppState, factor: f64) {
    let clamped = (factor * 100.0).round() / 100.0;
    st.settings.zoom_factor = clamped.clamp(0.5, 2.0);
    apply_webview_zoom(content_wv, st.settings.zoom_factor);
    st.settings.save();
}

fn parse_content_ipc(body: &str) -> Option<UserEvent> {
    #[derive(serde::Deserialize)]
    #[serde(tag = "type")]
    enum Msg {
        #[serde(rename = "playerState")]
        PlayerState {
            playing: bool,
            #[serde(default)]
            title: String,
            #[serde(default)]
            artist: String,
            #[serde(default, rename = "trackUrl")]
            track_url: String,
            #[serde(default, rename = "pageUrl")]
            page_url: String,
        },
        #[serde(rename = "lyricsState")]
        LyricsState { open: bool },
        #[serde(rename = "shortcut")]
        Shortcut { name: String },
        #[serde(rename = "nav")]
        Nav { action: String },
    }

    if let Ok(m) = serde_json::from_str::<Msg>(body) {
        match m {
            Msg::PlayerState {
                playing,
                title,
                artist,
                track_url,
                page_url,
            } => Some(UserEvent::PlayerState {
                playing,
                title,
                artist,
                track_url,
                page_url,
            }),
            Msg::LyricsState { open } => Some(UserEvent::LyricsState { open }),
            Msg::Shortcut { name } => Some(UserEvent::Shortcut(name)),
            Msg::Nav { action } => Some(UserEvent::Nav(action)),
        }
    } else {
        None
    }
}

fn handle_shortcut(
    name: &str,
    window: &Window,
    toolbar_wv: &WebView,
    content_wv: &WebView,
    st: &mut AppState,
    _tray_state: &Option<TrayState>,
    control_flow: &mut ControlFlow,
) {
    match name {
        "toggle_toolbar" => {
            st.settings.show_toolbar = !st.settings.show_toolbar;
            st.settings.save();
            update_layout(window, toolbar_wv, content_wv, st);
            sync_toolbar_state(toolbar_wv, st);
        }
        "mini_mode" => {
            handle_mini_toggle(window, toolbar_wv, content_wv, st);
        }
        "fullscreen" => {
            handle_fullscreen_toggle(window, toolbar_wv, content_wv, st);
        }
        "toggle_lyrics" => {
            let _ = content_wv.evaluate_script("window.__scToggleLyrics&&window.__scToggleLyrics();");
        }
        "open_lyrics_page" => {
            let _ = content_wv.evaluate_script("window.__scLyricsOpenPage&&window.__scLyricsOpenPage();");
        }
        "zoom_in" => {
            adjust_zoom(content_wv, st, 0.1);
        }
        "zoom_out" => {
            adjust_zoom(content_wv, st, -0.1);
        }
        "zoom_reset" => {
            set_zoom(content_wv, st, 1.0);
        }
        "copy_url" => {
            let _ = toolbar::copy_to_clipboard(window.hwnd() as isize, &st.best_track_url());
        }
        "open_browser" => {
            let _ = lyrics::open_external_url(&st.best_track_url());
        }
        "close" => {
            save_window_geometry(window, st);
            if st.settings.close_to_tray {
                window.set_visible(false);
            } else {
                *control_flow = ControlFlow::Exit;
            }
        }
        "play_pause" => {
            let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.playPause();");
        }
        "prev" => {
            let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.prev();");
        }
        "next" => {
            let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.next();");
        }
        _ => {}
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pending_url = args.get(1).filter(|a| !a.starts_with('-')).cloned();

    let _guard = match ipc_instance::SingleInstanceGuard::acquire_or_notify(pending_url.as_deref()) {
        Some(g) => g,
        None => return,
    };

    let settings = settings::AppSettings::load();
    let mut st = AppState::new(settings);

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let pipe_stop = Arc::new(AtomicBool::new(false));
    let p_pipe = proxy.clone();
    ipc_instance::spawn_pipe_server(pipe_stop.clone(), move |msg| {
        let _ = p_pipe.send_event(UserEvent::PipeMessage(msg));
    });

    let mut wb = WindowBuilder::new()
        .with_title("AuraWave - SoundCloud Desktop")
        .with_inner_size(LogicalSize::new(
            st.settings.window_width as f64,
            st.settings.window_height as f64,
        ))
        .with_min_inner_size(LogicalSize::new(420.0, 140.0))
        .with_always_on_top(st.settings.always_on_top)
        .with_theme(Some(if st.theme.dark_title_bar {
            TaoTheme::Dark
        } else {
            TaoTheme::Light
        }));

    if let (Some(x), Some(y)) = (st.settings.window_x, st.settings.window_y) {
        wb = wb.with_position(PhysicalPosition::new(x, y));
    }
    if let Some(icon) = load_window_icon() {
        wb = wb.with_window_icon(Some(icon));
    }

    let window = wb.build(&event_loop).expect("Failed to build window");
    apply_dwm_theme(&window, st.theme);
    if st.settings.is_maximized {
        window.set_maximized(true);
    }

    let tray_state = build_tray(&proxy, &st);

    let win_size = window.inner_size();
    let scale = window.scale_factor();
    let toolbar_visible = st.settings.show_toolbar && !st.mini_mode && !st.fullscreen;
    let bounds = toolbar::compute_webview_bounds(win_size.width, win_size.height, scale, toolbar_visible);

    let mut web_context = WebContext::new(Some(settings::AppSettings::webview_data_folder()));

    let p_tb = proxy.clone();
    let toolbar_wv = WebViewBuilder::new()
        .with_html(toolbar::TOOLBAR_HTML)
        .with_bounds(bounds.toolbar)
        .with_visible(bounds.show_toolbar)
        .with_ipc_handler(move |req| {
            if let Some(action) = toolbar::parse_toolbar_action(req.body()) {
                let _ = p_tb.send_event(UserEvent::Toolbar(action));
            }
        })
        .build_as_child(&window)
        .expect("Failed to build toolbar webview");

    let initial_url = pending_url
        .filter(|u| settings::is_valid_soundcloud_url(u))
        .unwrap_or_else(|| st.settings.home_url.clone());

    let adblock_enabled = Arc::new(AtomicBool::new(
        st.settings.block_ads || st.settings.hide_upsell_banners,
    ));

    let theme_script = build_theme_css_script(st.theme);
    let adblock_script = adblock::build_dom_adblock_script(
        st.settings.block_ads,
        st.settings.hide_upsell_banners,
    );
    let hq_script = build_audio_quality_script(st.settings.prefer_hq_audio);

    let p_lyrics = proxy.clone();
    let p_content = proxy.clone();
    let content_wv = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_url(&initial_url)
        .with_bounds(bounds.content)
        .with_visible(true)
        .with_initialization_script(&theme_script)
        .with_initialization_script(&adblock_script)
        .with_initialization_script(&hq_script)
        .with_initialization_script(toolbar::DESKTOP_BRIDGE_JS)
        .with_initialization_script(lyrics::OVERLAY_JS)
        .with_ipc_handler(move |req| {
            let body = req.body();
            if let Some(resp) = lyrics::handle_request(body) {
                let _ = p_lyrics.send_event(UserEvent::Lyrics(resp));
            } else if let Some(ev) = parse_content_ipc(body) {
                let _ = p_content.send_event(ev);
            }
        })
        .build_as_child(&window)
        .expect("Failed to build content webview");

    unsafe {
        if let Ok(core) = content_wv.controller().CoreWebView2() {
            adblock::install_adblock_filter(&core, adblock_enabled.clone());

            let p_hist = proxy.clone();
            let handler = HistoryChangedEventHandler::create(Box::new(move |_sender, _args| {
                let _ = p_hist.send_event(UserEvent::HistoryChanged);
                Ok(())
            }));
            let mut token = 0i64;
            let _ = core.add_HistoryChanged(&handler, &mut token);
        }
    }

    apply_webview_zoom(&content_wv, st.settings.zoom_factor);
    sync_toolbar_state(&toolbar_wv, &st);

    let mut active_theme_ids: Vec<(MenuId, &'static str)> = Vec::new();
    let mut active_settings_entries: Option<SettingsMenuEntries> = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    update_layout(&window, &toolbar_wv, &content_wv, &st);
                }
                WindowEvent::Moved(_) => {
                    save_window_geometry(&window, &st);
                }
                WindowEvent::CloseRequested => {
                    save_window_geometry(&window, &st);
                    if st.settings.close_to_tray {
                        window.set_visible(false);
                    } else {
                        pipe_stop.store(true, Ordering::Relaxed);
                        *control_flow = ControlFlow::Exit;
                    }
                }
                _ => {}
            },
            Event::UserEvent(user_event) => match user_event {
                UserEvent::PipeMessage(msg) => {
                    show_and_focus(&window);
                    let target = msg.trim();
                    let url = if let Some(stripped) = target.strip_prefix("NAV:") {
                        stripped.trim()
                    } else {
                        target
                    };
                    if settings::is_valid_soundcloud_url(url) {
                        let _ = content_wv.load_url(url);
                    }
                }
                UserEvent::HistoryChanged => unsafe {
                    if let Ok(core) = content_wv.controller().CoreWebView2() {
                        let mut can_back = BOOL(0);
                        if core.CanGoBack(&mut can_back).is_ok() {
                            st.can_go_back = can_back.as_bool();
                        }
                        let mut can_forward = BOOL(0);
                        if core.CanGoForward(&mut can_forward).is_ok() {
                            st.can_go_forward = can_forward.as_bool();
                        }
                        let mut uri = PWSTR::null();
                        if core.Source(&mut uri).is_ok() && !uri.is_null() {
                            st.current_page_url = adblock::pwstr_to_string(uri);
                            CoTaskMemFree(Some(uri.as_ptr() as *const _));
                        }
                        sync_toolbar_state(&toolbar_wv, &st);
                    }
                },
                UserEvent::PlayerState {
                    playing,
                    title,
                    artist,
                    track_url,
                    page_url,
                } => {
                    st.playing = playing;
                    let label = if !title.is_empty() && !artist.is_empty() {
                        format!("{} - {}", title, artist)
                    } else if !title.is_empty() {
                        title
                    } else {
                        "SoundCloud".to_string()
                    };
                    st.track_label = label;
                    if !track_url.is_empty() {
                        st.track_url = track_url;
                    }
                    if !page_url.is_empty() {
                        st.current_page_url = page_url;
                    }
                    if let Some(tray) = &tray_state {
                        let _ = tray.tray.set_tooltip(Some(format!("AuraWave - {}", st.track_label)));
                        tray.play_item.set_text(if st.playing { "Pause" } else { "Play" });
                    }
                    sync_toolbar_state(&toolbar_wv, &st);
                }
                UserEvent::LyricsState { open } => {
                    st.lyrics_open = open;
                    sync_toolbar_state(&toolbar_wv, &st);
                }
                UserEvent::Lyrics(payload) => {
                    let script = format!("window.__scLyricsReply&&window.__scLyricsReply({});", payload);
                    let _ = content_wv.evaluate_script(&script);
                }
                UserEvent::Shortcut(name) => {
                    handle_shortcut(
                        &name,
                        &window,
                        &toolbar_wv,
                        &content_wv,
                        &mut st,
                        &tray_state,
                        control_flow,
                    );
                }
                UserEvent::Nav(action) => match action.as_str() {
                    "home" => {
                        let _ = content_wv.load_url(&st.settings.home_url);
                    }
                    "back" => unsafe {
                        if let Ok(core) = content_wv.controller().CoreWebView2() {
                            let _ = core.GoBack();
                        }
                    },
                    "forward" => unsafe {
                        if let Ok(core) = content_wv.controller().CoreWebView2() {
                            let _ = core.GoForward();
                        }
                    },
                    "reload" => unsafe {
                        if let Ok(core) = content_wv.controller().CoreWebView2() {
                            let _ = core.Reload();
                        }
                    },
                    _ => {}
                },
                UserEvent::Tray(ev) => match ev {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        ..
                    }
                    | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } => {
                        show_and_focus(&window);
                    }
                    _ => {}
                },
                UserEvent::Menu(ev) => {
                    if let Some(tray) = &tray_state {
                        if ev.id == tray.show_id {
                            show_and_focus(&window);
                        } else if ev.id == tray.play_id {
                            let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.playPause();");
                        } else if ev.id == tray.next_id {
                            let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.next();");
                        } else if ev.id == tray.prev_id {
                            let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.prev();");
                        } else if ev.id == tray.lyrics_id {
                            let _ = content_wv.evaluate_script("window.__scToggleLyrics&&window.__scToggleLyrics();");
                        } else if ev.id == tray.page_id {
                            let _ = content_wv.evaluate_script("window.__scLyricsOpenPage&&window.__scLyricsOpenPage();");
                        } else if ev.id == tray.pin_id {
                            st.settings.always_on_top = !st.settings.always_on_top;
                            window.set_always_on_top(st.settings.always_on_top);
                            st.settings.save();
                            tray.pin_item.set_checked(st.settings.always_on_top);
                            sync_toolbar_state(&toolbar_wv, &st);
                        } else if ev.id == tray.close_to_tray_id {
                            st.settings.close_to_tray = !st.settings.close_to_tray;
                            st.settings.save();
                            tray.close_to_tray_item.set_checked(st.settings.close_to_tray);
                        } else if ev.id == tray.quit_id {
                            save_window_geometry(&window, &st);
                            pipe_stop.store(true, Ordering::Relaxed);
                            *control_flow = ControlFlow::Exit;
                        } else {
                            for (t_id, theme_name) in &tray.theme_ids {
                                if ev.id == *t_id {
                                    handle_theme_selection(&window, &toolbar_wv, &content_wv, &mut st, theme_name);
                                    break;
                                }
                            }
                        }
                    }
                    for (t_id, theme_name) in &active_theme_ids {
                        if ev.id == *t_id {
                            handle_theme_selection(&window, &toolbar_wv, &content_wv, &mut st, theme_name);
                            break;
                        }
                    }
                    if let Some(se) = &active_settings_entries {
                        if ev.id == se.toolbar_id {
                            st.settings.show_toolbar = !st.settings.show_toolbar;
                            st.settings.save();
                            update_layout(&window, &toolbar_wv, &content_wv, &st);
                            sync_toolbar_state(&toolbar_wv, &st);
                        } else if ev.id == se.mini_id {
                            handle_mini_toggle(&window, &toolbar_wv, &content_wv, &mut st);
                        } else if ev.id == se.fullscreen_id {
                            handle_fullscreen_toggle(&window, &toolbar_wv, &content_wv, &mut st);
                        } else if ev.id == se.pin_id {
                            st.settings.always_on_top = !st.settings.always_on_top;
                            window.set_always_on_top(st.settings.always_on_top);
                            st.settings.save();
                            if let Some(tray) = &tray_state {
                                tray.pin_item.set_checked(st.settings.always_on_top);
                            }
                            sync_toolbar_state(&toolbar_wv, &st);
                        } else if ev.id == se.clean_id {
                            let cur = st.settings.block_ads || st.settings.hide_upsell_banners;
                            let new_val = !cur;
                            st.settings.block_ads = new_val;
                            st.settings.hide_upsell_banners = new_val;
                            adblock_enabled.store(new_val, Ordering::Relaxed);
                            st.settings.save();
                            let js = adblock::build_dom_adblock_script(new_val, new_val);
                            let _ = content_wv.evaluate_script(&js);
                        } else if ev.id == se.close_to_tray_id {
                            st.settings.close_to_tray = !st.settings.close_to_tray;
                            st.settings.save();
                            if let Some(tray) = &tray_state {
                                tray.close_to_tray_item.set_checked(st.settings.close_to_tray);
                            }
                        } else if ev.id == se.lyrics_page_id {
                            let _ = content_wv.evaluate_script("window.__scLyricsOpenPage&&window.__scLyricsOpenPage();");
                        } else if ev.id == se.lyrics_lrclib_id {
                            st.settings.lyrics_provider = "lrclib".to_string();
                            st.settings.save();
                        } else if ev.id == se.lyrics_yandex_id {
                            st.settings.lyrics_provider = "yandex".to_string();
                            st.settings.save();
                        } else if ev.id == se.prefer_hq_id {
                            st.settings.prefer_hq_audio = !st.settings.prefer_hq_audio;
                            st.settings.save();
                            let js = build_audio_quality_script(st.settings.prefer_hq_audio);
                            let _ = content_wv.evaluate_script(&js);
                        } else if ev.id == se.zoom_in_id {
                            adjust_zoom(&content_wv, &mut st, 0.1);
                        } else if ev.id == se.zoom_out_id {
                            adjust_zoom(&content_wv, &mut st, -0.1);
                        } else if ev.id == se.zoom_reset_id {
                            set_zoom(&content_wv, &mut st, 1.0);
                        } else if ev.id == se.copy_url_id {
                            let _ = toolbar::copy_to_clipboard(window.hwnd() as isize, &st.best_track_url());
                        } else if ev.id == se.open_browser_id {
                            let _ = lyrics::open_external_url(&st.best_track_url());
                        } else if ev.id == se.quit_id {
                            save_window_geometry(&window, &st);
                            pipe_stop.store(true, Ordering::Relaxed);
                            *control_flow = ControlFlow::Exit;
                        } else {
                            for (t_id, theme_name) in &se.theme_ids {
                                if ev.id == *t_id {
                                    handle_theme_selection(&window, &toolbar_wv, &content_wv, &mut st, theme_name);
                                    break;
                                }
                            }
                        }
                    }
                }
                UserEvent::Toolbar(action) => match action {
                    toolbar::ToolbarAction::Back => unsafe {
                        if let Ok(core) = content_wv.controller().CoreWebView2() {
                            let _ = core.GoBack();
                        }
                    },
                    toolbar::ToolbarAction::Forward => unsafe {
                        if let Ok(core) = content_wv.controller().CoreWebView2() {
                            let _ = core.GoForward();
                        }
                    },
                    toolbar::ToolbarAction::Reload => unsafe {
                        if let Ok(core) = content_wv.controller().CoreWebView2() {
                            let _ = core.Reload();
                        }
                    },
                    toolbar::ToolbarAction::Navigate { url } => {
                        let target = if url.eq_ignore_ascii_case("home") {
                            st.settings.home_url.clone()
                        } else {
                            url
                        };
                        if settings::is_valid_soundcloud_url(&target) {
                            let _ = content_wv.load_url(&target);
                        }
                    }
                    toolbar::ToolbarAction::CopyTrackUrl => {
                        let url = st.best_track_url();
                        let _ = toolbar::copy_to_clipboard(window.hwnd() as isize, &url);
                    }
                    toolbar::ToolbarAction::Prev => {
                        let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.prev();");
                    }
                    toolbar::ToolbarAction::PlayPause => {
                        let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.playPause();");
                    }
                    toolbar::ToolbarAction::Next => {
                        let _ = content_wv.evaluate_script("window.__scDesktop&&window.__scDesktop.next();");
                    }
                    toolbar::ToolbarAction::ToggleLyrics => {
                        let _ = content_wv.evaluate_script("window.__scToggleLyrics&&window.__scToggleLyrics();");
                    }
                    toolbar::ToolbarAction::OpenLyricsPage => {
                        let _ = content_wv.evaluate_script("window.__scLyricsOpenPage&&window.__scLyricsOpenPage();");
                    }
                    toolbar::ToolbarAction::ShowThemeMenu { x, y } => {
                        let (menu, ids) = build_theme_menu(st.theme.id);
                        unsafe {
                            let _ = menu.show_context_menu_for_hwnd(
                                window.hwnd() as isize,
                                Some(muda::dpi::LogicalPosition::new(x, y).into()),
                            );
                        }
                        active_theme_ids = ids;
                    }
                    toolbar::ToolbarAction::ToggleMini => {
                        handle_mini_toggle(&window, &toolbar_wv, &content_wv, &mut st);
                    }
                    toolbar::ToolbarAction::TogglePin => {
                        st.settings.always_on_top = !st.settings.always_on_top;
                        window.set_always_on_top(st.settings.always_on_top);
                        st.settings.save();
                        if let Some(tray) = &tray_state {
                            tray.pin_item.set_checked(st.settings.always_on_top);
                        }
                        sync_toolbar_state(&toolbar_wv, &st);
                    }
                    toolbar::ToolbarAction::ShowSettingsMenu { x, y } => {
                        let entries = build_settings_menu(&st);
                        unsafe {
                            let _ = entries.menu.show_context_menu_for_hwnd(
                                window.hwnd() as isize,
                                Some(muda::dpi::LogicalPosition::new(x, y).into()),
                            );
                        }
                        active_settings_entries = Some(entries);
                    }
                    toolbar::ToolbarAction::Shortcut { name } => {
                        handle_shortcut(
                            &name,
                            &window,
                            &toolbar_wv,
                            &content_wv,
                            &mut st,
                            &tray_state,
                            control_flow,
                        );
                    }
                },
            },
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_ipc() {
        let msg = r#"{"type":"playerState","playing":true,"title":"My Song","artist":"My Artist","trackUrl":"https://soundcloud.com/artist/song","pageUrl":"https://soundcloud.com/discover"}"#;
        match parse_content_ipc(msg) {
            Some(UserEvent::PlayerState {
                playing,
                title,
                artist,
                track_url,
                page_url,
            }) => {
                assert!(playing);
                assert_eq!(title, "My Song");
                assert_eq!(artist, "My Artist");
                assert_eq!(track_url, "https://soundcloud.com/artist/song");
                assert_eq!(page_url, "https://soundcloud.com/discover");
            }
            _ => panic!("Expected PlayerState"),
        }

        let l_msg = r#"{"type":"lyricsState","open":true}"#;
        match parse_content_ipc(l_msg) {
            Some(UserEvent::LyricsState { open }) => assert!(open),
            _ => panic!("Expected LyricsState"),
        }

        let s_msg = r#"{"type":"shortcut","name":"toggle_toolbar"}"#;
        match parse_content_ipc(s_msg) {
            Some(UserEvent::Shortcut(name)) => assert_eq!(name, "toggle_toolbar"),
            _ => panic!("Expected Shortcut"),
        }

        let n_msg = r#"{"type":"nav","action":"home"}"#;
        match parse_content_ipc(n_msg) {
            Some(UserEvent::Nav(act)) => assert_eq!(act, "home"),
            _ => panic!("Expected Nav"),
        }

        assert!(parse_content_ipc("invalid json").is_none());
        assert!(parse_content_ipc(r#"{"type":"unknown"}"#).is_none());
    }

    #[test]
    fn test_app_state_best_track_url() {
        let mut st = AppState::new(settings::AppSettings::default());
        st.track_url = "".to_string();
        st.current_page_url = "".to_string();
        assert_eq!(st.best_track_url(), "https://soundcloud.com");

        st.current_page_url = "https://soundcloud.com/discover".to_string();
        assert_eq!(st.best_track_url(), "https://soundcloud.com/discover");

        st.track_url = "https://soundcloud.com/artist/track".to_string();
        assert_eq!(st.best_track_url(), "https://soundcloud.com/artist/track");
    }

    #[test]
    fn test_app_state_to_toolbar_state() {
        let mut st = AppState::new(settings::AppSettings::default());
        st.playing = true;
        st.can_go_back = true;
        st.track_label = "Song Title".to_string();
        let tb = st.to_toolbar_state();
        assert!(tb.playing);
        assert!(tb.can_go_back);
        assert_eq!(tb.track_label, "Song Title");
        assert_eq!(tb.theme.accent_color, "#a855f7");
    }

    #[test]
    fn test_theme_and_audio_scripts() {
        let theme = themes::get_theme("aurawave");
        let script = build_theme_css_script(theme);
        assert!(script.contains("--sc-toolbar"));
        assert!(script.contains("--sc-accent"));
        assert!(script.contains("aurawave-theme"));
        assert!(script.contains("playbackSoundBadge__showQueue"));

        let hq_true = build_audio_quality_script(true);
        assert!(hq_true.contains("window.__scHqAudio=true;"));
        let hq_false = build_audio_quality_script(false);
        assert!(hq_false.contains("window.__scHqAudio=false;"));
    }

    #[test]
    fn test_menu_builders() {
        let (theme_menu, theme_ids) = build_theme_menu("aurawave");
        assert_eq!(theme_ids.len(), themes::THEMES.len());
        assert_eq!(theme_menu.items().len(), themes::THEMES.len());

        let st = AppState::new(settings::AppSettings::default());
        let settings_entries = build_settings_menu(&st);
        assert_eq!(settings_entries.theme_ids.len(), themes::THEMES.len());
        assert!(!settings_entries.menu.items().is_empty());
    }
}
