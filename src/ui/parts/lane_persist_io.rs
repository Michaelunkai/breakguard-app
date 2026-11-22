//! slice lane_persist_io (owned by one team agent)
//! API: pub fn load(path: &Path) -> LayoutSave
//! API: pub fn save(path: &Path, state: &LayoutSave) -> io::Result<()>
//! save writes a sibling temp file, then renames it over the target. load never panics:
//! missing, empty, corrupt or truncated files yield LayoutSave::default().
#![allow(dead_code)]
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::ui::logic::persist::LayoutSave;

/// Files larger than this are treated as corrupt; real layouts are a few KB.
const MAX_BYTES: u64 = 256 * 1024;

pub fn load(path: &Path) -> LayoutSave {
    match fs::metadata(path) {
        Ok(m) if m.len() <= MAX_BYTES => {}
        _ => return LayoutSave::default(),
    }
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return LayoutSave::default(),
    };
    let text = String::from_utf8_lossy(&bytes);
    if !is_complete(&text) {
        return LayoutSave::default();
    }
    LayoutSave::from_text(&text)
}

pub fn save(path: &Path, state: &LayoutSave) -> io::Result<()> {
    let tmp = tmp_path(path);
    let result = write_then_rename(&tmp, path, &state.to_text());
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn write_then_rename(tmp: &Path, path: &Path, text: &str) -> io::Result<()> {
    let mut file = fs::File::create(tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(tmp, path)
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// to_text always ends with the "s=" line followed by '\n', so a file without that
/// trailing line is truncated or corrupt.
fn is_complete(text: &str) -> bool {
    if !text.ends_with('\n') {
        return false;
    }
    let last = text.trim_end_matches('\n').rsplit('\n').next().unwrap_or("");
    let last = last.strip_suffix('\r').unwrap_or(last);
    match last.strip_prefix("s=") {
        Some(v) => v.trim().parse::<u8>().is_ok(),
        None => false,
    }
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

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lane_persist_io_{}_{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn roundtrip_equal() {
        let dir = test_dir("roundtrip");
        let p = dir.join("layout.txt");
        save(&p, &sample()).unwrap();
        assert_eq!(load(&p), sample());
        assert!(!tmp_path(&p).exists(), "temp file must be renamed away");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn overwrite_existing_file() {
        let dir = test_dir("overwrite");
        let p = dir.join("layout.txt");
        save(&p, &sample()).unwrap();
        let other = LayoutSave { sort: 1, ..Default::default() };
        save(&p, &other).unwrap();
        assert_eq!(load(&p), other);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_defaults() {
        let dir = test_dir("missing");
        assert_eq!(load(&dir.join("nope.txt")), LayoutSave::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_file_defaults() {
        let dir = test_dir("empty");
        let p = dir.join("layout.txt");
        fs::write(&p, b"").unwrap();
        assert_eq!(load(&p), LayoutSave::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_bytes_defaults() {
        let dir = test_dir("corrupt");
        let p = dir.join("layout.txt");
        let bytes: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        fs::write(&p, &bytes).unwrap();
        assert_eq!(load(&p), LayoutSave::default());
        fs::write(&p, b"\xff\xfe\x00garbage\nc=\x80\nh=\x00:\x00\n").unwrap();
        assert_eq!(load(&p), LayoutSave::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn truncated_file_defaults() {
        let dir = test_dir("truncated");
        let p = dir.join("layout.txt");
        let full = sample().to_text();
        for end in 0..full.len() {
            fs::write(&p, &full.as_bytes()[..end]).unwrap();
            assert_eq!(load(&p), LayoutSave::default(), "prefix of length {end} must be defaults");
        }
        fs::write(&p, full.as_bytes()).unwrap();
        assert_eq!(load(&p), sample());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn huge_file_defaults() {
        let dir = test_dir("huge");
        let p = dir.join("layout.txt");
        let mut text = sample().to_text();
        while (text.len() as u64) <= MAX_BYTES {
            text.push_str("c=filler\n");
        }
        text.push_str("s=3\n");
        fs::write(&p, text.as_bytes()).unwrap();
        assert_eq!(load(&p), LayoutSave::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn directory_path_defaults() {
        let dir = test_dir("dirpath");
        assert_eq!(load(&dir), LayoutSave::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_bad_path_errors() {
        let dir = test_dir("badpath");
        let p = dir.join("no_such_sub").join("layout.txt");
        assert!(save(&p, &LayoutSave::default()).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lane_persist_roundtrip_with_column_layout() {
        let dir = test_dir("col_roundtrip");
        let p = dir.join("layout.txt");
        let s = LayoutSave {
            collapsed: vec!["left:lane1".into(), "right:lane2".into()],
            heights: vec![("left:lane1".into(), 240.0), ("right:lane2".into(), 66.0), ("right:lane3".into(), 800.0)],
            filter: "col\\umn\nx".into(),
            sort: 4,
        };
        save(&p, &s).unwrap();
        assert_eq!(load(&p), s);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lane_persist_old_file_without_new_fields_loads() {
        let dir = test_dir("old_file");
        let p = dir.join("layout.txt");
        fs::write(&p, b"c=a\nc=b:c\ns=1\n").unwrap();
        let r = load(&p);
        assert_eq!(r.collapsed, vec!["a".to_string(), "b:c".to_string()]);
        assert!(r.heights.is_empty());
        assert_eq!(r.filter, "");
        assert_eq!(r.sort, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lane_persist_garbage_and_truncated_give_defaults() {
        let dir = test_dir("garbage");
        let p = dir.join("layout.txt");
        let garbage: [&[u8]; 4] = [
            b"\xde\xad\xbe\xef\x00\x01",
            b"c=a\nh=x:\xff\xfe\n",
            b"c=a\nh=b:1",
            b"s=\n",
        ];
        for g in garbage {
            fs::write(&p, g).unwrap();
            assert_eq!(load(&p), LayoutSave::default(), "garbage {g:?} must be defaults");
        }
        let full = sample().to_text();
        fs::write(&p, &full.as_bytes()[..full.len() - 3]).unwrap();
        assert_eq!(load(&p), LayoutSave::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lane_persist_huge_finite_height_is_capped() {
        let dir = test_dir("huge_height");
        let p = dir.join("layout.txt");
        fs::write(&p, b"h=a:3.4e38\nh=b:1e30\ns=0\n").unwrap();
        let r = load(&p);
        let hs: Vec<f32> = r.heights.iter().map(|x| x.1).collect();
        assert_eq!(hs, vec![800.0, 800.0]);
        assert!(hs.iter().all(|h| h.is_finite()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lane_persist_save_is_atomic_temp_then_rename() {
        let dir = test_dir("atomic");
        let p = dir.join("layout.txt");
        save(&p, &sample()).unwrap();
        let tmp = tmp_path(&p);
        assert_eq!(tmp.parent(), p.parent(), "temp file must be a sibling of target");
        assert_eq!(tmp.file_name().unwrap(), "layout.txt.tmp");

        // a blocking directory at the temp path makes the write fail before rename:
        // the previous good file must survive untouched and no temp must remain
        fs::create_dir(&tmp).unwrap();
        let other = LayoutSave { sort: 9, ..Default::default() };
        assert!(save(&p, &other).is_err());
        assert_eq!(load(&p), sample());
        assert!(tmp.is_dir(), "failed save must not remove unrelated paths");
        let _ = fs::remove_dir_all(&dir);
    }
}
