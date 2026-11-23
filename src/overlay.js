(function () {
  if (window.top !== window || window.__auraWaveLyricsInit) return;
  window.__auraWaveLyricsInit = true;
  let lines = [], plain = '', source = '', state = 'idle', reqId = 0, cur = -1, key = '', open = false, pageOpen = false, showCfg = false;
  let basePos = 0, baseTs = 0, playing = false, lastDur = 0;
  let lastTrack = null, curTrackMeta = { title: '', artist: '', art: '' };

  const waveSvg = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true" style="flex-shrink:0"><defs><linearGradient id="awg" x1="0" y1="0" x2="24" y2="24" gradientUnits="userSpaceOnUse"><stop offset="0%" stop-color="#A855F7"/><stop offset="55%" stop-color="#EC4899"/><stop offset="100%" stop-color="#22D3EE"/></linearGradient></defs><rect x="2" y="9" width="3" height="7" rx="1.5" fill="url(#awg)"/><rect x="7" y="5" width="3" height="14" rx="1.5" fill="url(#awg)"/><path d="M12 19V9a6 6 0 0 1 10-4.4c-2.6-.2-4.7 1.4-5 4.1V19a1.5 1.5 0 0 1-3 0h-2z" fill="url(#awg)"/><rect x="19" y="12" width="3" height="7" rx="1.5" fill="url(#awg)"/></svg>';
  const bookSvg = '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" style="flex-shrink:0"><path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"/><path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"/></svg>';

  const css = document.createElement('style');
  css.textContent = `
  #scl-btn,#scl-page-btn,#scl-panel,#scl-panel *,#scl-page,#scl-page *{box-sizing:border-box}
  #scl-btn{position:fixed;right:16px;bottom:64px;z-index:2147483645;background:var(--sc-accent,#A855F7);color:#fff;border:0;border-radius:20px;padding:8px 14px;height:36px;font:600 13px/1.2 sans-serif;cursor:pointer;opacity:.95;box-shadow:0 3px 14px #0007;display:inline-flex;align-items:center;justify-content:center;gap:6px;white-space:nowrap;max-width:calc(100vw - 32px);transition:background-color .2s,transform .15s,opacity .15s}
  #scl-btn:hover,#scl-page-btn:hover{opacity:1;transform:translateY(-1px)}
  #scl-page-btn{position:fixed;right:116px;bottom:64px;z-index:2147483645;background:var(--sc-toolbar,#13131C);color:var(--sc-text,#F0F0F7);border:1px solid var(--sc-border,#28283A);border-radius:20px;padding:8px 14px;height:36px;font:600 13px/1.2 sans-serif;cursor:pointer;opacity:.95;box-shadow:0 3px 14px #0007;display:inline-flex;align-items:center;justify-content:center;gap:6px;white-space:nowrap;max-width:calc(100vw - 140px);transition:border-color .2s,background-color .2s,transform .15s,opacity .15s}
  #scl-page-btn:hover{border-color:var(--sc-accent,#A855F7)}
  #scl-page-btn svg{color:var(--sc-accent,#A855F7)}
  #scl-panel{position:fixed;right:16px;bottom:108px;width:min(390px,calc(100vw - 32px));height:min(560px,calc(100vh - 124px));max-height:calc(100vh - 124px);z-index:2147483645;background:var(--sc-bg,rgba(14,14,20,.96));backdrop-filter:blur(16px);border:1px solid var(--sc-border,#28283A);border-radius:14px;color:var(--sc-text,#F0F0F7);font-family:sans-serif;display:none;flex-direction:column;box-shadow:0 10px 34px #000b;overflow:hidden}
  #scl-panel.on{display:flex}
  #scl-head{display:flex;align-items:center;gap:6px;padding:10px 12px;font-size:12px;color:var(--sc-muted,#9898B0);border-bottom:1px solid var(--sc-border,#28283A);background:var(--sc-toolbar,#13131C)}
  #scl-brand{font-size:11px;font-weight:700;letter-spacing:.03em;color:var(--sc-accent,#A855F7);flex-shrink:0}
  #scl-title{flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-weight:600;color:var(--sc-text,#F0F0F7)}
  .scl-src{font-size:11px;padding:2px 7px;border-radius:10px;background:var(--sc-hover,#1F1F2E);color:var(--sc-accent,#A855F7);display:none;flex-shrink:0}
  .scl-src.on{display:inline-block}
  .scl-hbtn{background:transparent;border:1px solid var(--sc-border,#28283A);color:var(--sc-text,#F0F0F7);border-radius:8px;padding:3px 8px;font-size:11px;line-height:1.3;cursor:pointer;flex-shrink:0;white-space:nowrap}
  .scl-hbtn:hover{background:var(--sc-hover,#1F1F2E);border-color:var(--sc-accent,#A855F7)}
  #scl-cfg{display:none;flex-direction:column;gap:8px;padding:12px 14px;background:var(--sc-toolbar,#13131C);border-bottom:1px solid var(--sc-border,#28283A);font-size:12px;color:var(--sc-text,#F0F0F7)}
  #scl-cfg.on{display:flex}
  #scl-cfg label{display:flex;flex-direction:column;gap:4px;color:var(--sc-muted,#9898B0)}
  #scl-cfg select,#scl-cfg input,#scl-page-search input{background:var(--sc-bg,#0E0E14);color:var(--sc-text,#F0F0F7);border:1px solid var(--sc-border,#28283A);border-radius:6px;padding:6px 8px;font-size:12px}
  .scl-cfg-row{display:flex;gap:8px;align-items:center;flex-wrap:wrap}
  .scl-cfg-row button,.scl-pbtn{background:var(--sc-accent,#A855F7);color:#fff;border:0;border-radius:8px;padding:6px 12px;font-size:12px;font-weight:600;cursor:pointer;white-space:nowrap}
  .scl-cfg-row button.sec,.scl-pbtn.sec{background:var(--sc-hover,#1F1F2E);color:var(--sc-text,#F0F0F7);border:1px solid var(--sc-border,#28283A)}
  .scl-pbtn.sec:hover{border-color:var(--sc-accent,#A855F7)}
  #scl-cfg-msg{font-size:11px;color:#9fd49f}
  #scl-body{position:relative;flex:1;overflow-y:auto;padding:40% 16px;scroll-behavior:smooth;scrollbar-width:none}
  .scl-l{font-size:21px;font-weight:700;line-height:1.35;margin:10px 0;color:var(--sc-text,#F0F0F7);opacity:.35;transition:all .35s;cursor:pointer;transform-origin:left}
  .scl-l.past{opacity:.2}
  .scl-l.cur{opacity:1;color:var(--sc-accent,#A855F7);transform:scale(1.06)}
  .scl-msg{text-align:center;color:var(--sc-muted,#9898B0);font-size:15px;padding-top:30%}
  .scl-plain{white-space:pre-wrap;font-size:15px;line-height:1.6;color:var(--sc-text,#F0F0F7)}
  #scl-page{position:fixed;inset:0;z-index:2147483647;display:none;flex-direction:column;color:var(--sc-text,#F0F0F7);font-family:sans-serif;background:radial-gradient(circle at 18% 12%,rgba(168,85,247,.18),transparent 45%),radial-gradient(circle at 85% 85%,rgba(34,211,238,.10),transparent 40%),var(--sc-bg,#0E0E14)}
  #scl-page.on{display:flex}
  #scl-page-top{display:flex;align-items:center;gap:10px;padding:12px 20px;border-bottom:1px solid var(--sc-border,#28283A);background:var(--sc-toolbar,#13131C);flex-wrap:wrap}
  #scl-page-badge{display:inline-flex;align-items:center;gap:7px;font-size:13px;font-weight:700;color:var(--sc-text,#F0F0F7);margin-right:auto}
  #scl-page-hero{display:flex;align-items:center;justify-content:space-between;gap:16px;padding:16px 24px;border-bottom:1px solid var(--sc-border,#28283A);background:rgba(0,0,0,.18);flex-wrap:wrap}
  #scl-page-track{display:flex;align-items:center;gap:14px;min-width:220px;flex:1}
  #scl-page-art{width:56px;height:56px;border-radius:10px;object-fit:cover;border:1px solid var(--sc-border,#28283A);display:none;flex-shrink:0}
  #scl-page-art.on{display:block}
  #scl-page-meta{min-width:0}
  #scl-page-track-title{font-size:18px;font-weight:700;color:var(--sc-text,#F0F0F7);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;max-width:46vw}
  #scl-page-track-sub{display:flex;align-items:center;gap:10px;font-size:13px;color:var(--sc-muted,#9898B0);margin-top:3px;flex-wrap:wrap}
  #scl-page-search{display:flex;align-items:center;gap:8px;flex-wrap:wrap}
  #scl-page-search input{width:155px}
  #scl-page-body{position:relative;flex:1;overflow-y:auto;padding:18vh 24px 36vh;scroll-behavior:smooth;scrollbar-width:thin}
  #scl-page-body .scl-l{font-size:clamp(20px,2.5vw,32px);text-align:center;transform-origin:center;max-width:860px;margin:16px auto}
  #scl-page-body .scl-plain{max-width:740px;margin:0 auto;font-size:17px;line-height:1.75;text-align:center}
  #scl-page-body .scl-msg{padding-top:14vh;font-size:17px}`;

  const btn = document.createElement('button');
  btn.id = 'scl-btn';
  btn.innerHTML = waveSvg + '<span>Lyrics</span>';
  btn.title = 'AuraWave Mini Lyrics (Alt+L)';

  const pageBtn = document.createElement('button');
  pageBtn.id = 'scl-page-btn';
  pageBtn.innerHTML = bookSvg + '<span>Страница текста</span>';
  pageBtn.title = 'Открыть страницу текста песни';
  pageBtn.setAttribute('aria-label', 'Открыть страницу текста песни');

  const cfgHtml = `
    <label>Primary Lyrics Provider
      <select id="scl-prov">
        <option value="lrclib">LRCLIB (Default, Yandex fallback)</option>
        <option value="yandex">Yandex Music (LRCLIB fallback)</option>
      </select>
    </label>
    <label>Yandex Music OAuth Token (<span id="scl-tok-state">not set</span>)
      <input id="scl-tok" type="password" autocomplete="off" placeholder="Paste OAuth token (leave blank to keep)" />
    </label>
    <label style="flex-direction:row;align-items:center;gap:6px;cursor:pointer">
      <input id="scl-hq" type="checkbox" checked style="width:auto;margin:0" />
      <span>Prefer best standard audio (Opus / SQ)</span>
    </label>
    <div class="scl-cfg-row">
      <button id="scl-save" type="button">Save &amp; Refetch</button>
      <button id="scl-clear" class="sec" type="button">Clear Token</button>
      <span id="scl-cfg-msg"></span>
    </div>`;

  const panel = document.createElement('div');
  panel.id = 'scl-panel';
  panel.innerHTML = `
    <div id="scl-head">
      ${waveSvg}
      <span id="scl-brand">AuraWave</span>
      <span id="scl-title">Lyrics</span>
      <span id="scl-src" class="scl-src"></span>
      <button id="scl-expand-btn" class="scl-hbtn" title="Открыть страницу текста песни">Страница текста</button>
      <button id="scl-cfg-btn" class="scl-hbtn" title="AuraWave lyrics &amp; audio settings">⚙</button>
    </div>
    <div id="scl-cfg">${cfgHtml}</div>
    <div id="scl-body"></div>`;

  const page = document.createElement('div');
  page.id = 'scl-page';
  page.innerHTML = `
    <div id="scl-page-top">
      <button id="scl-page-back" class="scl-pbtn sec" type="button">← Назад к SoundCloud</button>
      <span id="scl-page-badge">${waveSvg}<span>AuraWave • Страница текста</span></span>
      <span id="scl-page-src" class="scl-src"></span>
      <button id="scl-page-genius" class="scl-pbtn sec" type="button">Найти в браузере (Genius)</button>
      <button id="scl-page-cfg-btn" class="scl-pbtn sec" type="button">Настройки</button>
    </div>
    <div id="scl-page-hero">
      <div id="scl-page-track">
        <img id="scl-page-art" alt="" />
        <div id="scl-page-meta">
          <div id="scl-page-track-title">Нет активного трека</div>
          <div id="scl-page-track-sub">
            <span id="scl-page-track-artist">Запустите трек в SoundCloud</span>
            <span id="scl-page-time">0:00 / 0:00</span>
            <button id="scl-page-play" class="scl-pbtn sec" type="button">Play / Pause</button>
          </div>
        </div>
      </div>
      <form id="scl-page-search">
        <input id="scl-man-artist" type="text" placeholder="Исполнитель" />
        <input id="scl-man-title" type="text" placeholder="Название трека" />
        <button id="scl-man-go" class="scl-pbtn" type="submit">Вручную найти текст</button>
      </form>
    </div>
    <div id="scl-page-body"></div>`;

  function syncFabPosition() {
    const w = Math.ceil(btn.getBoundingClientRect().width || 92);
    pageBtn.style.right = (16 + w + 8) + 'px';
  }

  function mount() {
    if (!document.body) return setTimeout(mount, 100);
    document.head.appendChild(css);
    document.body.append(pageBtn, btn, panel, page);
    syncFabPosition();
    if (typeof ResizeObserver === 'function') new ResizeObserver(syncFabPosition).observe(btn);
  }
  mount();

  const q = s => panel.querySelector(s);
  const qp = s => page.querySelector(s);
  const body = () => q('#scl-body');
  const pageBody = () => qp('#scl-page-body');

  function fmtTime(sec) {
    const s = Math.max(0, Math.floor(sec || 0));
    const m = Math.floor(s / 60);
    const r = s % 60;
    return m + ':' + (r < 10 ? '0' : '') + r;
  }

  function togglePanel(force) {
    open = typeof force === 'boolean' ? force : !open;
    panel.classList.toggle('on', open);
    if (open) {
      renderAll();
      window.ipc.postMessage(JSON.stringify({ type: 'getSettings' }));
    }
  }

  function openPage() {
    pageOpen = true;
    open = false;
    panel.classList.remove('on');
    page.classList.add('on');
    updatePageHero();
    renderAll();
    window.ipc.postMessage(JSON.stringify({ type: 'getSettings' }));
  }

  function closePage() {
    pageOpen = false;
    page.classList.remove('on');
  }

  window.__scToggleLyrics = togglePanel;
  window.__scLyricsOpenPage = openPage;

  btn.onclick = () => togglePanel();
  pageBtn.onclick = () => (pageOpen ? closePage() : openPage());
  q('#scl-expand-btn').onclick = openPage;
  qp('#scl-page-back').onclick = closePage;

  qp('#scl-page-cfg-btn').onclick = () => {
    closePage();
    open = true;
    showCfg = true;
    panel.classList.add('on');
    q('#scl-cfg').classList.add('on');
    renderAll();
    window.ipc.postMessage(JSON.stringify({ type: 'getSettings' }));
  };

  qp('#scl-page-play').onclick = () => {
    const pb = document.querySelector('.playControls__play') || document.querySelector('.playControl');
    if (pb) pb.click();
  };

  qp('#scl-page-genius').onclick = () => {
    const query = ((curTrackMeta.artist || '') + ' ' + (curTrackMeta.title || '')).trim();
    const url = 'https://genius.com/search?q=' + encodeURIComponent(query || 'soundcloud');
    window.ipc.postMessage(JSON.stringify({ type: 'openExternal', url }));
  };

  qp('#scl-page-search').onsubmit = e => {
    e.preventDefault();
    const ma = qp('#scl-man-artist').value.trim();
    const mt = qp('#scl-man-title').value.trim();
    if (!ma && !mt) return;
    curTrackMeta = { title: mt || ma, artist: mt ? ma : '', art: curTrackMeta.art };
    updatePageHero();
    request({ artist: ma, title: mt || ma, dur: lastDur });
  };

  q('#scl-cfg-btn').onclick = () => {
    showCfg = !showCfg;
    q('#scl-cfg').classList.toggle('on', showCfg);
    if (showCfg) {
      q('#scl-cfg-msg').textContent = '';
      window.ipc.postMessage(JSON.stringify({ type: 'getSettings' }));
    }
  };

  q('#scl-save').onclick = () => {
    const prov = q('#scl-prov').value;
    const tok = q('#scl-tok').value;
    const hq = !!q('#scl-hq').checked;
    window.__scHqAudio = hq;
    q('#scl-tok').value = '';
    q('#scl-cfg-msg').textContent = 'Saved';
    window.ipc.postMessage(JSON.stringify({ type: 'saveSettings', lyrics_provider: prov, yandex_music_token: tok, prefer_hq_audio: hq, clear_token: false }));
    if (lastTrack) request(lastTrack);
  };

  q('#scl-clear').onclick = () => {
    q('#scl-tok').value = '';
    q('#scl-cfg-msg').textContent = 'Token cleared';
    window.ipc.postMessage(JSON.stringify({ type: 'saveSettings', lyrics_provider: q('#scl-prov').value, prefer_hq_audio: !!q('#scl-hq').checked, clear_token: true }));
    if (lastTrack) request(lastTrack);
  };

  addEventListener('keydown', e => {
    if (e.key === 'Escape' && pageOpen) {
      e.preventDefault();
      closePage();
      return;
    }
    if (e.altKey && e.code === 'KeyL') {
      e.preventDefault();
      if (pageOpen) closePage();
      else togglePanel();
    }
  }, true);

  const secs = t => {
    const p = (t || '').trim().split(':').map(Number);
    if (!p.length || p.some(isNaN)) return 0;
    return p.reduce((a, b) => a * 60 + b, 0);
  };

  function detectArtwork() {
    const img = document.querySelector('.playbackSoundBadge img');
    if (img && img.src) return img.src;
    const span = document.querySelector('.playbackSoundBadge .sc-artwork');
    if (span) {
      const bg = getComputedStyle(span).backgroundImage || span.style.backgroundImage || '';
      const m = bg.match(/url\(["']?(.*?)["']?\)/i);
      if (m && m[1]) return m[1];
    }
    if (navigator.mediaSession && navigator.mediaSession.metadata && navigator.mediaSession.metadata.artwork && navigator.mediaSession.metadata.artwork.length) {
      return navigator.mediaSession.metadata.artwork[0].src || '';
    }
    return '';
  }

  function info() {
    const t = document.querySelector('.playbackSoundBadge__titleLink');
    const a = document.querySelector('.playbackSoundBadge__lightLink');
    let title = t ? (t.getAttribute('title') || t.textContent || '').trim() : '';
    let artist = a ? (a.textContent || '').trim() : '';

    if (!title && navigator.mediaSession && navigator.mediaSession.metadata) {
      title = (navigator.mediaSession.metadata.title || '').trim();
      if (!artist) artist = (navigator.mediaSession.metadata.artist || '').trim();
    }
    if (!title) return null;

    if (!artist && title.includes(' - ')) {
      const idx = title.indexOf(' - ');
      artist = title.slice(0, idx).trim();
      title = title.slice(idx + 3).trim();
    }

    const pe = document.querySelector('.playbackTimeline__timePassed span[aria-hidden="true"]') || document.querySelector('.playbackTimeline__timePassed');
    const de = document.querySelector('.playbackTimeline__duration span[aria-hidden="true"]') || document.querySelector('.playbackTimeline__duration');
    const pb = document.querySelector('.playControls__play') || document.querySelector('.playControl');
    const isPlaying = !!pb && (pb.classList.contains('playing') || /pause/i.test(pb.getAttribute('aria-label') || pb.getAttribute('title') || ''));
    return {
      title,
      artist,
      art: detectArtwork(),
      pos: pe ? secs(pe.textContent) : 0,
      dur: de ? secs(de.textContent) : 0,
      playing: isPlaying
    };
  }

  function clean(s) {
    return (s || '').replace(/\s*[\(\[][^\)\]]*(official|lyrics?|audio|video|visuali[sz]er|hd|hq|remaster)[^\)\]]*[\)\]]/ig, '').trim();
  }

  function updatePageHero() {
    qp('#scl-page-track-title').textContent = curTrackMeta.title || 'Нет активного трека';
    qp('#scl-page-track-artist').textContent = curTrackMeta.artist || (curTrackMeta.title ? 'SoundCloud' : 'Запустите трек в SoundCloud');
    const artEl = qp('#scl-page-art');
    if (curTrackMeta.art) {
      artEl.src = curTrackMeta.art;
      artEl.classList.add('on');
    } else {
      artEl.removeAttribute('src');
      artEl.classList.remove('on');
    }
    if (!qp('#scl-man-artist').value && curTrackMeta.artist) qp('#scl-man-artist').placeholder = curTrackMeta.artist;
    if (!qp('#scl-man-title').value && curTrackMeta.title) qp('#scl-man-title').placeholder = curTrackMeta.title;
  }

  function renderInto(b) {
    if (!b) return;
    if (state === 'loading') b.innerHTML = '<div class="scl-msg">Loading lyrics…</div>';
    else if (state === 'idle') b.innerHTML = '<div class="scl-msg">Play a track to see lyrics</div>';
    else if (lines.length) {
      b.innerHTML = '';
      lines.forEach(l => {
        const d = document.createElement('div');
        d.className = 'scl-l';
        d.textContent = l.s || '♪';
        d.onclick = () => { basePos = l.t; baseTs = performance.now(); };
        b.appendChild(d);
      });
    } else if (plain.trim()) {
      b.innerHTML = '<div class="scl-plain"></div>';
      b.firstChild.textContent = plain;
    } else {
      b.innerHTML = '<div class="scl-msg">No lyrics found</div>';
    }
  }

  function renderAll() {
    cur = -1;
    [q('#scl-src'), qp('#scl-page-src')].forEach(el => {
      if (!el) return;
      el.textContent = source || '';
      el.classList.toggle('on', !!source);
    });
    if (open) renderInto(body());
    if (pageOpen) renderInto(pageBody());
  }

  window.__scLyrics = function (json) {
    let r;
    try { r = JSON.parse(json); } catch (e) { return; }
    if (r.type === 'settings') {
      q('#scl-prov').value = r.lyrics_provider === 'yandex' ? 'yandex' : 'lrclib';
      q('#scl-tok-state').textContent = r.yandex_token_from_env ? 'set via env' : (r.has_yandex_token ? 'saved' : 'not set');
      if (typeof r.prefer_hq_audio === 'boolean') {
        q('#scl-hq').checked = r.prefer_hq_audio;
        window.__scHqAudio = r.prefer_hq_audio;
      }
      return;
    }
    if (r.id !== reqId) return;
    lines = r.synced || [];
    plain = r.plain || '';
    source = r.source || '';
    state = 'done';
    renderAll();
  };

  function request(i) {
    lastTrack = i;
    reqId++;
    state = 'loading';
    lines = [];
    plain = '';
    source = '';
    renderAll();
    window.ipc.postMessage(JSON.stringify({
      type: 'lyrics',
      id: reqId,
      artist: clean(i.artist),
      title: clean(i.title),
      dur: i.dur || 0
    }));
  }

  function syncContainer(b, idx) {
    if (!b) return;
    const els = b.children;
    for (let i = 0; i < els.length; i++) {
      els[i].className = 'scl-l' + (i < idx ? ' past' : i === idx ? ' cur' : '');
    }
    if (els[idx]) {
      const e = els[idx];
      b.scrollTo({ top: e.offsetTop - b.clientHeight / 2 + e.clientHeight / 2, behavior: 'smooth' });
    }
  }

  function update(idx) {
    if (idx === cur) return;
    cur = idx;
    if (open) syncContainer(body(), idx);
    if (pageOpen) syncContainer(pageBody(), idx);
  }

  setInterval(() => {
    const i = info();
    if (!i) return;
    lastDur = i.dur || 0;
    const k = i.artist + '|' + i.title;
    if (k !== key) {
      key = k;
      curTrackMeta = { title: i.title, artist: i.artist, art: i.art };
      q('#scl-title').textContent = i.artist ? (i.artist + ' — ' + i.title) : i.title;
      qp('#scl-man-artist').value = '';
      qp('#scl-man-title').value = '';
      updatePageHero();
      request(i);
    } else if (i.art && i.art !== curTrackMeta.art) {
      curTrackMeta.art = i.art;
      if (pageOpen) updatePageHero();
    }
    const est = basePos + (i.playing ? (performance.now() - baseTs) / 1000 : 0);
    if (Math.abs(est - i.pos) > 1.2 || i.playing !== playing) {
      basePos = i.pos;
      baseTs = performance.now();
    }
    playing = i.playing;
    if (pageOpen) {
      qp('#scl-page-time').textContent = fmtTime(i.pos) + ' / ' + fmtTime(i.dur);
      qp('#scl-page-play').textContent = playing ? 'Pause' : 'Play';
    }
  }, 500);

  function tick() {
    if ((open || pageOpen) && lines.length) {
      const p = basePos + (playing ? (performance.now() - baseTs) / 1000 : 0) + 0.2;
      let idx = -1;
      for (let j = 0; j < lines.length; j++) {
        if (lines[j].t <= p) idx = j;
        else break;
      }
      update(idx);
    }
    requestAnimationFrame(tick);
  }
  tick();
})();
