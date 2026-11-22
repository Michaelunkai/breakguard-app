#![allow(dead_code)]
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAX_LEN: usize = 60;
const FALLBACK: &str = "session";

fn is_reserved(s: &str) -> bool {
    let stem = s.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    match stem.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" => true,
        _ => {
            let b = stem.as_bytes();
            b.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && (b'1'..=b'9').contains(&b[3])
        }
    }
}

fn trim_edges(s: &str) -> &str {
    s.trim_matches(|c: char| c == '.' || c.is_whitespace())
}

pub fn safe_filename(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = trim_edges(&replaced);
    let capped: String = trimmed.chars().take(MAX_LEN).collect();
    let out = trim_edges(&capped);
    if out.is_empty() || is_reserved(out) {
        FALLBACK.to_string()
    } else {
        out.to_string()
    }
}

pub fn export_text(lines: &[String]) -> String {
    let mut s = String::new();
    for l in lines {
        s.push_str(l);
        s.push_str("\r\n");
    }
    s
}

pub fn write_export(dir: &Path, name: &str, lines: &[String]) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.log", safe_filename(name)));
    fs::write(&path, export_text(lines))?;
    Ok(path)
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "bg_export_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn plain_kept() {
        assert_eq!(safe_filename("my session 1"), "my session 1");
    }
    #[test]
    fn bad_chars_replaced() {
        assert_eq!(safe_filename("a<b>c:d\"e/f\\g|h?i*j"), "a_b_c_d_e_f_g_h_i_j");
    }
    #[test]
    fn control_chars_replaced() {
        assert_eq!(safe_filename("a\nb\t\0c\u{7f}d"), "a_b__c_d");
    }
    #[test]
    fn trims_dots_spaces() {
        assert_eq!(safe_filename("  ..name.. "), "name");
        assert_eq!(safe_filename(". . ."), "session");
    }
    #[test]
    fn empty_and_blank() {
        assert_eq!(safe_filename(""), "session");
        assert_eq!(safe_filename("   "), "session");
        assert_eq!(safe_filename("..."), "session");
    }
    #[test]
    fn reserved() {
        for n in ["CON", "con", "Prn", "NUL", "COM1", "com9", "LPT3", "AUX", "CON.txt", "nul.", " con "] {
            assert_eq!(safe_filename(n), "session", "{n}");
        }
    }
    #[test]
    fn near_reserved_kept() {
        for n in ["COM0", "COM10", "CONSOLE", "COM", "xCON", "LPT"] {
            assert_eq!(safe_filename(n), n);
        }
    }
    #[test]
    fn hebrew_kept() {
        assert_eq!(safe_filename("הפסקה בוקר"), "הפסקה בוקר");
    }
    #[test]
    fn cap_60_chars() {
        let s = "א".repeat(100);
        let o = safe_filename(&s);
        assert_eq!(o.chars().count(), 60);
        assert!(o.len() > 60); // multibyte, cut on char boundary
        assert_eq!(safe_filename(&"a".repeat(61)).len(), 60);
        assert_eq!(safe_filename(&"a".repeat(60)).len(), 60);
    }
    #[test]
    fn cap_then_trim() {
        let s = format!("{}{}b", "a".repeat(59), " ..");
        assert_eq!(safe_filename(&s), "a".repeat(59));
    }
    #[test]
    fn cap_to_reserved() {
        let s = format!("CON{}x", " ".repeat(60));
        assert_eq!(safe_filename(&s), "session");
    }
    #[test]
    fn traversal() {
        let o = safe_filename("../../etc/passwd");
        assert!(!o.contains('/') && !o.contains('\\') && !o.starts_with('.'));
        assert_eq!(o, "_.._etc_passwd");
        assert_eq!(safe_filename("..\\..\\x"), "_.._x");
    }
    #[test]
    fn idempotent() {
        for n in ["a/b", "CON", "  x. ", "", &"é".repeat(90)] {
            let a = safe_filename(n);
            assert_eq!(safe_filename(&a), a);
        }
    }
    #[test]
    fn text_empty() {
        assert_eq!(export_text(&[]), "");
    }
    #[test]
    fn text_join() {
        assert_eq!(export_text(&v(&["a", "b"])), "a\r\nb\r\n");
        assert_eq!(export_text(&v(&["x"])), "x\r\n");
    }
    #[test]
    fn text_empty_line_kept() {
        assert_eq!(export_text(&v(&[""])), "\r\n");
        assert_eq!(export_text(&v(&["a", "", "b"])), "a\r\n\r\nb\r\n");
    }
    #[test]
    fn write_roundtrip() {
        let d = tmp();
        let lines = v(&["שלום", "line2", ""]);
        let p = write_export(&d, "my/log", &lines).unwrap();
        assert_eq!(p, d.join("my_log.log"));
        assert_eq!(fs::read(&p).unwrap(), export_text(&lines).into_bytes());
        let _ = fs::remove_dir_all(&d);
    }
    #[test]
    fn write_empty_lines_empty_file() {
        let d = tmp();
        let p = write_export(&d, "e", &[]).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "");
        let _ = fs::remove_dir_all(&d);
    }
    #[test]
    fn write_reserved_name() {
        let d = tmp();
        let p = write_export(&d, "CON", &v(&["a"])).unwrap();
        assert_eq!(p.file_name().unwrap(), "session.log");
        let _ = fs::remove_dir_all(&d);
    }
    #[test]
    fn write_overwrites() {
        let d = tmp();
        write_export(&d, "n", &v(&["long long long"])).unwrap();
        let p = write_export(&d, "n", &v(&["s"])).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "s\r\n");
        let _ = fs::remove_dir_all(&d);
    }
    #[test]
    fn write_stays_in_dir() {
        let d = tmp();
        let p = write_export(&d, "../../escape", &v(&["a"])).unwrap();
        assert_eq!(p.parent().unwrap(), d.as_path());
        let _ = fs::remove_dir_all(&d);
    }
    #[test]
    fn write_dir_is_file_errors() {
        let d = tmp();
        fs::create_dir_all(&d).unwrap();
        let f = d.join("afile");
        fs::write(&f, "x").unwrap();
        assert!(write_export(&f, "n", &v(&["a"])).is_err());
        let _ = fs::remove_dir_all(&d);
    }
}
