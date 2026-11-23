#![windows_subsystem = "windows"]
mod adblock;
mod ipc_instance;
mod lyrics;
mod settings;
mod themes;

use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tao::dpi::{LogicalSize, PhysicalPosition};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::windows::WindowExtWindows;
use tao::window::{Icon as WindowIcon, Theme as TaoTheme, Window, WindowBuilder};
use tray_icon::{MouseButton, TrayIconBuilder, TrayIconEvent};
use windows::core::BOOL;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWINDOWATTRIBUTE, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
};
use wry::{WebContext, WebView, WebViewBuilder, WebViewExtWindows};

const ICON_RGBA_64: &[u8] = include_bytes!("../assets/aurawave_rgba.bin");
const ICON_SIZE: u32 = 64;

enum UserEvent {
    Lyrics(String),
    PipeMessage(String),
    Tray(TrayIconEvent),
    Menu(MenuEvent),
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
        ".playbackSoundBadge__lightLink,.playbackTimeline__timePassed{color:var(--sc-muted)!important;}"
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

/// Legitimate client-side audio preference: prioritizes the highest-quality non-paywalled
/// standard (`sq`, non-snipped) transcoding already returned by SoundCloud (e.g. Opus over MP3).
/// Does NOT spoof subscription/Go+ flags or touch paywalled/DRM `hq` streams.
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

fn save_window_geometry(window: &Window) {
    let mut s = settings::AppSettings::load();
    let max = window.is_maximized();
    s.is_maximized = max;
    if !max {
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

struct TrayState {
    _tray: tray_icon::TrayIcon,
    show_id: muda::MenuId,
    lyrics_id: muda::MenuId,
    page_id: muda::MenuId,
    quit_id: muda::MenuId,
    theme_ids: Vec<(muda::MenuId, &'static str)>,
}

fn build_tray(proxy: &tao::event_loop::EventLoopProxy<UserEvent>) -> Option<TrayState> {
    let tray_menu = Menu::new();
    let show_item = MenuItem::new("Show AuraWave", true, None);
    let lyrics_item = MenuItem::new("Toggle Lyrics (Alt+L)", true, None);
    let page_item = MenuItem::new("Open Lyrics Page", true, None);
    let theme_sub = Submenu::new("Theme", true);
    let mut theme_ids = Vec::new();
    for theme in themes::THEMES {
        let item = MenuItem::new(theme.name, true, None);
        theme_ids.push((item.id().clone(), theme.id));
        let _ = theme_sub.append(&item);
    }
    let quit_item = MenuItem::new("Quit", true, None);

    let _ = tray_menu.append(&show_item);
    let _ = tray_menu.append(&lyrics_item);
    let _ = tray_menu.append(&page_item);
    let _ = tray_menu.append(&theme_sub);
    let _ = tray_menu.append(&PredefinedMenuItem::separator());
    let _ = tray_menu.append(&quit_item);

    let show_id = show_item.id().clone();
    let lyrics_id = lyrics_item.id().clone();
    let page_id = page_item.id().clone();
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
        _tray: tray,
        show_id,
        lyrics_id,
        page_id,
        quit_id,
        theme_ids,
    })
}

fn apply_webview_zoom(webview: &WebView, zoom: f64) {
    let _ = webview.zoom(zoom);
}

fn main() {
    let arg_url = std::env::args()
        .nth(1)
        .filter(|u| settings::is_valid_soundcloud_url(u));
    let Some(_instance_guard) =
        ipc_instance::SingleInstanceGuard::acquire_or_notify(arg_url.as_deref())
    else {
        return;
    };

    let s = settings::AppSettings::load();
    let theme = themes::get_theme(&s.theme_id);

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let stop_pipe = Arc::new(AtomicBool::new(false));
    let pipe_proxy = proxy.clone();
    ipc_instance::spawn_pipe_server(stop_pipe.clone(), move |msg| {
        let _ = pipe_proxy.send_event(UserEvent::PipeMessage(msg));
    });

    let mut wb = WindowBuilder::new()
        .with_title("AuraWave")
        .with_inner_size(LogicalSize::new(s.window_width as f64, s.window_height as f64))
        .with_always_on_top(s.always_on_top)
        .with_maximized(s.is_maximized)
        .with_window_icon(load_window_icon())
        .with_theme(Some(if theme.dark_title_bar {
            TaoTheme::Dark
        } else {
            TaoTheme::Light
        }));

    if let (Some(x), Some(y)) = (s.window_x, s.window_y) {
        wb = wb.with_position(PhysicalPosition::new(x, y));
    }

    let window = wb.build(&event_loop).expect("window");
    apply_dwm_theme(&window, theme);

    let tray_state = build_tray(&proxy);

    let mut ctx = WebContext::new(Some(settings::AppSettings::webview_data_folder()));
    let ads = Arc::new(AtomicBool::new(s.block_ads));
    let ipc_proxy = proxy.clone();
    let url = arg_url
        .or_else(|| {
            if settings::is_valid_soundcloud_url(&s.home_url) {
                Some(s.home_url.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "https://soundcloud.com/discover".to_string());

    let theme_script = build_theme_css_script(theme);
    let adblock_script = adblock::build_dom_adblock_script(s.block_ads, s.hide_upsell_banners);
    let audio_script = build_audio_quality_script(s.prefer_hq_audio);
    let init_script = format!(
        "{}\n{}\n{}\n{}",
        theme_script, adblock_script, audio_script, lyrics::OVERLAY_JS
    );

    let webview = WebViewBuilder::new_with_web_context(&mut ctx)
        .with_url(&url)
        .with_background_color((theme.body_bg.r, theme.body_bg.g, theme.body_bg.b, 255))
        .with_initialization_script(&init_script)
        .with_ipc_handler(move |req| {
            let body = req.body().to_string();
            let p = ipc_proxy.clone();
            std::thread::spawn(move || {
                if let Some(json) = lyrics::handle_request(&body) {
                    let _ = p.send_event(UserEvent::Lyrics(json));
                }
            });
        })
        .build(&window)
        .expect("webview");

    apply_webview_zoom(&webview, s.zoom_factor);

    {
        let ads = ads.clone();
        unsafe {
            if let Ok(core) = webview.controller().CoreWebView2() {
                adblock::install_adblock_filter(&core, ads);
            }
        }
    }

    let mut exiting = false;
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        let _keep_tray_alive = &tray_state;

        match event {
            Event::UserEvent(UserEvent::Lyrics(json)) => {
                if let Ok(lit) = serde_json::to_string(&json) {
                    let _ = webview
                        .evaluate_script(&format!("window.__scLyrics&&window.__scLyrics({})", lit));
                }
            }
            Event::UserEvent(UserEvent::PipeMessage(msg)) => {
                show_and_focus(&window);
                if settings::is_valid_soundcloud_url(&msg) {
                    let _ = webview.load_url(&msg);
                }
            }
            Event::UserEvent(UserEvent::Tray(ev)) => {
                if let TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } = ev
                {
                    show_and_focus(&window);
                }
            }
            Event::UserEvent(UserEvent::Menu(ev)) => {
                if let Some(ts) = &tray_state {
                    if ev.id == ts.show_id {
                        show_and_focus(&window);
                    } else if ev.id == ts.lyrics_id {
                        show_and_focus(&window);
                        let _ = webview
                            .evaluate_script("window.__scToggleLyrics&&window.__scToggleLyrics()");
                    } else if ev.id == ts.page_id {
                        show_and_focus(&window);
                        let _ = webview.evaluate_script(
                            "window.__scLyricsOpenPage&&window.__scLyricsOpenPage()",
                        );
                    } else if ev.id == ts.quit_id {
                        exiting = true;
                        stop_pipe.store(true, Ordering::Relaxed);
                        save_window_geometry(&window);
                        *flow = ControlFlow::Exit;
                    } else if let Some((_, tid)) = ts.theme_ids.iter().find(|(id, _)| id == &ev.id)
                    {
                        let mut cur_s = settings::AppSettings::load();
                        cur_s.theme_id = (*tid).to_string();
                        cur_s.save();
                        let t = themes::get_theme(&cur_s.theme_id);
                        window.set_theme(Some(if t.dark_title_bar {
                            TaoTheme::Dark
                        } else {
                            TaoTheme::Light
                        }));
                        apply_dwm_theme(&window, t);
                        let js = build_theme_css_script(t);
                        let _ = webview.evaluate_script(&js);
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                let close_to_tray = settings::AppSettings::load().close_to_tray;
                if !exiting && close_to_tray && tray_state.is_some() {
                    save_window_geometry(&window);
                    window.set_visible(false);
                } else {
                    stop_pipe.store(true, Ordering::Relaxed);
                    save_window_geometry(&window);
                    *flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icon_assets_are_valid() {
        assert_eq!(ICON_RGBA_64.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        assert!(ICON_RGBA_64.iter().any(|&b| b != 0));
        assert!(load_window_icon().is_some());

        let png = include_bytes!("../assets/aurawave.png");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

        let ico = include_bytes!("../assets/aurawave.ico");
        assert!(ico.len() > 22);
        assert_eq!(&ico[0..4], &[0, 0, 1, 0]); // ICO type 1
        let count = u16::from_le_bytes([ico[4], ico[5]]);
        assert_eq!(count, 6);
    }

    #[test]
    fn default_aurawave_theme_has_expected_palette() {
        let t = themes::get_theme("aurawave");
        assert_eq!(t.id, "aurawave");
        assert_eq!(t.name, "AuraWave Midnight");
        assert_eq!((t.accent_color.r, t.accent_color.g, t.accent_color.b), (168, 85, 247));
        assert_eq!((t.body_bg.r, t.body_bg.g, t.body_bg.b), (0x0E, 0x0E, 0x14));
    }
}
