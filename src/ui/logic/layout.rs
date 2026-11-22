#![allow(dead_code)]
use std::collections::HashSet;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct LaneLayout {
    pub collapsed: HashSet<String>,
    pub solo: Option<String>,
}

impl LaneLayout {
    pub fn toggle_collapse(&mut self, id: &str) {
        if !self.collapsed.remove(id) {
            self.collapsed.insert(id.to_string());
        }
    }

    pub fn toggle_solo(&mut self, id: &str) {
        if self.solo.as_deref() == Some(id) {
            self.solo = None;
        } else {
            self.solo = Some(id.to_string());
        }
    }

    pub fn is_collapsed(&self, id: &str) -> bool {
        self.collapsed.contains(id)
    }

    pub fn visible(&self, ids: &[String]) -> Vec<String> {
        if let Some(s) = &self.solo {
            if ids.iter().any(|i| i == s) {
                return vec![s.clone()];
            }
        }
        ids.to_vec()
    }

    pub fn prune(&mut self, live: &[String]) {
        let set: HashSet<&str> = live.iter().map(|s| s.as_str()).collect();
        self.collapsed.retain(|c| set.contains(c.as_str()));
        if let Some(s) = &self.solo {
            if !set.contains(s.as_str()) {
                self.solo = None;
            }
        }
    }
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;
    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn default_empty() {
        let l = LaneLayout::default();
        assert!(!l.is_collapsed("a"));
        assert_eq!(l.visible(&v(&["a", "b"])), v(&["a", "b"]));
    }
    #[test]
    fn double_collapse_toggle() {
        let mut l = LaneLayout::default();
        l.toggle_collapse("a");
        assert!(l.is_collapsed("a"));
        l.toggle_collapse("a");
        assert!(!l.is_collapsed("a"));
        assert!(l.collapsed.is_empty());
    }
    #[test]
    fn double_solo_toggle() {
        let mut l = LaneLayout::default();
        l.toggle_solo("a");
        assert_eq!(l.solo.as_deref(), Some("a"));
        l.toggle_solo("a");
        assert_eq!(l.solo, None);
    }
    #[test]
    fn solo_switch() {
        let mut l = LaneLayout::default();
        l.toggle_solo("a");
        l.toggle_solo("b");
        assert_eq!(l.solo.as_deref(), Some("b"));
    }
    #[test]
    fn solo_shows_only() {
        let mut l = LaneLayout::default();
        l.toggle_solo("b");
        assert_eq!(l.visible(&v(&["a", "b", "c"])), v(&["b"]));
    }
    #[test]
    fn solo_missing_shows_all() {
        let mut l = LaneLayout::default();
        l.toggle_solo("zzz");
        assert_eq!(l.visible(&v(&["a", "b"])), v(&["a", "b"]));
        assert_eq!(l.visible(&[]), Vec::<String>::new());
    }
    #[test]
    fn solo_duplicate_ids_single() {
        let mut l = LaneLayout::default();
        l.toggle_solo("a");
        assert_eq!(l.visible(&v(&["a", "a"])), v(&["a"]));
    }
    #[test]
    fn collapsed_still_visible() {
        let mut l = LaneLayout::default();
        l.toggle_collapse("a");
        assert_eq!(l.visible(&v(&["a", "b"])), v(&["a", "b"]));
    }
    #[test]
    fn prune_drops_vanished() {
        let mut l = LaneLayout::default();
        l.toggle_collapse("a");
        l.toggle_collapse("b");
        l.toggle_solo("b");
        l.prune(&v(&["a"]));
        assert!(l.is_collapsed("a"));
        assert!(!l.is_collapsed("b"));
        assert_eq!(l.solo, None);
    }
    #[test]
    fn prune_keeps_live_solo() {
        let mut l = LaneLayout::default();
        l.toggle_solo("a");
        l.prune(&v(&["a", "b"]));
        assert_eq!(l.solo.as_deref(), Some("a"));
    }
    #[test]
    fn prune_empty_live_clears_all() {
        let mut l = LaneLayout::default();
        l.toggle_collapse("a");
        l.toggle_solo("a");
        l.prune(&[]);
        assert_eq!(l, LaneLayout::default());
    }
    #[test]
    fn empty_and_unicode_ids() {
        let mut l = LaneLayout::default();
        l.toggle_collapse("");
        l.toggle_collapse("ß🦀");
        assert!(l.is_collapsed("") && l.is_collapsed("ß🦀"));
        assert!(!l.is_collapsed("ss"));
        l.toggle_solo("");
        assert_eq!(l.visible(&v(&["", "x"])), v(&[""]));
    }
    #[test]
    fn case_sensitive() {
        let mut l = LaneLayout::default();
        l.toggle_collapse("A");
        assert!(!l.is_collapsed("a"));
    }
}
