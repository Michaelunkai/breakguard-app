//! stats: per-day break counters, streak, persistence (tolerant text file). Pure, no Win32.
#![allow(dead_code)]
use std::path::PathBuf;

/// Days of history kept on disk.
pub const KEEP_DAYS: usize = 60;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DayStats {
    /// Local day number (days since 1970-01-01, local midnight based).
    pub day: u32,
    pub taken: u32,
    pub skipped: u32,
    pub snoozed: u32,
    pub rest_secs: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    days: Vec<DayStats>,
    dirty: bool,
}

impl Stats {
    pub fn path() -> PathBuf {
        crate::engine::data_dir().join("stats.ini")
    }

    pub fn load() -> Stats {
        let Ok(b) = std::fs::read(Self::path()) else { return Stats::default() };
        if b.len() > 256 * 1024 {
            return Stats::default();
        }
        Stats::parse(&String::from_utf8_lossy(&b))
    }

    /// Atomic write; clears the dirty flag on success.
    pub fn save(&mut self) -> bool {
        let path = Self::path();
        if let Some(p) = path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let tmp = path.with_extension("ini.tmp");
        if std::fs::write(&tmp, self.serialize()).is_err() {
            return false;
        }
        if std::fs::rename(&tmp, &path).is_ok() {
            self.dirty = false;
            return true;
        }
        let _ = std::fs::remove_file(&tmp);
        false
    }

    pub fn serialize(&self) -> String {
        let mut o = String::new();
        for d in &self.days {
            o.push_str(&format!("d={},{},{},{},{}\n", d.day, d.taken, d.skipped, d.snoozed, d.rest_secs));
        }
        o
    }

    /// Tolerant: bad lines ignored, duplicates merged, sorted, capped.
    pub fn parse(text: &str) -> Stats {
        let mut s = Stats::default();
        for line in text.lines() {
            let Some(v) = line.trim().trim_start_matches('\u{feff}').strip_prefix("d=") else { continue };
            let p: Vec<u32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if p.len() != 5 || p[0] == 0 {
                continue;
            }
            let e = s.day_mut(p[0]);
            e.taken = e.taken.saturating_add(p[1].min(10_000));
            e.skipped = e.skipped.saturating_add(p[2].min(10_000));
            e.snoozed = e.snoozed.saturating_add(p[3].min(10_000));
            e.rest_secs = e.rest_secs.saturating_add(p[4].min(86_400));
        }
        s.days.sort_by_key(|d| d.day);
        let n = s.days.len();
        if n > KEEP_DAYS {
            s.days.drain(0..n - KEEP_DAYS);
        }
        s.dirty = false;
        s
    }

    fn day_mut(&mut self, day: u32) -> &mut DayStats {
        if let Some(i) = self.days.iter().position(|d| d.day == day) {
            return &mut self.days[i];
        }
        self.days.push(DayStats { day, ..Default::default() });
        self.days.sort_by_key(|d| d.day);
        let n = self.days.len();
        if n > KEEP_DAYS {
            self.days.drain(0..n - KEEP_DAYS);
        }
        let i = self.days.iter().position(|d| d.day == day).unwrap_or(0);
        &mut self.days[i]
    }

    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn record_taken(&mut self, day: u32, rest_secs: u32) {
        let d = self.day_mut(day);
        d.taken += 1;
        d.rest_secs = d.rest_secs.saturating_add(rest_secs);
        self.dirty = true;
    }
    pub fn record_skipped(&mut self, day: u32) {
        self.day_mut(day).skipped += 1;
        self.dirty = true;
    }
    pub fn record_snoozed(&mut self, day: u32) {
        self.day_mut(day).snoozed += 1;
        self.dirty = true;
    }

    pub fn clear(&mut self) {
        self.days.clear();
        self.dirty = true;
    }

    pub fn day(&self, day: u32) -> DayStats {
        self.days.iter().find(|d| d.day == day).copied().unwrap_or(DayStats { day, ..Default::default() })
    }

    /// Last `n` days ending at `today`, oldest first (missing days are zero).
    pub fn last_days(&self, today: u32, n: u32) -> Vec<DayStats> {
        (0..n).map(|i| self.day(today.saturating_sub(n - 1 - i))).collect()
    }

    /// Consecutive days with at least `min_taken` breaks, counting back from today
    /// (today may still be empty without breaking the streak).
    pub fn streak(&self, today: u32, min_taken: u32) -> u32 {
        let min_taken = min_taken.max(1);
        let mut n = 0;
        let mut d = today;
        if self.day(d).taken < min_taken {
            if d == 0 {
                return 0;
            }
            d -= 1;
        }
        while d > 0 && self.day(d).taken >= min_taken {
            n += 1;
            d -= 1;
        }
        n
    }

    pub fn all_days(&self) -> &[DayStats] {
        &self.days
    }

    pub fn total_taken(&self) -> u32 {
        self.days.iter().map(|d| d.taken).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_roundtrip() {
        let mut s = Stats::default();
        s.record_taken(100, 20);
        s.record_taken(100, 30);
        s.record_skipped(100);
        s.record_snoozed(101);
        assert!(s.is_dirty());
        assert_eq!(s.day(100), DayStats { day: 100, taken: 2, skipped: 1, snoozed: 0, rest_secs: 50 });
        let p = Stats::parse(&s.serialize());
        assert_eq!(p.day(100), s.day(100));
        assert_eq!(p.day(101).snoozed, 1);
        assert!(!p.is_dirty());
    }

    #[test]
    fn corrupt_lines_ignored() {
        let s = Stats::parse("\u{0}junk\nd=1,2\nd=x,1,1,1,1\nd=0,1,1,1,1\nd=5,1,0,0,9\n");
        assert_eq!(s.total_taken(), 1);
        assert_eq!(s.day(5).rest_secs, 9);
        assert_eq!(Stats::parse(""), Stats::default());
    }

    #[test]
    fn duplicate_days_merge_and_cap() {
        let s = Stats::parse("d=7,1,0,0,5\nd=7,2,0,0,5\n");
        assert_eq!(s.day(7).taken, 3);
        let mut txt = String::new();
        for i in 1..=100 {
            txt.push_str(&format!("d={i},1,0,0,1\n"));
        }
        let s = Stats::parse(&txt);
        assert_eq!(s.days.len(), KEEP_DAYS);
        assert_eq!(s.day(100).taken, 1);
        assert_eq!(s.day(1).taken, 0);
    }

    #[test]
    fn streak_rules() {
        let mut s = Stats::default();
        for d in [8, 9, 10] {
            s.record_taken(d, 20);
        }
        assert_eq!(s.streak(10, 1), 3);
        assert_eq!(s.streak(11, 1), 3, "empty today keeps streak");
        assert_eq!(s.streak(12, 1), 0, "a missed full day breaks it");
        assert_eq!(s.streak(10, 2), 0);
        s.record_taken(10, 20);
        assert_eq!(s.streak(10, 2), 1);
    }

    #[test]
    fn last_days_pads_zero() {
        let mut s = Stats::default();
        s.record_taken(10, 5);
        let v = s.last_days(10, 3);
        assert_eq!(v.iter().map(|d| d.taken).collect::<Vec<_>>(), vec![0, 0, 1]);
        assert_eq!(v[0].day, 8);
    }

    #[test]
    fn clear_wipes() {
        let mut s = Stats::default();
        s.record_taken(3, 1);
        s.clear();
        assert_eq!(s.total_taken(), 0);
        assert!(s.is_dirty());
    }
}
