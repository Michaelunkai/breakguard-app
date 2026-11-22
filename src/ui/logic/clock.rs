#![allow(dead_code)]
//! Clock: relative and absolute time formatting. std only.

pub fn relative(now_ms: i64, then_ms: i64) -> String {
    if then_ms <= 0 {
        return "-".to_string();
    }
    let diff = now_ms.saturating_sub(then_ms);
    if diff < 5_000 {
        "just now".to_string()
    } else if diff < 60_000 {
        format!("{}s ago", diff / 1_000)
    } else if diff < 3_600_000 {
        format!("{} min ago", diff / 60_000)
    } else if diff < 86_400_000 {
        format!("{} h ago", diff / 3_600_000)
    } else {
        format!("{} d ago", diff / 86_400_000)
    }
}

pub fn absolute(ms: i64) -> String {
    if ms < 0 {
        return "--:--:--".to_string();
    }
    let s = (ms / 1_000) % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3_600, (s % 3_600) / 60, s % 60)
}

#[allow(dead_code)]
fn main() {}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: i64 = 10_000_000_000;

    fn r(d: i64) -> String {
        relative(NOW, NOW - d)
    }

    #[test]
    fn then_nonpositive_dash() {
        assert_eq!(relative(NOW, 0), "-");
        assert_eq!(relative(NOW, -1), "-");
        assert_eq!(relative(NOW, i64::MIN), "-");
        assert_eq!(relative(0, 0), "-");
    }
    #[test]
    fn just_now_and_future() {
        assert_eq!(r(0), "just now");
        assert_eq!(r(4999), "just now");
        assert_eq!(r(-1), "just now");
        assert_eq!(r(-1_000_000), "just now");
        assert_eq!(relative(i64::MIN, 1), "just now");
    }
    #[test]
    fn seconds() {
        assert_eq!(r(5000), "5s ago");
        assert_eq!(r(5999), "5s ago");
        assert_eq!(r(59_999), "59s ago");
    }
    #[test]
    fn minutes() {
        assert_eq!(r(60_000), "1 min ago");
        assert_eq!(r(119_999), "1 min ago");
        assert_eq!(r(180_000), "3 min ago");
        assert_eq!(r(3_599_999), "59 min ago");
    }
    #[test]
    fn hours() {
        assert_eq!(r(3_600_000), "1 h ago");
        assert_eq!(r(7_200_000), "2 h ago");
        assert_eq!(r(86_399_999), "23 h ago");
    }
    #[test]
    fn days() {
        assert_eq!(r(86_400_000), "1 d ago");
        assert_eq!(r(3 * 86_400_000), "3 d ago");
    }
    #[test]
    fn overflow_no_panic() {
        assert_eq!(relative(i64::MAX, 1), "106751991167 d ago");
        assert_eq!(relative(i64::MIN, i64::MAX), "just now");
        assert_eq!(relative(i64::MAX, i64::MAX), "just now");
    }
    #[test]
    fn absolute_basic() {
        assert_eq!(absolute(0), "00:00:00");
        assert_eq!(absolute(999), "00:00:00");
        assert_eq!(absolute(1000), "00:00:01");
        assert_eq!(absolute(3_661_000), "01:01:01");
        assert_eq!(absolute(86_399_999), "23:59:59");
        assert_eq!(absolute(86_400_000), "00:00:00");
        assert_eq!(absolute(1_700_000_000_000), "22:13:20");
    }
    #[test]
    fn absolute_negative_and_extreme() {
        assert_eq!(absolute(-1), "--:--:--");
        assert_eq!(absolute(i64::MIN), "--:--:--");
        assert_eq!(absolute(i64::MAX).len(), 8);
    }
}
