//! egui theme tokens ("calm night"). All colours, spacing, radii, fonts and the user-selectable accent
//! live here; see DESIGN.md for the contrast table.
use eframe::egui::{
    self, epaint::Shadow, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, TextStyle,
};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

// ---- palette (see DESIGN.md) ----
pub const BG: Color32 = Color32::from_rgb(9, 12, 22);
pub const SURFACE: Color32 = Color32::from_rgb(16, 22, 36);
pub const SURFACE_HI: Color32 = Color32::from_rgb(26, 35, 55);
/// Slightly darker than SURFACE: inset wells, tracks.
pub const SURFACE_LO: Color32 = Color32::from_rgb(12, 17, 29);
/// Decorative separators only (not for identifying controls).
pub const BORDER: Color32 = Color32::from_rgb(38, 49, 72);
/// Control outlines, >= 3:1 on SURFACE.
pub const BORDER_STRONG: Color32 = Color32::from_rgb(100, 116, 139);
pub const TEXT: Color32 = Color32::from_rgb(232, 238, 247);
pub const TEXT_DIM: Color32 = Color32::from_rgb(148, 163, 184);
pub const OK: Color32 = Color32::from_rgb(74, 222, 128);
pub const WARN: Color32 = Color32::from_rgb(251, 191, 36);
pub const DANGER: Color32 = Color32::from_rgb(248, 113, 113);
pub const DANGER_HI: Color32 = Color32::from_rgb(252, 165, 165);
/// Dark text used on top of bright fills (accent, danger, warn).
pub const ON_FILL: Color32 = Color32::from_rgb(6, 20, 24);

/// Surface and text roles that change between dark and light. `apply` reads these; the `pub const`
/// values above stay the dark palette for code that still uses them directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pal {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_hi: Color32,
    pub surface_lo: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_dim: Color32,
}

pub const DARK_PAL: Pal = Pal {
    bg: BG,
    surface: SURFACE,
    surface_hi: SURFACE_HI,
    surface_lo: SURFACE_LO,
    border: BORDER,
    border_strong: BORDER_STRONG,
    text: TEXT,
    text_dim: TEXT_DIM,
};

pub const LIGHT_PAL: Pal = Pal {
    bg: Color32::from_rgb(246, 248, 251),
    surface: Color32::from_rgb(255, 255, 255),
    surface_hi: Color32::from_rgb(234, 239, 246),
    surface_lo: Color32::from_rgb(241, 244, 249),
    border: Color32::from_rgb(214, 221, 232),
    border_strong: Color32::from_rgb(100, 116, 139),
    text: Color32::from_rgb(15, 23, 42),
    text_dim: Color32::from_rgb(71, 85, 105),
};

/// Status colours for the light palette (dark-palette OK/WARN/DANGER are too pale on white).
pub const LIGHT_OK: Color32 = Color32::from_rgb(21, 110, 50);
pub const LIGHT_WARN: Color32 = Color32::from_rgb(160, 72, 8);
pub const LIGHT_DANGER: Color32 = Color32::from_rgb(185, 28, 28);

static LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Settings hook: `true` switches window/panel/text/control colours to the light palette.
pub fn set_light(on: bool) {
    LIGHT.store(on, Ordering::Relaxed);
}
pub fn is_light() -> bool {
    LIGHT.load(Ordering::Relaxed)
}
/// Active surface/text palette.
pub fn pal() -> Pal {
    if is_light() { LIGHT_PAL } else { DARK_PAL }
}
/// Accent used for links, selection stroke and caret. Light mode darkens it (mix 50% to black) so it
/// keeps >= 4.5:1 on white surfaces for every swatch.
pub fn link_accent(light: bool) -> Color32 {
    if light { mix(accent(), Color32::BLACK, 0.5) } else { accent() }
}

/// Selectable accent colours (name, colour). Index is stored in settings (`accent`).
pub const SWATCHES: [(&str, Color32); 6] = [
    ("Teal", Color32::from_rgb(45, 212, 191)),
    ("Sky", Color32::from_rgb(96, 165, 250)),
    ("Violet", Color32::from_rgb(167, 139, 250)),
    ("Rose", Color32::from_rgb(251, 113, 133)),
    ("Orange", Color32::from_rgb(251, 146, 60)),
    ("Lime", Color32::from_rgb(163, 230, 53)),
];

static ACCENT_IDX: AtomicU8 = AtomicU8::new(0);

