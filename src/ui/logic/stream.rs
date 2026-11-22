#![allow(dead_code)]
//! Log perf budget bench: ring buffer + per-frame row iteration simulation.
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub const CAP: usize = 2000;
pub const VISIBLE_ROWS: usize = 60;

pub struct Ring {
    buf: VecDeque<String>,
    cap: usize,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        Ring { buf: VecDeque::with_capacity(cap), cap }
    }
    pub fn push(&mut self, s: String) {
        if self.cap == 0 {
            return;
        }
        if self.buf.len() == self.cap {
            self.buf.pop_front();
        }
        self.buf.push_back(s);
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    /// Last `n` rows, oldest first.
    pub fn tail(&self, n: usize) -> impl Iterator<Item = &String> {
        self.buf.iter().skip(self.buf.len().saturating_sub(n))
    }
}

pub fn make_line(session: usize, i: usize) -> String {
    let s = format!("[s{:03}] {:06} INFO lane event payload=abcdefghijklmnopqrstuvwxyz0123456789 ok", session, i);
    s
}

/// One frame: iterate last 60 rows of each lane; returns checksum (anti-elision).
pub fn frame(rings: &[Ring]) -> usize {
    let mut sum = 0usize;
    for r in rings {
        for row in r.tail(VISIBLE_ROWS) {
            sum = sum.wrapping_add(row.len()).wrapping_add(row.as_bytes()[0] as usize);
        }
    }
    sum
}

pub fn simulate(sessions: usize, lines_per_sec: usize, seconds: usize) -> Duration {
    let mut rings: Vec<Ring> = (0..sessions).map(|_| Ring::new(CAP)).collect();
    let mut sink = 0usize;
    let t = Instant::now();
    for sec in 0..seconds {
        for (si, r) in rings.iter_mut().enumerate() {
            for l in 0..lines_per_sec {
                r.push(make_line(si, sec * lines_per_sec + l));
            }
        }
        sink = sink.wrapping_add(frame(&rings));
    }
    let d = t.elapsed();
    std::hint::black_box(sink);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_about_80_chars() {
        let l = make_line(1, 1);
        assert!((75..=90).contains(&l.len()), "len {}", l.len());
    }

    #[test]
    fn ring_caps_and_evicts_oldest() {
        let mut r = Ring::new(3);
        for i in 0..10 {
            r.push(i.to_string());
        }
        assert_eq!(r.len(), 3);
        let v: Vec<_> = r.tail(10).cloned().collect();
        assert_eq!(v, ["7", "8", "9"]);
    }

    #[test]
    fn adversarial_zero_cap_and_empty() {
        let mut r = Ring::new(0);
        r.push("x".into());
        assert!(r.is_empty());
        assert_eq!(r.tail(5).count(), 0);
        assert_eq!(frame(&[Ring::new(5)]), 0);
        assert_eq!(frame(&[]), 0);
    }

    #[test]
    fn tail_fewer_than_n_and_zero() {
        let mut r = Ring::new(10);
        r.push("a".into());
        r.push("b".into());
        assert_eq!(r.tail(60).count(), 2);
        assert_eq!(r.tail(0).count(), 0);
    }

    #[test]
    fn simulate_degenerate_inputs() {
        let _ = simulate(0, 100, 10);
        let _ = simulate(5, 0, 10);
        let _ = simulate(5, 100, 0);
    }

    #[test]
    fn simulate_overflow_burst_stays_capped() {
        let mut r = Ring::new(CAP);
        for i in 0..CAP * 5 + 7 {
            r.push(make_line(0, i));
        }
        assert_eq!(r.len(), CAP);
    }

    #[test]
    fn budget_total_cpu_under_1000ms() {
        let d = simulate(20, 100, 10);
        println!("simulate(20 sessions x 100 lps x 10 s) = {:.3} ms", d.as_secs_f64() * 1e3);
        assert!(d < Duration::from_millis(1000), "{:?}", d);
    }

    #[test]
    fn budget_frame_under_16ms() {
        let mut rings: Vec<Ring> = (0..20).map(|_| Ring::new(CAP)).collect();
        for (si, r) in rings.iter_mut().enumerate() {
            for i in 0..CAP + 500 {
                r.push(make_line(si, i));
            }
        }
        let t = Instant::now();
        let s = frame(&rings);
        let d = t.elapsed();
        std::hint::black_box(s);
        println!("one frame 20 lanes x 60 rows = {:.4} ms", d.as_secs_f64() * 1e3);
        assert!(s > 0);
        assert!(d < Duration::from_millis(16), "{:?}", d);
    }
}
