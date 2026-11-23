// Opt-in diagnostics. Active only when env BREAKGUARD_DIAG=<existing dir> is set.
// Writes <dir>\breakguard-diag.log and <dir>\breakguard-frame.bmp (GL framebuffer of OUR window only).
use eframe::egui;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

pub static FRAMES: AtomicU64 = AtomicU64::new(0);

pub fn dir() -> Option<std::path::PathBuf> {
    let d = std::env::var_os("BREAKGUARD_DIAG")?;
    let p = std::path::PathBuf::from(d);
    if p.is_dir() { Some(p) } else { None }
}

pub fn log(msg: &str) {
    if let Some(d) = dir() {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("breakguard-diag.log")) {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|x| x.as_secs()).unwrap_or(0);
            let _ = writeln!(f, "[{t}] {msg}");
        }
    }
}

pub fn install_panic_hook() {
    if dir().is_none() { return; }
    std::panic::set_hook(Box::new(|i| log(&format!("PANIC: {i}"))));
    log("diag on");
}

pub fn log_gl(cc: &eframe::CreationContext<'_>) {
    if dir().is_none() { return; }
    match cc.gl.as_ref() {
        None => log("GL: cc.gl is None"),
        Some(gl) => {
            use eframe::glow::HasContext;
            unsafe {
                log(&format!("GL renderer={} vendor={} version={}",
                    gl.get_parameter_string(eframe::glow::RENDERER),
                    gl.get_parameter_string(eframe::glow::VENDOR),
                    gl.get_parameter_string(eframe::glow::VERSION)));
            }
        }
    }
}

// Call every update(). Returns Some(tab_index) when the app should switch tab.
// Per tab i (0..5): switch at frame 20+40*i, screenshot at frame 30+40*i, write breakguard-tabN.bmp.
pub static LAST_SHOT: AtomicU64 = AtomicU64::new(0);

pub fn per_frame(ctx: &egui::Context) -> Option<usize> {
    if dir().is_none() { return None; }
    let n = FRAMES.fetch_add(1, Ordering::SeqCst) + 1;
    if n == 1 || n % 40 == 0 {
        let (outer, inner, minimized, focused, ppp) = ctx.input(|i| {
            let v = i.viewport();
            (v.outer_rect, v.inner_rect, v.minimized, v.focused, i.pixels_per_point())
        });
        log(&format!("frame={n} outer={outer:?} inner={inner:?} minimized={minimized:?} focused={focused:?} ppp={ppp}"));
    }
    let mut switch = None;
    if n >= 20 && (n - 20) % 60 == 0 && (n - 20) / 60 < 6 { switch = Some(((n - 20) / 60) as usize); }
    if n >= 50 && (n - 50) % 60 == 0 && (n - 50) / 60 < 6 {
        LAST_SHOT.store((n - 50) / 60, Ordering::SeqCst);
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        log(&format!("screenshot requested tab {}", (n - 50) / 60));
    }
    let shot = ctx.input(|i| i.events.iter().find_map(|e| match e {
        egui::Event::Screenshot { image, .. } => Some(image.clone()), _ => None }));
    if let Some(img) = shot {
        let [w, h] = img.size;
        let t = LAST_SHOT.load(Ordering::SeqCst);
        let mut distinct = std::collections::HashSet::new();
        for p in img.pixels.iter().step_by(97) { distinct.insert(p.to_array()); }
        log(&format!("screenshot tab {t} got {w}x{h} distinct_sampled_colors={}", distinct.len()));
        if let Some(d) = dir() { let _ = write_bmp(&d.join(format!("breakguard-tab{t}.bmp")), w, h, &img.pixels); }
        if t == 5 { log("diag done, closing"); ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
    }
    if n == 600 { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
    switch
}

fn write_bmp(path: &std::path::Path, w: usize, h: usize, px: &[egui::Color32]) -> std::io::Result<()> {
    let row = (w * 3 + 3) & !3;
    let size = 54 + row * h;
    let mut b = Vec::with_capacity(size);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&(size as u32).to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(w as i32).to_le_bytes());
    b.extend_from_slice(&(h as i32).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&[0; 24]);
    for y in (0..h).rev() {
        let start = b.len();
        for x in 0..w { let p = px[y * w + x]; b.extend_from_slice(&[p.b(), p.g(), p.r()]); }
        while b.len() - start < row { b.push(0); }
    }
    std::fs::write(path, b)
}

pub fn apply_scroll(sa: egui::ScrollArea) -> egui::ScrollArea {
    if dir().is_none() { return sa; }
    match std::env::var("BREAKGUARD_DIAG_SCROLL").ok().and_then(|s| s.parse::<f32>().ok()) {
        Some(y) => sa.vertical_scroll_offset(y),
        None => sa,
    }
}
