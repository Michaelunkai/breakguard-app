//! slice qa_breaker (owned by one team agent)
//! API: none (test-only module; adversarial #[cfg(test)] tests of teammates' pub fns)
#![allow(dead_code)]

// STD-BEGIN (std-only adversarial harness; run via rustc --test, see QA report)
#[cfg(test)]
#[path = "../logic/keys.rs"]
mod keys_logic;
#[cfg(test)]
#[path = "../logic/persist.rs"]
mod persist_logic;
#[cfg(test)]
#[path = "../logic/badge.rs"]
mod badge_logic;
#[cfg(test)]
#[path = "../logic/retain.rs"]
mod retain_logic;
#[cfg(test)]
#[path = "../logic/fade.rs"]
mod fade_logic;
#[cfg(test)]
#[path = "../logic/ease.rs"]
mod ease_logic;
#[cfg(test)]
#[path = "../logic/ring.rs"]
mod ring_logic;
#[cfg(test)]
#[path = "../logic/picker.rs"]
mod picker_logic;

#[cfg(test)]
mod std_logic {
    use super::{
        badge_logic::{label, Badges}, ease_logic::*, keys_logic::{Key, Nav, Out},
        persist_logic::LayoutSave, picker_logic::{Overrides, CREATURES}, retain_logic::lane_opacity,
        retain_logic::should_remove, ring_logic::{arc_points, fraction_left, mmss},
    };

