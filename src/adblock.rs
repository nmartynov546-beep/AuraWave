use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2_2, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
};
use webview2_com::WebResourceRequestedEventHandler;
use windows::core::{Interface, BOOL, HSTRING, PCWSTR, PWSTR};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::SHCreateMemStream;

pub fn is_blocked_ad_request(raw_uri: &str) -> Option<bool> {
    let lower = raw_uri.to_ascii_lowercase();
    let after_scheme = match lower.split("://").nth(1) {
        Some(rest) => rest,
        None => return None,
    };

    let (authority, path_and_query) = match after_scheme.find('/') {
        Some(idx) => (&after_scheme[..idx], &after_scheme[idx..]),
        None => (after_scheme, ""),
    };
    let host = authority.split(':').next().unwrap_or("");

    const BLOCKED_HOSTS: &[&str] = &[
        "ads.soundcloud.com",
        "promoted.soundcloud.com",
        "eventlogger.soundcloud.com",
        "adswizz.com",
        "tritondigital.com",
        "2mdn.net",
        "doubleclick.net",
        "googlesyndication.com",
        "googleadservices.com",
        "googletagservices.com",
        "googletagmanager.com",
        "google-analytics.com",
        "imasdk.googleapis.com",
        "pagead2.googlesyndication.com",
        "adservice.google.com",
        "adsrvr.org",
        "adnxs.com",
        "adform.net",
        "adroll.com",
        "advertising.com",
        "appnexus.com",
        "rubiconproject.com",
        "pubmatic.com",
        "openx.net",
        "criteo.com",
        "criteo.net",
        "taboola.com",
        "outbrain.com",
        "scorecardresearch.com",
        "comscore.com",
        "quantserve.com",
        "quantcount.com",
        "moatads.com",
        "doubleverify.com",
        "adsafeprotected.com",
        "amazon-adsystem.com",
        "branch.io",
        "app.link",
        "krxd.net",
        "demdex.net",
        "omtrdc.net",
        "rlcdn.com",
        "casalemedia.com",
        "33across.com",
        "indexww.com",
        "sharethrough.com",
        "smartadserver.com",
        "yieldmo.com",
        "spotxchange.com",
        "spotx.tv",
        "teads.tv",
        "bidswitch.net",
        "flashtalking.com",
        "innovid.com",
        "mathtag.com",
        "serving-sys.com",
        "sizmek.com",
        "bluekai.com",
        "exelator.com",
        "eyeota.net",
        "tapad.com",
        "an.facebook.com",
        "pixel.facebook.com",
        "ads-twitter.com",
        "analytics.twitter.com",
    ];

    if host_matches_any(host, BLOCKED_HOSTS) {
        let is_sc = host.ends_with("soundcloud.com");
        return Some(is_sc);
    }

    const SC_API_HOSTS: &[&str] = &[
        "api-v2.soundcloud.com",
        "api.soundcloud.com",
        "api-mobi.soundcloud.com",
    ];

    if host_matches_any(host, SC_API_HOSTS) {
        let path_only = path_and_query.split('?').next().unwrap_or("");
        if path_only.contains("/ads")
            || path_only.contains("/audio-ads")
            || path_only.contains("/video-ads")
            || path_only.contains("/visual-ads")
            || path_only.contains("/ad-delivery")
            || path_only.contains("/ad_delivery")
            || path_only.contains("/vast")
            || path_only.contains("/promoted")
            || path_only.contains("/promotions")
            || path_only.contains("/sponsorship")
            || path_only.contains("/commercials")
            || path_only.contains("/monetization")
            || path_only.contains("/me/upsell")
            || path_only.contains("/upsell")
            || path_and_query.contains("ad_urn=")
            || path_and_query.contains("ad_id=")
            || path_and_query.contains("promoted_by=")
        {
            return Some(true);
        }
    }

    None
}

fn host_matches_any(host: &str, roots: &[&str]) -> bool {
    for &root in roots {
        if host == root
            || (host.len() > root.len()
                && host.ends_with(root)
                && host.as_bytes()[host.len() - root.len() - 1] == b'.')
        {
            return true;
        }
    }
    false
}

pub fn install_adblock_filter(core: &ICoreWebView2, enabled: Arc<AtomicBool>) {
    unsafe {
        let _ = core.AddWebResourceRequestedFilter(
            windows::core::w!("*"),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        );

        let Ok(core2) = core.cast::<ICoreWebView2_2>() else {
            return;
        };
        let Ok(env) = core2.Environment() else {
            return;
        };

        let handler = WebResourceRequestedEventHandler::create(Box::new(move |_sender, args| {
            if !enabled.load(Ordering::Relaxed) {
                return Ok(());
            }
            let Some(args) = args else {
                return Ok(());
            };
            let Ok(request) = args.Request() else {
                return Ok(());
            };

            let mut uri_ptr = PWSTR::null();
            if request.Uri(&mut uri_ptr).is_err() || uri_ptr.is_null() {
                return Ok(());
            }
            let uri_str = pwstr_to_string(uri_ptr);
            CoTaskMemFree(Some(uri_ptr.as_ptr() as *const _));

            let Some(is_sc_api) = is_blocked_ad_request(&uri_str) else {
                return Ok(());
            };

            if is_sc_api {
                let empty_json = b"{\"collection\":[],\"ads\":[],\"next_href\":null}";
                if let Some(stream) = SHCreateMemStream(Some(empty_json)) {
                    let headers = HSTRING::from(
                        "Content-Type: application/json\r\nAccess-Control-Allow-Origin: https://soundcloud.com",
                    );
                    if let Ok(resp) = env.CreateWebResourceResponse(
                        &stream,
                        200,
                        windows::core::w!("OK"),
                        PCWSTR(headers.as_ptr()),
                    ) {
                        let _ = args.SetResponse(&resp);
                    }
                }
            } else if let Ok(resp) = env.CreateWebResourceResponse(
                None,
                403,
                windows::core::w!("Blocked"),
                windows::core::w!(""),
            ) {
                let _ = args.SetResponse(&resp);
            }

            let _ = BOOL::from(true);
            Ok(())
        }));

        let mut token = 0i64;
        let _ = core.add_WebResourceRequested(&handler, &mut token);
    }
}

