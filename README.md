# AuraWave

Standalone Windows desktop client for SoundCloud built in Rust (`tao` + `wry` WebView2), featuring a custom **AuraWave Midnight** theme, synced lyrics overlay, ad & upsell suppression, and system tray integration.

## Features

- **AuraWave Midnight Theme**: Obsidian shell (`#0E0E14`) with electric violet (`#A855F7`) accents, live theme switching via tray menu (including Dark Obsidian, Midnight OLED, Light Classic, Synthwave Purple, Deep Ocean), and DWM dark title bar styling.
- **Synced Lyrics Overlay (`♪ Lyrics` / `Alt+L`)**:
  - Reads track title, artist, duration, and playback position from SoundCloud (with `navigator.mediaSession.metadata` fallback).
  - Pluggable lyrics providers: **LRCLIB** (default, no key required) and **Yandex Music** (fallback or primary via `YANDEX_MUSIC_TOKEN` env var or inline `⚙` settings drawer).
  - Smooth auto-scrolling synced lines, past lines dimmed, current line highlighted, plain-text fallback, and "No lyrics found" state.
- **Ad & Upsell Suppression**: WebView2 network filtering (`adblock.rs`) + DOM promoted-card/upsell hiding and safe audio-ad mute/skip guard.
- **Standard Audio Quality Preference**: Prioritizes highest-efficiency non-paywalled standard streams (e.g., Opus over MP3) without spoofing or bypassing SoundCloud Go+ / DRM entitlements (256 kbps AAC / Go+ tracks remain server-side gated).
- **Single-Instance & System Tray**: Isolated mutex/pipe and `%LOCALAPPDATA%\AuraWave` storage, window geometry persistence, and close-to-tray support.

## Build & Test

Ensure Rust (`cargo`) is installed (e.g., `$env:USERPROFILE\.cargo\bin\cargo.exe`):

```powershell
# Run unit tests
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test

# Debug build
& "$env:USERPROFILE\.cargo\bin\cargo.exe" build

# Release build (outputs target\release\AuraWave.exe)
& "$env:USERPROFILE\.cargo\bin\cargo.exe" build --release
```
