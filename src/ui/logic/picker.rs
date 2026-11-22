#![allow(dead_code)]
use std::collections::BTreeMap;

pub const CREATURES: usize = 12;

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Overrides {
    map: BTreeMap<String, usize>,
}

fn id_ok(id: &str) -> bool {
    !id.is_empty() && !id.contains(['=', '\n', '\r'])
}

impl Overrides {
    pub fn set(&mut self, id: &str, idx: usize) {
        if idx < CREATURES && id_ok(id) {
            self.map.insert(id.to_string(), idx);
        }
    }

    pub fn clear(&mut self, id: &str) {
        self.map.remove(id);
    }

    pub fn get(&self, id: &str) -> Option<usize> {
        self.map.get(id).copied()
    }

    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for (id, idx) in &self.map {
            if id_ok(id) {
                s.push_str(&format!("{}={}\n", id, idx));
            }
        }
        s
    }

    pub fn from_text(text: &str) -> Overrides {
        let mut o = Overrides::default();
        for line in text.split('\n') {
            let line = line.trim_end_matches('\r');
            if let Some((id, n)) = line.split_once('=') {
                if !id_ok(id) {
                    continue;
                }
                if let Ok(idx) = n.trim().parse::<usize>() {
                    o.set(id, idx);
                }
            }
        }
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_clear() {
        let mut o = Overrides::default();
        o.set("a", 3);
        assert_eq!(o.get("a"), Some(3));
        o.set("a", 11);
        assert_eq!(o.get("a"), Some(11));
        o.clear("a");
        assert_eq!(o.get("a"), None);
        o.clear("missing");
    }

    #[test]
    fn idx_bounds_ignored() {
        let mut o = Overrides::default();
        o.set("a", 12);
        o.set("b", usize::MAX);
        assert_eq!(o.get("a"), None);
        assert_eq!(o.get("b"), None);
        o.set("c", 5);
        o.set("c", 12); // keeps old
        assert_eq!(o.get("c"), Some(5));
        o.set("z", 0);
        assert_eq!(o.get("z"), Some(0));
    }

    #[test]
    fn set_rejects_ids_that_cannot_persist() {
        let mut o = Overrides::default();
        o.set("", 1);
        o.set("a=b", 1);
        o.set("a\nb", 1);
        o.set("a\rb", 1);
        assert_eq!(o.get(""), None);
        assert_eq!(o.get("a=b"), None);
        assert_eq!(o.to_text(), "");
        assert_eq!(Overrides::from_text(&o.to_text()), o);
    }

    #[test]
    fn roundtrip() {
        let mut o = Overrides::default();
        o.set("x1", 0);
        o.set("y", 11);
        o.set("with space", 4);
        let t = o.to_text();
        assert_eq!(t, "with space=4\nx1=0\ny=11\n");
        assert_eq!(Overrides::from_text(&t), o);
    }

    #[test]
    fn bad_ids_skipped_in_text() {
        let mut o = Overrides::default();
        o.set("a=b", 1);
        o.set("a\nb", 2);
        o.set("a\rb", 2);
        o.set("", 3);
        o.set("ok", 4);
        assert_eq!(o.to_text(), "ok=4\n");
    }

    #[test]
    fn empty() {
        assert_eq!(Overrides::default().to_text(), "");
        assert_eq!(Overrides::from_text(""), Overrides::default());
        assert_eq!(Overrides::from_text("\n\r\n\n"), Overrides::default());
    }

    #[test]
    fn crlf() {
        let o = Overrides::from_text("a=1\r\nb=2\r\n");
        assert_eq!(o.get("a"), Some(1));
        assert_eq!(o.get("b"), Some(2));
    }

    #[test]
    fn corrupt() {
        let t = "junk\n=5\na=\na=x\na=-1\na=12\na=99999999999999999999999\nb=1.5\n\
                 c==3\nd=3=4\ne=7\n\u{0}\u{ffff}=2\nno newline=6";
        let o = Overrides::from_text(t);
        assert_eq!(o.get("a"), None);
        assert_eq!(o.get("b"), None);
        assert_eq!(o.get("c"), None);
        assert_eq!(o.get("d"), None);
        assert_eq!(o.get("e"), Some(7));
        assert_eq!(o.get("\u{0}\u{ffff}"), Some(2));
        assert_eq!(o.get("no newline"), Some(6));
        assert_eq!(o.get(""), None);
    }

    #[test]
    fn duplicates_last_valid_wins_and_invalid_keeps_old() {
        let o = Overrides::from_text("a=1\na=2\na=50\na=zz\n");
        assert_eq!(o.get("a"), Some(2));
    }

    #[test]
    fn idx_whitespace_tolerated() {
        let o = Overrides::from_text("a= 3 \n");
        assert_eq!(o.get("a"), Some(3));
    }

    #[test]
    fn huge_garbage_no_panic() {
        let t = "=\n".repeat(10000) + &"a=1\n".repeat(10000);
        let o = Overrides::from_text(&t);
        assert_eq!(o.get("a"), Some(1));
    }
}
