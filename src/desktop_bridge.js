(function () {
  if (window.top !== window || window.__auraWaveBridgeInit) return;
  window.__auraWaveBridgeInit = true;

  const STYLE_ID = 'aurawave-mini-style';
  function ensureMiniStyle() {
    if (document.getElementById(STYLE_ID)) return;
    const st = document.createElement('style');
    st.id = STYLE_ID;
    st.textContent = `
      html.sc-desktop-mini body {
        overflow: hidden !important;
        background: var(--sc-bg, #0E0E14) !important;
      }
      html.sc-desktop-mini .header,
      html.sc-desktop-mini #content,
      html.sc-desktop-mini .announcements,
      html.sc-desktop-mini #scl-panel,
      html.sc-desktop-mini #scl-page {
        visibility: hidden !important;
        pointer-events: none !important;
      }
      html.sc-desktop-mini .playControls {
        position: fixed !important;
        top: 0 !important;
        left: 0 !important;
        right: 0 !important;
        bottom: 0 !important;
        height: 100vh !important;
        width: 100vw !important;
        z-index: 999999 !important;
        background: var(--sc-toolbar, #13131C) !important;
        border-top: none !important;
        display: flex !important;
        align-items: center !important;
        justify-content: center !important;
        padding: 0 12px !important;
        box-sizing: border-box !important;
      }
      html.sc-desktop-mini .playControls__inner {
        width: 100% !important;
        max-width: 100% !important;
      }
    `;
    (document.head || document.documentElement).appendChild(st);
  }

  function getTrackState() {
    const t = document.querySelector('.playbackSoundBadge__titleLink');
    const a = document.querySelector('.playbackSoundBadge__lightLink');
    let title = t ? (t.getAttribute('title') || t.textContent || '').trim() : '';
    let artist = a ? (a.textContent || '').trim() : '';
    if (!title && navigator.mediaSession && navigator.mediaSession.metadata) {
      title = (navigator.mediaSession.metadata.title || '').trim();
      if (!artist) artist = (navigator.mediaSession.metadata.artist || '').trim();
    }
    const href = t && t.getAttribute('href') ? t.getAttribute('href') : '';
    let trackUrl = '';
    if (href) {
      try { trackUrl = new URL(href, location.origin).href; } catch (_e) { trackUrl = ''; }
    }
    const pb = document.querySelector('.playControls__play') || document.querySelector('.playControl');
    const playing = !!pb && (pb.classList.contains('playing') || /pause/i.test(pb.getAttribute('aria-label') || pb.getAttribute('title') || ''));
    return {
      type: 'playerState',
      playing,
      title,
      artist,
      trackUrl: trackUrl || location.href,
      pageUrl: location.href
    };
  }

  let lastSig = '';
  function pushState(force) {
    const st = getTrackState();
    const sig = [st.playing, st.title, st.artist, st.trackUrl, st.pageUrl].join('|');
    if (force || sig !== lastSig) {
      lastSig = sig;
      window.ipc && window.ipc.postMessage(JSON.stringify(st));
    }
  }

  window.__scDesktop = {
    togglePlay() {
      const b = document.querySelector('.playControl') || document.querySelector('.playControls__play');
      if (b) b.click();
      setTimeout(() => pushState(true), 120);
    },
    next() {
      const b = document.querySelector('.skipControl__next');
      if (b) b.click();
      setTimeout(() => pushState(true), 180);
    },
    prev() {
      const b = document.querySelector('.skipControl__previous');
      if (b) b.click();
      setTimeout(() => pushState(true), 180);
    },
    focusSearch() {
      const inp = document.querySelector('.headerSearch__input, input[type="search"]');
      if (inp) { inp.focus(); inp.select(); }
    },
    setMiniMode(enabled) {
      ensureMiniStyle();
      document.documentElement.classList.toggle('sc-desktop-mini', !!enabled);
    }
  };

  window.addEventListener('mouseup', e => {
    if (e.button === 3) {
      e.preventDefault();
      window.ipc && window.ipc.postMessage(JSON.stringify({ type: 'nav', action: 'back' }));
    } else if (e.button === 4) {
      e.preventDefault();
      window.ipc && window.ipc.postMessage(JSON.stringify({ type: 'nav', action: 'forward' }));
    }
  }, true);

  window.addEventListener('keydown', e => {
    const mod = (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey;
    const k = (e.key || '').toLowerCase();
    if (mod && (k === 'k' || k === 'l')) {
      e.preventDefault();
      window.__scDesktop.focusSearch();
    } else if (mod && k === 'm') {
      e.preventDefault();
      window.ipc && window.ipc.postMessage(JSON.stringify({ type: 'shortcut', name: 'toggleMini' }));
    } else if (mod && k === 'h') {
      e.preventDefault();
      window.ipc && window.ipc.postMessage(JSON.stringify({ type: 'shortcut', name: 'toggleToolbar' }));
    } else if (mod && k === 'y') {
      e.preventDefault();
      window.ipc && window.ipc.postMessage(JSON.stringify({ type: 'shortcut', name: 'toggleLyrics' }));
    } else if (mod && (e.key === '=' || e.key === '+')) {
      e.preventDefault();
      window.ipc && window.ipc.postMessage(JSON.stringify({ type: 'shortcut', name: 'zoomIn' }));
    } else if (mod && e.key === '-') {
      e.preventDefault();
      window.ipc && window.ipc.postMessage(JSON.stringify({ type: 'shortcut', name: 'zoomOut' }));
    } else if (mod && e.key === '0') {
      e.preventDefault();
      window.ipc && window.ipc.postMessage(JSON.stringify({ type: 'shortcut', name: 'zoomReset' }));
    } else if (e.key === 'F11') {
      e.preventDefault();
      window.ipc && window.ipc.postMessage(JSON.stringify({ type: 'shortcut', name: 'toggleFullscreen' }));
    }
  }, true);

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => { ensureMiniStyle(); pushState(true); });
  } else {
    ensureMiniStyle();
    pushState(true);
  }
  setInterval(() => pushState(false), 450);
})();