pub fn set_accent(i: u8) {
    ACCENT_IDX.store(i.min(SWATCHES.len() as u8 - 1), Ordering::Relaxed);
}
pub fn accent_index() -> u8 {
    ACCENT_IDX.load(Ordering::Relaxed)
}
pub fn accent() -> Color32 {
    SWATCHES[accent_index() as usize].1
}
pub fn accent_hi() -> Color32 {
    mix(accent(), Color32::WHITE, 0.35)
}
pub fn accent_lo() -> Color32 {
    mix(accent(), Color32::BLACK, 0.3)
}

// ---- spacing scale (8px grid) ----
pub const SP_XS: f32 = 4.0;
pub const SP_S: f32 = 8.0;
pub const SP_M: f32 = 16.0;
pub const SP_L: f32 = 24.0;
#[allow(dead_code)] // reserved for section gaps on Settings
pub const SP_XL: f32 = 32.0;

// ---- type scale (px) ----
pub const TYPE_SMALL: f32 = 12.0;
pub const TYPE_BODY: f32 = 14.5;
pub const TYPE_HEADING: f32 = 26.0;
pub const TYPE_MONO: f32 = 13.5;

// ---- elevation: 0 flat, 1 card lift, 2 popup/toast ----
/// Shadow for an elevation level. Dark theme uses stronger alpha than light.
pub fn elev(level: u8) -> Shadow {
    let light = is_light();
    let (blur, dy, a) = match level {
        0 => (0, 0, 0),
        1 => (12, 3, if light { 18 } else { 60 }),
        _ => (28, 10, if light { 40 } else { 140 }),
    };
    Shadow { offset: [0, dy], blur, spread: 0, color: Color32::from_black_alpha(a) }
}

/// Switch from a settings string: "dark" or "light" (anything else = dark).
#[allow(dead_code)] // called by Settings once SettToast wires the theme control
pub fn set_theme_name(name: &str) {
    set_light(name.eq_ignore_ascii_case("light"));
}

// ---- radii ----
#[allow(dead_code)] // reserved for chips and badges
pub const R_XS: u8 = 4;
pub const R_S: u8 = 8;
pub const R_M: u8 = 12;
pub const R_L: u8 = 16;

/// Max animation time (s).
pub const ANIM: f32 = 0.15;

pub fn rad(r: u8) -> CornerRadius {
    CornerRadius::same(r)
}

/// Font for log lines and code.
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Bold family (Segoe UI Bold when installed, otherwise the default font).
pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

/// Big numerals and headings.
pub fn num(size: f32) -> FontId {
    FontId::new(size, bold_family())
}

/// Mix `a` towards `b` by `t` (0..1), in gamma space.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

/// Transparent version of `c` (alpha 0..255).
pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Soft glow: stacked translucent strokes around a circle (hero ring, running dots).
pub fn glow_circle(p: &egui::Painter, c: egui::Pos2, r: f32, col: Color32, layers: u8) {
    for i in (1..=layers).rev() {
        let t = i as f32 / layers as f32;
        p.circle_stroke(c, r + 2.0 * i as f32, Stroke::new(2.5_f32, with_alpha(col, (34.0 * (1.0 - t) + 6.0) as u8)));
    }
}

/// Vertical two-colour gradient filling `rect` (cheap mesh, no texture).
#[allow(dead_code)] // reserved for banner backgrounds
pub fn gradient_rect(p: &egui::Painter, rect: egui::Rect, top: Color32, bottom: Color32, radius: u8) {
    p.rect_filled(rect, rad(radius), top);
    let h = rect.height();
    let steps = (h / 4.0).ceil().max(1.0) as usize;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let r = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + h * t),
            egui::pos2(rect.right(), (rect.top() + h * (i + 1) as f32 / steps as f32).min(rect.bottom())),
        );
        let rr = if i == 0 { CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 } } else if i + 1 == steps { CornerRadius { nw: 0, ne: 0, sw: radius, se: radius } } else { CornerRadius::ZERO };
        p.rect_filled(r, rr, mix(top, bottom, t));
    }
}


fn read_font(file: &str) -> Option<Vec<u8>> {
    let win = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
    std::fs::read(win.join("Fonts").join(file)).ok()
}

