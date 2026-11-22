# BreakGuard design system — "calm night-cockpit"

Deep blue-black surfaces, one electric-teal accent for primary actions, amber for warnings/breaks, soft red only for destructive/off actions. Egui default fonts only (proportional + monospace for countdowns). Icons are drawn with Painter shapes (no emoji). Source: `src/ui/theme.rs`, `src/ui/widgets.rs`.

## Colour tokens

| Token | RGB | Use |
|---|---|---|
| BG | 9,12,22 | window background |
| SURFACE | 16,22,36 | cards, panels |
| SURFACE_HI | 26,35,55 | inputs, hover surface |
| SURFACE_LO | 12,17,29 | contrast reference surface (`tokens_doc`) |
| BORDER | 38,49,72 | decorative card edges only |
| BORDER_STRONG | 100,116,139 | control outlines (>=3:1) |
| TEXT | 232,238,247 | body text |
| TEXT_DIM | 148,163,184 | secondary text |
| ACCENT / ACCENT_HI / ACCENT_LO | 45,212,191 / 94,234,212 / 20,150,135 | primary, hover, pressed, focus ring |
| OK | 74,222,128 | healthy |
| WARN | 251,191,36 | break / warning |
| DANGER / DANGER_HI | 248,113,113 / 252,165,165 | destructive, screen off |
| ON_FILL | 6,20,24 | text on bright fills |

### Light palette (`theme::LIGHT_PAL`, `LIGHT_OK/WARN/DANGER`)
| Token | RGB | Use |
|---|---|---|
| bg | 246,248,251 | window background |
| surface | 255,255,255 | cards, panels |
| surface_hi | 234,239,246 | inputs, hover |
| surface_lo | 241,244,249 | wells, tracks |
| border | 214,221,232 | decorative edges |
| border_strong | 100,116,139 | control outlines (>=3:1) |
| text / text_dim | 15,23,42 / 71,85,105 | body / secondary |
| ok / warn / danger | 21,110,50 / 160,72,8 / 185,28,28 | status text (AA on all surfaces) |

Switch: `theme::set_light(bool)` (global, no handle), read with `theme::is_light()`, active roles via `theme::pal()`. Call before `theme::apply(ctx)`. `apply` installs egui light or dark visuals plus the palette above. The `pub const` colours (BG, SURFACE, TEXT, ...) stay the dark palette for direct use; light-mode migration of those call sites is tracked separately.

## Spacing, radii, type, motion
- Spacing (8px grid): SP_XS 4, SP_S 8, SP_M 16, SP_L 24, SP_XL 32.
- Radii: R_XS 4 (badges), R_S 8 (buttons/inputs), R_M 12 (cards/tiles), R_L 16 (windows); chips/pills fully round.
- Elevation: `theme::elev(level)` -> Shadow. 0 flat, 1 card lift (blur 12, dy 3), 2 popup/toast/window (blur 28, dy 10). Dark uses stronger alpha than light.
- Type scale (px): TYPE_SMALL 12, TYPE_BODY 14.5 (body + button), TYPE_HEADING 26 (bold family), TYPE_MONO 13.5. Font: Segoe UI from the Windows font folder when present, else egui default fonts (no font file is bundled; no new crates). Ring digits = monospace at 20% of ring size.
- Theme switch: stored in the settings file as `light_theme` (`0` dark, `1` light; `engine.rs` Settings, read at startup by `Settings::parse`). Applied with `theme::set_light(bool)` then `theme::apply(ctx)`. `theme::set_theme_name(&str)` is a helper that maps "light" to `set_light(true)`.
- Borders 1px, selected 2px. Shadows only on popups/toasts.
- Motion: hover/press colour fades `ANIM`=0.15s via `animate_bool_with_time`; ring value 0.15s. No continuous repaint from widgets.

## Widgets (`ui::widgets`)
| Widget | Signature | States |
|---|---|---|
| card | `card(ui,title,\|ui\| ..)` | titled surface |
| section_header | `section_header(ui,text)` | heading |
| primary/danger/ghost_button | `(ui,text)->Response` | idle, hover (fade), pressed, focus ring, disabled (use `ui.add_enabled_ui`) |
| chip | `chip(ui,text,selected)->Response` | idle, hover, selected (tint + check), focus, disabled |
| status_pill | `status_pill(ui,text,PillKind)` | Ok, Warn, Danger, Info, Neutral (dot + text, never colour alone) |
| countdown_ring | `countdown_ring(ui,frac,big,small,color)` | frac = remaining 0..1; 180–260px; ACCENT work, WARN break |
| monitor_tile | `monitor_tile(ui,&MonitorInfo,selected,TileState)` | Idle, Selected (teal border + check badge), Off (power icon), Blackout (amber zz), Unsupported (cross, not clickable) |
| toast | `toast(ctx,text,PillKind)` | bottom-centre; caller shows it every frame while visible and owns the timer |
| gallery | `gallery(ui)` | every widget in every state, for Ctrl+Shift+D page |

Keyboard: every clickable widget is focusable; focus draws a 2px ACCENT_HI ring offset 2px. Widgets publish `WidgetInfo` for screen readers. State is never conveyed by colour alone (check marks, icons, labels).

Layout: widgets size themselves (tile 176x124, ring 180–260, button height 36); use `horizontal_wrapped` so 640x520 wraps cleanly.

## Contrast (computed, WCAG 2.x relative luminance; `F:\bgbuild\contrast.py`)

Text and status tokens (TEXT, TEXT_DIM, OK, WARN, DANGER, ACCENT) must reach WCAG AA 4.5:1 against SURFACE_LO. Ratios are not hand-copied here; `tokens_doc::contrast_report()` computes them and its test enforces the floor. Control outlines (BORDER_STRONG) need 3:1.

BORDER (38,49,72) is decorative only and intentionally below 3:1; no control relies on it for identification.

## 0.4.0 notes
Sessions are scanned by `sessions::spawn` on its own thread and sent over an mpsc channel as snapshots; the UI only receives them. Tray and hotkeys live on one hidden window thread (`tray.rs`), never started in mock mode.