    // --- ease: NaN / inf / extreme ---
    #[test]
    fn ease_nan_and_inf() {
        assert_eq!(ease_out_cubic(f32::NAN), 1.0);
        assert_eq!(ease_in_out_cubic(f32::NAN), 1.0);
        assert_eq!(spring_settle(f32::NAN), 1.0);
        assert_eq!(clamp_step_ms(f32::NAN), 0.0);
        assert_eq!(ease_out_cubic(f32::INFINITY), 1.0);
        assert_eq!(spring_settle(f32::NEG_INFINITY), 0.0);
        assert_eq!(clamp_step_ms(f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn transition_extremes() {
        let t = Transition { start_ms: i64::MIN, dur_ms: i64::MAX };
        let p = t.progress(i64::MAX);
        assert!(p.is_finite() && (0.0..=1.0).contains(&p));
        assert!(t.offset_px(i64::MAX, 10.0).is_finite());
        let z = Transition { start_ms: 0, dur_ms: i64::MIN };
        assert_eq!(z.progress(i64::MAX), 1.0);
        assert!(!z.is_running(0));
    }

    #[test]
    #[ignore = "DEFECT ease.rs:81 offset_px returns NaN for NaN distance (no guard)"]
    fn ease_offset_nan_distance_finite() {
        let t = Transition::new(0);
        assert!(t.offset_px(0, f32::NAN).is_finite());
    }

    // --- keys: empty/huge lanes, typing guard ---
    #[test]
    fn keys_huge_selected_typing_and_space() {
        let mut n = Nav { selected: usize::MAX, filter_focus: false };
        assert_eq!(n.handle(Key::Space, 3, true), Out::None);
        assert_eq!(n.selected, usize::MAX, "typing must not clamp");
        assert_eq!(n.handle(Key::Space, 3, false), Out::ToggleFreeze(2));
        assert_eq!(n.handle(Key::K, usize::MAX, false), Out::None);
        assert_eq!(n.selected, 1);
    }

    // --- badge: saturation and labels ---
    #[test]
    fn badge_saturation_and_unicode_ids() {
        let mut b = Badges::default();
        b.mark_seen("", u64::MAX);
        assert_eq!(b.unseen("", 0), 0);
        assert_eq!(b.nav_total(&[(String::new(), u64::MAX), ("x".into(), 1)]), 1);
        assert_eq!(b.nav_total(&[("x".into(), u64::MAX), ("y".into(), u64::MAX)]), u64::MAX);
        assert_eq!(label(9), "9");
        assert_eq!(label(10), "9+");
    }

    // --- retain: clock skew and fade edge ---
    #[test]
    fn retain_zero_and_negative_windows() {
        assert!(!should_remove(Some(0), 0, 0, 0) || lane_opacity(Some(0), 0, 0, 0) == 0.0);
        assert_eq!(lane_opacity(Some(i64::MAX), i64::MIN, 0, 0), 0.55_f32.min(0.55));
        assert_eq!(lane_opacity(Some(0), 1, 0, 1), 0.0);
    }

    #[test]
    #[ignore = "DEFECT retain.rs:23-24 f64 rounding: fade > 2^53 hits 0.0 before end, lane removed early"]
    fn retain_no_early_zero_with_huge_fade() {
        let o = lane_opacity(Some(0), i64::MAX - 1, 0, i64::MAX);
        assert!(o > 0.0, "opacity {o} should be > 0 before fade end");
        assert!(!should_remove(Some(0), i64::MAX - 1, 0, i64::MAX));
    }

    // --- persist: corrupt / extreme input ---
    #[test]
    fn persist_extreme_values_clamped() {
        let r = LayoutSave::from_text("h=a:1e39\nh=b:-1e39\ns=256\nf=\u{0}\n");
        assert_eq!(r.heights, vec![("a".to_string(), 800.0), ("b".to_string(), 66.0)]);
        assert_eq!(r.sort, 0);
        assert_eq!(r.filter, "\u{0}");
    }

    #[test]
    fn persist_crlf_filter_roundtrip() {
        let s = LayoutSave { filter: "x\r".into(), ..Default::default() };
        assert_eq!(LayoutSave::from_text(&s.to_text()), s);
    }

    #[test]
    fn persist_empty_ids_not_written() {
        let s = LayoutSave { collapsed: vec![String::new()], heights: vec![(String::new(), 100.0)], ..Default::default() };
        assert_eq!(s.to_text(), "f=\ns=0\n");
    }

    // --- picker: id validity, creature bounds ---
    #[test]
    fn picker_bounds_and_malformed() {
        let mut o = Overrides::default();
        o.set("a", CREATURES - 1);
        assert_eq!(o.get("a"), Some(CREATURES - 1));
        let o2 = Overrides::from_text("a=b=3\na=1\r\r\n");
        assert_eq!(o2.get("a"), Some(1));
        assert_eq!(Overrides::from_text("x=18446744073709551616").get("x"), None);
    }

    #[test]
    fn picker_set_rejects_unpersistable_id() {
        let mut o = Overrides::default();
        o.set("", 1);
        o.set("a=b", 1);
        assert_eq!(o.get(""), None);
        assert_eq!(o.get("a=b"), None);
    }

    // --- ring: degenerate geometry ---
    #[test]
    fn ring_fraction_and_mmss_extremes() {
        assert_eq!(fraction_left(1, i64::MAX), (1.0_f64 / i64::MAX as f64) as f32);
        assert_eq!(mmss(i64::MIN), "00:00");
        let p = arc_points((0.0, 0.0), 1.0, f32::MIN_POSITIVE, 3);
        assert_eq!(p.len(), 4);
    }

    #[test]
    fn ring_nan_radius_points_finite() {
        assert!(arc_points((0.0, 0.0), f32::NAN, 0.5, 4).is_empty());
    }

    #[test]
    fn ring_huge_segments_bounded() {
        assert!(arc_points((0.0, 0.0), 1.0, 0.5, usize::MAX).len() <= 1025);
    }
}
// STD-END

#[cfg(test)]
mod tests {
    use crate::ui::parts::{
        error_badge::badge_text, fade_in::row_anim, sparkline::sparkline_model,
        time_col::stamp, tokens_render::highlighted_job, tool_chips::chip_color,
    };
    use eframe::egui::{Color32, FontId};

    fn mono() -> FontId {
        FontId::monospace(12.0)
    }

    #[test]
    fn stamp_zero_and_negative_ts_never_panic_or_leak() {
        for mode in 0u8..=255 {
            for ts in [i64::MIN, -1, 0, 1] {
                let _ = stamp(mode, 0, ts);
                let _ = stamp(mode, i64::MIN, ts);
                let _ = stamp(mode, i64::MAX, ts);
            }
        }
    }

    #[test]
    fn stamp_extreme_now_and_ts() {
        assert!(!stamp(1, i64::MIN, i64::MAX).is_empty());
        assert!(!stamp(1, i64::MAX, i64::MAX).is_empty());
        assert_eq!(stamp(2, i64::MIN, i64::MAX).len(), 8);
    }

    #[test]
    fn sparkline_empty_and_zero_now() {
        let (h, r) = sparkline_model(&[], 0);
        assert_eq!(h.len(), 30);
        assert!(h.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        assert!(r.is_finite() && r >= 0.0);
    }

    #[test]
    fn sparkline_huge_counts_and_extreme_times() {
        let many: Vec<i64> = (0..100_000).map(|i| i * 1_000).collect();
        let (h, r) = sparkline_model(&many, 100_000_000);
        assert_eq!(h.len(), 30);
        assert!(h.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        assert!(r.is_finite());
        let edge = [i64::MIN, i64::MAX, 0, -1];
        let (h2, r2) = sparkline_model(&edge, i64::MAX);
        assert_eq!(h2.len(), 30);
        assert!(h2.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        assert!(r2.is_finite());
        let (h3, _) = sparkline_model(&edge, i64::MIN);
        assert!(h3.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
    }

    #[test]
    fn badge_text_boundaries() {
        assert_eq!(badge_text(0), "");
        assert_eq!(badge_text(9), "9");
        assert_eq!(badge_text(10), "9+");
        assert_eq!(badge_text(u32::MAX), "9+");
    }

    #[test]
    fn highlighted_job_empty_text_and_query() {
        let j = highlighted_job("", Color32::WHITE, mono(), "");
        assert!(j.text.is_empty());
        let j = highlighted_job("", Color32::WHITE, mono(), "error");
        assert!(j.text.is_empty());
    }

    #[test]
    fn highlighted_job_sections_cover_text_exactly() {
        let samples = [
            "",
            "a",
            "héllo wörld 🦀 /usr/bin/x 42 error `code`",
            "\u{202E}rtl \u{200B}zero-width",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ];
        for text in samples {
            for query in ["", "a", "é", "🦀", "zzz", "error error", text] {
                let j = highlighted_job(text, Color32::WHITE, mono(), query);
                assert_eq!(j.text, text);
                let mut pos = 0usize;
                for s in &j.sections {
                    assert_eq!(s.byte_range.start, pos, "gap/overlap in {text:?} q={query:?}");
                    assert!(s.byte_range.end >= s.byte_range.start);
                    pos = s.byte_range.end;
                }
                assert_eq!(pos, text.len(), "sections do not cover {text:?} q={query:?}");
            }
        }
    }

    #[test]
    fn row_anim_extreme_ages_stay_in_range() {
        for age in [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX] {
            let (a, dy) = row_anim(age, false);
            assert!(a.is_finite() && (0.0..=1.0).contains(&a), "alpha {a} at {age}");
            assert!(dy.is_finite() && (0.0..=8.0).contains(&dy), "dy {dy} at {age}");
        }
    }

    #[test]
    fn lane_wrap_row_degenerate_widths_and_hebrew() {
        use crate::ui::parts::lane_wrap_row::{row_height, truncated, wrap_lines};
        let heb = "שלום עולם זה טקסט ארוך בעברית לבדיקה";
        for width in [0usize, 1, 2, usize::MAX] {
            for text in ["", " ", "a", heb, &"x".repeat(5_000)] {
                let lines = wrap_lines(text, width);
                assert!(!lines.is_empty() || text.is_empty(), "no lines for {text:?} w={width}");
                for l in &lines {
                    assert!(std::str::from_utf8(l.as_bytes()).is_ok());
                }
                let t = truncated(text, width);
                assert!(t.is_char_boundary(t.len()));
                for wrap in [false, true] {
                    for expanded in [false, true] {
                        let h = row_height(text, width, wrap, expanded, 14.0);
                        assert!(h.is_finite() && h >= 0.0, "h={h} text={text:?} w={width}");
                    }
                }
            }
        }
        let joined: String = wrap_lines(heb, 3).concat();
        assert_eq!(joined.chars().filter(|c| !c.is_whitespace()).count(),
                   heb.chars().filter(|c| !c.is_whitespace()).count());
    }

    #[test]
    fn activity_text_and_creature_fx_extremes() {
        use crate::ui::parts::creature_state_fx::{breathe_scale, busy_bounce};
        use crate::ui::parts::lane_header_stats::{counters_text, last_activity_text};
        for (now, last) in [(i64::MIN, i64::MAX), (i64::MAX, i64::MIN), (0, 0), (5, 5), (i64::MAX, 1)] {
            let _ = last_activity_text(now, last);
        }
        assert_eq!(last_activity_text(1_000_000, 0), "idle");
        assert_eq!(last_activity_text(1_000_000, -5), "idle");
        for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1e300, 1e300, 0.0] {
            let s = breathe_scale(t);
            assert!(s.is_finite() && (1.0..=1.04).contains(&s), "breathe {s} at {t}");
            let b = busy_bounce(t);
            assert!(b.is_finite() && (-2.0..=2.0).contains(&b), "bounce {b} at {t}");
        }
        assert!(!counters_text(u64::MAX, u64::MAX, u64::MAX).is_empty());
        assert!(!counters_text(0, 0, 0).is_empty());
    }

    #[test]
    fn chip_color_unknown_and_degenerate_names() {
        let fallback = chip_color("definitely-not-a-tool");
        assert_eq!(chip_color(""), fallback);
        assert_eq!(chip_color("bash"), fallback, "case-sensitive lookup expected");
        let long = "x".repeat(1_000_000);
        assert_eq!(chip_color(&long), fallback);
        assert_ne!(chip_color("Bash"), fallback);
    }
}
