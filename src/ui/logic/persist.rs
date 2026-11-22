#![allow(dead_code)]
use std::fs;
use std::io;
use std::path::Path;

const MIN_H: f32 = 66.0;
const MAX_H: f32 = 800.0;

#[derive(PartialEq, Debug, Clone, Default)]
pub struct LayoutSave {
    pub collapsed: Vec<String>,
    pub heights: Vec<(String, f32)>,
    pub filter: String,
    pub sort: u8,
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            c => o.push(c),
        }
    }
    o
}

fn unesc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            o.push(c);
            continue;
        }
        match it.next() {
            Some('n') => o.push('\n'),
            Some('r') => o.push('\r'),
            Some('\\') => o.push('\\'),
            Some(x) => {
                o.push('\\');
                o.push(x);
            }
            None => o.push('\\'),
        }
    }
    o
}

impl LayoutSave {
    pub fn to_text(&self) -> String {
        let mut o = String::new();
        // empty ids are dropped on load, so never write them
        for c in self.collapsed.iter().filter(|c| !c.is_empty()) {
            o.push_str(&format!("c={}\n", esc(c)));
        }
        for (id, h) in self.heights.iter().filter(|(id, _)| !id.is_empty()) {
            o.push_str(&format!("h={}:{}\n", esc(id), h));
        }
        o.push_str(&format!("f={}\n", esc(&self.filter)));
        o.push_str(&format!("s={}\n", self.sort));
        o
    }

    pub fn from_text(t: &str) -> LayoutSave {
        let mut r = LayoutSave::default();
        let t = t.strip_prefix('\u{feff}').unwrap_or(t);
        for line in t.split('\n') {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if let Some(v) = line.strip_prefix("c=") {
                let id = unesc(v);
                if !id.is_empty() {
                    r.collapsed.push(id);
                }
            } else if let Some(v) = line.strip_prefix("h=") {
                // id may contain ':'; the number is after the last one
                if let Some((id, n)) = v.rsplit_once(':') {
                    let id = unesc(id);
                    if let Ok(h) = n.trim().parse::<f32>() {
                        if !id.is_empty() && !h.is_nan() {
                            r.heights.push((id, h.clamp(MIN_H, MAX_H)));
                        }
                    }
                }
            } else if let Some(v) = line.strip_prefix("f=") {
                r.filter = unesc(v);
            } else if let Some(v) = line.strip_prefix("s=") {
                r.sort = v.trim().parse::<u8>().unwrap_or(0);
            }
        }
        r
    }
}

pub fn load(path: &Path) -> LayoutSave {
    match fs::read(path) {
        Ok(b) => LayoutSave::from_text(&String::from_utf8_lossy(&b)),
        Err(_) => LayoutSave::default(),
    }
}