/// Segoe UI (regular, bold, symbols) from the Windows font folder when present; the bundled default
/// fonts stay as fallback so a missing file never breaks the UI.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut regular = false;
    let mut bold = false;
    if let Some(b) = read_font("segoeui.ttf") {
        fonts.font_data.insert("segoe".into(), Arc::new(FontData::from_owned(b)));
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "segoe".into());
        regular = true;
    }
    if let Some(b) = read_font("segoeuib.ttf") {
        fonts.font_data.insert("segoe-bold".into(), Arc::new(FontData::from_owned(b)));
        bold = true;
    }
    if let Some(b) = read_font("seguisym.ttf") {
        fonts.font_data.insert("segoe-sym".into(), Arc::new(FontData::from_owned(b)));
        fonts.families.entry(FontFamily::Proportional).or_default().push("segoe-sym".into());
    }
    let mut bold_list: Vec<String> = Vec::new();
    if bold {
        bold_list.push("segoe-bold".into());
    } else if regular {
        bold_list.push("segoe".into());
    }
    bold_list.extend(fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    fonts.families.insert(bold_family(), bold_list);
    ctx.set_fonts(fonts);
}

/// Install visuals, spacing and text styles. Call at startup and whenever the accent changes.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let acc = accent();

    style.text_styles = [
        (TextStyle::Small, FontId::proportional(TYPE_SMALL)),
        (TextStyle::Body, FontId::proportional(TYPE_BODY)),
        (TextStyle::Button, FontId::proportional(TYPE_BODY)),
        (TextStyle::Heading, num(TYPE_HEADING)),
        (TextStyle::Monospace, FontId::monospace(TYPE_MONO)),
    ]
    .into();

    let sp = &mut style.spacing;
    sp.item_spacing = egui::vec2(SP_S, SP_S);
    sp.button_padding = egui::vec2(SP_M, SP_S);
    sp.interact_size = egui::vec2(40.0, 32.0);
    sp.window_margin = Margin::same(SP_M as i8);
    sp.menu_margin = Margin::same(SP_S as i8);
    sp.indent = SP_M;
    sp.slider_width = 220.0;
    sp.text_edit_width = 240.0;
    sp.scroll.bar_width = 8.0;
    // Larger checkbox box so the 8px widget radius reads as a rounded square, not a circle.
    sp.icon_width = 20.0;
    sp.icon_width_inner = 12.0;

    let p = pal();
    let light = is_light();
    let v = &mut style.visuals;
    *v = if light { egui::Visuals::light() } else { egui::Visuals::dark() };
    v.override_text_color = Some(p.text);
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0_f32, p.border);
    v.window_corner_radius = rad(R_L);
    v.menu_corner_radius = rad(R_S);
    v.extreme_bg_color = p.surface_lo;
    v.faint_bg_color = p.surface;
    v.code_bg_color = p.surface_hi;
    let link = link_accent(light);
    v.hyperlink_color = link;
    v.warn_fg_color = if light { LIGHT_WARN } else { WARN };
    v.error_fg_color = if light { LIGHT_DANGER } else { DANGER };
    v.window_shadow = elev(2);
    v.popup_shadow = v.window_shadow;
    v.selection.bg_fill = with_alpha(acc, 70);
    v.selection.stroke = Stroke::new(1.5_f32, link);
    v.text_cursor.stroke = Stroke::new(2.0_f32, link);

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
    w.noninteractive.corner_radius = rad(R_S);

    // bg_fill also paints slider rails; border_strong keeps the track >= 3:1 on surface.
    w.inactive.bg_fill = p.border_strong;
    w.inactive.weak_bg_fill = p.surface_hi;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, p.border_strong);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
    w.inactive.corner_radius = rad(R_S);

    w.hovered.bg_fill = mix(p.surface_hi, acc, 0.14);
    w.hovered.weak_bg_fill = w.hovered.bg_fill;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, acc);
    w.hovered.fg_stroke = Stroke::new(1.5_f32, p.text);
    w.hovered.corner_radius = rad(R_S);

    w.active.bg_fill = mix(p.surface_hi, acc, 0.28);
    w.active.weak_bg_fill = w.active.bg_fill;
    w.active.bg_stroke = Stroke::new(1.5_f32, accent_hi());
    w.active.fg_stroke = Stroke::new(2.0_f32, p.text);
    w.active.corner_radius = rad(R_S);

    w.open = w.active;

    ctx.set_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lum(c: Color32) -> f32 {
        let f = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.03928 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }
    fn ratio(a: Color32, b: Color32) -> f32 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn text_and_control_contrast_meets_aa() {
        // pill/status text on its tinted pill fill (worst case 0.14 tint) and plain text on surfaces
        for c in [OK, WARN, DANGER, TEXT_DIM] {
            assert!(ratio(c, mix(SURFACE, c, 0.14)) >= 4.5, "pill text {c:?}");
        }
        for bg in [BG, SURFACE, SURFACE_HI, SURFACE_LO] {
            assert!(ratio(TEXT, bg) >= 4.5 && ratio(TEXT_DIM, bg) >= 4.5, "text on {bg:?}");
        }
        // control outlines and slider rail >= 3:1 on the surface they sit on
        assert!(ratio(BORDER_STRONG, SURFACE) >= 3.0);
        assert!(ratio(BORDER_STRONG, BG) >= 3.0);
    }

    #[test]
    fn dark_and_light_palettes_meet_aa() {
        for (name, p, ok, warn, danger) in [
            ("dark", DARK_PAL, OK, WARN, DANGER),
            ("light", LIGHT_PAL, LIGHT_OK, LIGHT_WARN, LIGHT_DANGER),
        ] {
            for bg in [p.bg, p.surface, p.surface_hi, p.surface_lo] {
                assert!(ratio(p.text, bg) >= 4.5, "{name} text on {bg:?}");
                assert!(ratio(p.text_dim, bg) >= 4.5, "{name} text_dim on {bg:?}");
                assert!(ratio(ok, bg) >= 4.5, "{name} ok on {bg:?}");
                assert!(ratio(warn, bg) >= 4.5, "{name} warn on {bg:?}");
                assert!(ratio(danger, bg) >= 4.5, "{name} danger on {bg:?}");
            }
            assert!(ratio(p.border_strong, p.surface) >= 3.0, "{name} outline on surface");
            assert!(ratio(p.border_strong, p.bg) >= 3.0, "{name} outline on bg");
        }
    }

    /// Serialises tests that write the global accent/light switches (cargo runs tests in parallel).
    static GLOBALS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn globals() -> std::sync::MutexGuard<'static, ()> {
        GLOBALS.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn light_link_accent_meets_aa_on_every_swatch() {
        let _g = globals();
        for (name, c) in SWATCHES {
            set_accent(SWATCHES.iter().position(|s| s.0 == name).unwrap() as u8);
            let link = link_accent(true);
            for bg in [LIGHT_PAL.surface, LIGHT_PAL.surface_hi, LIGHT_PAL.bg] {
                assert!(ratio(link, bg) >= 4.5, "{name} link on {bg:?}");
            }
            assert_eq!(link_accent(false), c, "{name} dark accent unchanged");
        }
        set_accent(0);
    }

    #[test]
    fn text_on_hover_and_active_fills_meets_aa_both_palettes() {
        for (name, p) in [("dark", DARK_PAL), ("light", LIGHT_PAL)] {
            for (_, c) in SWATCHES {
                for t in [0.14, 0.28] {
                    let fill = mix(p.surface_hi, c, t);
                    assert!(ratio(p.text, fill) >= 4.5, "{name} text on fill {fill:?} t={t}");
                }
            }
        }
    }

    #[test]
    fn light_switch_selects_palette() {
        let _g = globals();
        set_light(true);
        assert_eq!(pal(), LIGHT_PAL);
        set_light(false);
        assert_eq!(pal(), DARK_PAL);
    }

    #[test]
    fn every_accent_swatch_meets_aa() {
        for (name, c) in SWATCHES {
            assert!(ratio(c, mix(SURFACE, c, 0.14)) >= 4.5, "{name} text on tinted pill");
            assert!(ratio(c, mix(SURFACE, c, 0.20)) >= 4.5, "{name} text on selected chip");
            assert!(ratio(c, SURFACE) >= 3.0 && ratio(c, BG) >= 3.0, "{name} outline");
            assert!(ratio(ON_FILL, c) >= 4.5, "{name} dark text on accent fill");
            assert!(ratio(ON_FILL, mix(c, Color32::WHITE, 0.35)) >= 4.5, "{name} on hover fill");
        }
    }

    #[test]
    fn light_text_pairs_meet_aa_on_every_light_surface() {
        let _g = globals();
        // Every light text role (and the light link accent, every swatch) on every light surface.
        let p = LIGHT_PAL;
        let surfaces = [p.bg, p.surface, p.surface_hi, p.surface_lo];
        let mut texts = vec![("text", p.text), ("text_dim", p.text_dim), ("ok", LIGHT_OK), ("warn", LIGHT_WARN), ("danger", LIGHT_DANGER)];
        for (idx, _) in SWATCHES.iter().enumerate() {
            set_accent(idx as u8);
            texts.push(("link", link_accent(true)));
        }
        set_accent(0);
        for (name, t) in texts {
            for s in surfaces {
                let r = ratio(t, s);
                assert!(r >= 4.5, "light {name} {t:?} on {s:?} = {r:.2}");
            }
        }
    }

    #[test]
    fn accent_index_is_clamped() {
        let _g = globals();
        set_accent(200);
        assert_eq!(accent_index(), 5);
        set_accent(0);
        assert_eq!(accent(), SWATCHES[0].1);
    }
}
