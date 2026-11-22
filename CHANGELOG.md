# Changelog

## Unreleased
- Live tab lanes sit side by side in a grid and no longer overlap. Narrow windows stack them, and each lane is clipped to its own box. Proven by `narrow_width_stacks`, `lane_paint_clipped`, `lane_scroll_ids_unique`, `resize_clamps_no_overlap`, `wrap_3_4_5_lanes_no_overlap`, `page_rects_match_lane_rects_for_1_and_8_lanes`, `huge_persisted_height_stays_finite` (cargo test --bin breakguard: 906 passed, 0 failed). Checked on screen with 2 lanes at 1600 and 2400 px (`QA-Shots/team/after-1600.png`, `after-2400.png`); narrow 1200 px stacks. Draw counts for 8 and 30 lanes not measured.
- Live feed rows read in plain English: "Reads the file X", "Runs a command: <description>", "Searches the web for \"q\"". Unknown tools show "Uses the <name> tool". Command rows show the description first. Proven by `plain_tool_sentences`, `plain_command_prefers_description`, `plain_english_for_every_tool_and_result` (1 passed each). Edge-case inputs not yet tested.
- Status bar "N Claude running" now counts the same running sessions as the Live tab lanes (was a transcript-status count: 6 running vs 5 lanes). Cheap count (`agents::count_live`, no desktop-title walk): page 1 CPU 484-562 ms per 10 s with real Claude folders.
- Screen-off ends at the exact millisecond: the engine timer and the blackout overlay no longer use whole unix seconds (a 7 s blackout ended at 6.49 s; a 1 s off could end almost at once). Measured: 1 s -> 1.21 s, 3 s -> 3.21 s, 7 s -> 7.17 s process time (start-up included). Previous exe kept as `breakguard.exe.bak-20261122-pre-exact-ms` / `-pre-count-live`.
- Background CPU with real Claude folders cut from ~6 s to under 1 s per 10 s (page 1 766 ms, Live page 984 ms): session digests memoised, unchanged transcripts (same mtime+size) are not reopened, desktop session JSON shared.
- Live tab tool chips: MCP tools `mcp__server__tool` show the tool part, max 16 characters, cut on character boundaries.
- Installed 2026-11-22 (previous exe kept as `breakguard.exe.bak-20261122-pre-live-tail`).
- Screen-off length is now exact and free: hours / minutes / seconds boxes, a type box (`1h30m`, `90` = minutes, `1:30:00`, `45s`, `2.5h`) and 12 quick chips (30 s to 4 h) on Dashboard, Breaks and Monitors. Any length from 1 s to 24 h (was minutes only, 1-600, reset to 15 every start). Saved as `off_secs` in settings.ini. Monitors page: per-screen and "Turn off all screens for <length>" buttons. Screens come back at the exact second; the hardware failsafe gets the length rounded up to a minute. `off --minutes` CLI accepts 1-1440.
- Live log tab: only sessions open in Claude (desktop app + terminal); SDK helper processes are skipped. Names follow the Claude sidebar on every scan (matched by `hostSessionId`, renames show within a second; old title records in a transcript no longer override). An ended session leaves the tab 3 s after its process exits (was 10 min). Desktop title files are cached by modified time instead of re-parsed every second.
- Breaks page: "Break status" card with 120 px countdown ring (MM:SS, repaints every 500 ms while timer active) and one monitor card per screen (max 3): "Ready" when working, "Screen off" during a break. Shows "No screens detected" when none found.
- Settings: "Light theme" toggle in Appearance. Saved as `light_theme=0/1`; old settings files load dark; bad value keeps dark. Applied at startup and on reset.
- Theme: light palette (`LIGHT_PAL`) with `set_light` / `pal()`; light status colours (`LIGHT_OK/WARN/DANGER`) darkened to pass WCAG AA 4.5:1 on all surfaces. Dark palette unchanged. Known gap: pages still using the dark-only `pub const` colours ignore light mode until migrated to `pal()`.

## 0.4.0
- Sessions page: live list of Claude Code sessions (name, environment, status, activity, plan progress) from a background thread polling every second; summary header; search and status chips; pinned sessions.
- Log page Sessions tab, status-change toasts, tooltips on every control.
- Appearance: 6 accent colours, UI scale 90-150 percent, drawn sidebar icons with live mini timer.
- Autostart toggle, sound options with distinct tones, global hotkeys (Ctrl+Alt+letter) with conflict check, tray icon with menu, start minimized, close to tray.
- Nine pages, Ctrl+1..9. Clippy clean with -D warnings.

## 0.3.0
- Rebuilt engine: break timer with warn/skip/snooze, daily/once schedules, night window, timed sleep, tolerant atomic settings, rotating log.
- Safer monitor layer with full mock mode (`BREAKGUARD_DDC_MOCK`, `BREAKGUARD_DATA_DIR`).
- Hardware off arms a failsafe helper first; automatic fallback to software blackout.
- New egui theme, shared widgets and pages.
- CLI: `blackout`, `helper-on`, `selftest`, `--version`.
- Embedded icon, manifest (PerMonitorV2 DPI, UTF-8) and version info.
- Release profile: opt-level 3, thin LTO. Reproducible `scripts\build.ps1`.

## 0.2.0
- Earlier release (source lost; behavior recovered from the live exe).

## Unreleased - 2026-11-22
- Screen-off: new 'Start in' (now to 24 h from now) next to the exact length, on Dashboard and Monitors; waiting offs listed with Cancel. Schedule page: Start in / For quick chips and a live 'Turns off at HH:MM (in X) for Y' preview. Engine: off_in_secs, cancel_delayed, view.delayed_off (test off_in_starts_later_for_chosen_length_and_cancels). check-all -SkipInstalled ALL OK; installed exe sha256 46EAAA53...