pub fn save(path: &Path, s: &LayoutSave) -> io::Result<()> {
    fs::write(path, s.to_text())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> LayoutSave {
        LayoutSave {
            collapsed: vec!["a".into(), "b:c".into()],
            heights: vec![("a".into(), 120.5), ("x:y".into(), 700.0)],
            filter: "err or warn".into(),
            sort: 3,
        }
    }

    #[test]
    fn roundtrip() {
        let s = sample();
        assert_eq!(LayoutSave::from_text(&s.to_text()), s);
    }

    #[test]
    fn format_lines() {
        let t = sample().to_text();
        assert_eq!(t, "c=a\nc=b:c\nh=a:120.5\nh=x:y:700\nf=err or warn\ns=3\n");
    }

    #[test]
    fn roundtrip_default() {
        let d = LayoutSave::default();
        assert_eq!(LayoutSave::from_text(&d.to_text()), d);
    }

    #[test]
    fn roundtrip_nasty_strings() {
        let s = LayoutSave {
            collapsed: vec!["a\nb".into(), "back\\slash".into(), "\\n".into(), "é😀".into()],
            heights: vec![("l1\r\nl2".into(), 100.0)],
            filter: "x\ny\r\\".into(),
            sort: 255,
        };
        assert_eq!(LayoutSave::from_text(&s.to_text()), s);
    }

    #[test]
    fn empty_ids_not_written() {
        let s = LayoutSave {
            collapsed: vec!["".into(), "ok".into()],
            heights: vec![("".into(), 100.0), ("k".into(), 100.0)],
            ..Default::default()
        };
        let r = LayoutSave::from_text(&s.to_text());
        assert_eq!(r.collapsed, vec!["ok"]);
        assert_eq!(r.heights, vec![("k".to_string(), 100.0)]);
    }

    #[test]
    fn empty_input() {
        assert_eq!(LayoutSave::from_text(""), LayoutSave::default());
    }

    #[test]
    fn corrupt_input() {
        let t = "garbage\nc=\nh=noColon\nh=a:abc\nh=:50\nh=b:NaN\ns=999\ns=-1\nzzz=1\n=\n:";
        assert_eq!(LayoutSave::from_text(t), LayoutSave::default());
    }

    #[test]
    fn partial_corruption_keeps_good() {
        let t = "c=ok\nh=bad\nh=k:90\ns=x\nf=hi\n";
        let r = LayoutSave::from_text(t);
        assert_eq!(r.collapsed, vec!["ok"]);
        assert_eq!(r.heights, vec![("k".to_string(), 90.0)]);
        assert_eq!(r.filter, "hi");
        assert_eq!(r.sort, 0);
    }

    #[test]
    fn clamp_heights() {
        let r = LayoutSave::from_text("h=a:1\nh=b:99999\nh=c:-5\nh=d:inf\nh=e:-inf\nh=f:66\nh=g:800\n");
        let hs: Vec<f32> = r.heights.iter().map(|x| x.1).collect();
        assert_eq!(hs, vec![66.0, 800.0, 66.0, 800.0, 66.0, 66.0, 800.0]);
    }

    #[test]
    fn crlf_and_bom() {
        let r = LayoutSave::from_text("\u{feff}c=a\r\nh=a:100\r\nf=q\r\ns=2\r\n");
        assert_eq!(r.collapsed, vec!["a"]);
        assert_eq!(r.heights, vec![("a".to_string(), 100.0)]);
        assert_eq!(r.filter, "q");
        assert_eq!(r.sort, 2);
    }

    #[test]
    fn binary_no_panic() {
        let bytes: Vec<u8> = (0..=255u8).cycle().take(5000).collect();
        let txt = String::from_utf8_lossy(&bytes).to_string();
        let _ = LayoutSave::from_text(&txt);
        let _ = LayoutSave::from_text("\0\0\0c=\0\nh=\0:\0\n\u{fffd}");
    }

    #[test]
    fn nan_in_memory_not_persisted_as_valid() {
        let s = LayoutSave { heights: vec![("n".into(), f32::NAN)], ..Default::default() };
        assert!(LayoutSave::from_text(&s.to_text()).heights.is_empty());
    }

    #[test]
    fn last_wins_scalars() {
        let r = LayoutSave::from_text("f=a\nf=b\ns=1\ns=2\n");
        assert_eq!(r.filter, "b");
        assert_eq!(r.sort, 2);
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("persist_{}_{}_{}", std::process::id(), nanos, name))
    }

    #[test]
    fn missing_file_default() {
        assert_eq!(load(&tmp("nope_missing")), LayoutSave::default());
    }

    #[test]
    fn save_load_roundtrip() {
        let p = tmp("rt");
        let s = sample();
        save(&p, &s).unwrap();
        assert_eq!(load(&p), s);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn load_binary_file() {
        let p = tmp("bin");
        fs::write(&p, [0xffu8, 0xfe, 0, 1, 2, 0x80, b'\n', b's', b'=', b'7']).unwrap();
        assert_eq!(load(&p).sort, 7);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn load_directory_is_default() {
        assert_eq!(load(&std::env::temp_dir()), LayoutSave::default());
    }

    #[test]
    fn save_bad_path_errors() {
        let p = tmp("no_such_dir").join("x").join("y");
        assert!(save(&p, &LayoutSave::default()).is_err());
    }
}