unsafe fn pwstr_to_string(ptr: PWSTR) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.0.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr.0, len))
}

/// Returns JS that hides promoted/sponsored DOM cards and guards against audio ads in `.playControls`.
pub fn build_dom_adblock_script(block_ads: bool, hide_upsells: bool) -> String {
    let ad_css = if block_ads {
        ".promotedTrack,.visualAd,.audioAd,.displayAd,.adContainer,.sidebarAd,.leaderboardAd,.mrecAd,[data-testid*='promoted' i],[class*='VisualAd' i],[class*='AudioAd' i],.soundList__item:has(.sc-promoted-icon),.searchList__item:has(.sc-promoted-icon),.stream__item:has(.sc-promoted-icon){display:none!important;}"
    } else {
        ""
    };
    let upsell_css = if hide_upsells {
        ".header__upsellWrapper,.l-upsell,.upsellBanner,.sidebarModule__upsell,.g-upsell-banner,[class*='Upsell'],.tryGoPlus,.nextProBanner{display:none!important;}"
    } else {
        ""
    };
    format!(
        r#"(function(){{
if(window.top!==window)return;
const css={css:?};
const blockAds={block_ads};
const applyCss=()=>{{
  let st=document.getElementById('aurawave-adblock-css');
  if(!st){{st=document.createElement('style');st.id='aurawave-adblock-css';(document.head||document.documentElement).appendChild(st);}}
  st.textContent=css;
}};
if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',applyCss);else applyCss();
if(blockAds && !window.__auraWaveAdGuard){{
  window.__auraWaveAdGuard=true;
  let mutedForAd=false,lastSkipTs=0;
  const hidePromoted=()=>{{
    document.querySelectorAll('.sc-promoted-icon,[aria-label*="Promoted" i]').forEach(el=>{{
      const card=el.closest('.soundList__item,.searchList__item,.stream__item,.userStreamItem,.promotedTrack');
      if(card)card.style.setProperty('display','none','important');
    }});
  }};
  const obs=new MutationObserver(()=>hidePromoted());
  const startObs=()=>{{
    if(!document.body)return setTimeout(startObs,200);
    hidePromoted();
    obs.observe(document.body,{{childList:true,subtree:true}});
  }};
  startObs();
  setInterval(()=>{{
    const badge=document.querySelector('.playbackSoundBadge');
    const titleEl=document.querySelector('.playbackSoundBadge__titleLink');
    const title=(titleEl?(titleEl.getAttribute('title')||titleEl.textContent||''):'').trim();
    const isAd=!!badge && (
      !!badge.querySelector('.playbackSoundBadge__ad,.sc-promoted-icon,[class*="AdBadge" i]') ||
      /^advertisement$/i.test(title) ||
      /^sponsored$/i.test(title)
    );
    if(isAd){{
      document.querySelectorAll('audio,video').forEach(m=>{{if(!m.muted){{m.muted=true;mutedForAd=true;}}}});
      const now=Date.now();
      if(now-lastSkipTs>1500){{
        const nextBtn=document.querySelector('.skipControl__next');
        if(nextBtn && !nextBtn.disabled){{lastSkipTs=now;nextBtn.click();}}
      }}
    }}else if(mutedForAd){{
      document.querySelectorAll('audio,video').forEach(m=>{{m.muted=false;}});
      mutedForAd=false;
    }}
  }},500);
}}
}})();"#,
        css = format!("{}{}", ad_css, upsell_css),
        block_ads = block_ads
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_soundcloud_and_third_party_ads_but_keeps_media_and_auth() {
        assert_eq!(
            is_blocked_ad_request("https://ads.soundcloud.com/v1/vast"),
            Some(true)
        );
        assert_eq!(
            is_blocked_ad_request("https://api-v2.soundcloud.com/audio-ads?client_id=123"),
            Some(true)
        );
        assert_eq!(
            is_blocked_ad_request("https://api-v2.soundcloud.com/me/upsell"),
            Some(true)
        );
        assert_eq!(
            is_blocked_ad_request("https://deliveryengine.adswizz.com/www/delivery/afr.php"),
            Some(false)
        );
        assert_eq!(
            is_blocked_ad_request("https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js"),
            Some(false)
        );

        assert_eq!(
            is_blocked_ad_request("https://api-v2.soundcloud.com/tracks/123456"),
            None
        );
        assert_eq!(
            is_blocked_ad_request("https://i1.sndcdn.com/avatars-000123-large.jpg"),
            None
        );
        assert_eq!(
            is_blocked_ad_request("https://wave.sndcdn.com/abcdef.json"),
            None
        );
        assert_eq!(
            is_blocked_ad_request("https://soundcloud.com/signin"),
            None
        );
    }
}
