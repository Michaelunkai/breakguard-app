#![allow(dead_code)]
//! Log font size logic. std-only.

pub const MIN: f32 = 10.0;
pub const MAX: f32 = 22.0;
pub const DEFAULT: f32 = 15.0;

const KEY: &str = "log_font";

pub fn clamp(v: f32) -> f32 {
    if v.is_nan() {
        DEFAULT
    } else {
        v.clamp(MIN, MAX)
    }
}

pub fn bigger(cur: f32) -> f32 {
    clamp(clamp(cur) + 1.0)
}

pub fn smaller(cur: f32) -> f32 {
    clamp(clamp(cur) - 1.0)
}

pub fn to_ini(v: f32) -> String {
    format!("{}={}", KEY, clamp(v))
}

pub fn from_ini(text: &str) -> f32 {
    for line in text.lines() {
        let line = line.trim();
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == KEY {
                return v.trim().parse::<f32>().map(clamp).unwrap_or(DEFAULT);
            }
        }
    }
    DEFAULT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps() {
        assert_eq!(bigger(13.0), 14.0);
        assert_eq!(smaller(13.0), 12.0);
    }
    #[test]
    fn ends() {
        assert_eq!(bigger(MAX), MAX);
        assert_eq!(smaller(MIN), MIN);
        assert_eq!(bigger(21.5), MAX);
        assert_eq!(smaller(10.5), MIN);
    }
    #[test]
    fn out_of_range_inputs() {
        assert_eq!(bigger(1000.0), MAX);
        assert_eq!(smaller(-5.0), MIN);
        assert_eq!(bigger(f32::NEG_INFINITY), 11.0);
        assert_eq!(smaller(f32::INFINITY), 21.0);
    }
    #[test]
    fn nan() {
        assert_eq!(clamp(f32::NAN), DEFAULT);
        assert_eq!(bigger(f32::NAN), 16.0);
        assert_eq!(smaller(f32::NAN), 14.0);
    }
    #[test]
    fn clamp_basic() {
        assert_eq!(clamp(0.0), MIN);
        assert_eq!(clamp(99.0), MAX);
        assert_eq!(clamp(15.0), 15.0);
        assert_eq!(clamp(f32::INFINITY), MAX);
        assert_eq!(clamp(f32::NEG_INFINITY), MIN);
    }
    #[test]
    fn to_ini_fmt() {
        assert_eq!(to_ini(13.0), "log_font=13");
        assert_eq!(to_ini(13.5), "log_font=13.5");
        assert_eq!(to_ini(f32::NAN), "log_font=15");
        assert_eq!(to_ini(100.0), "log_font=22");
    }
    #[test]
    fn roundtrip() {
        let mut v = MIN;
        while v <= MAX {
            assert_eq!(from_ini(&to_ini(v)), v);
            v += 0.5;
        }
        assert_eq!(from_ini(&to_ini(f32::NAN)), DEFAULT);
    }
    #[test]
    fn crlf_and_multiline() {
        assert_eq!(from_ini("a=1\r\nlog_font=17\r\nb=2\r\n"), 17.0);
        assert_eq!(from_ini("  log_font = 18  \n"), 18.0);
        assert_eq!(from_ini("log_font=15\r"), 15.0);
    }
    #[test]
    fn missing_garbage() {
        assert_eq!(from_ini(""), DEFAULT);
        assert_eq!(from_ini("\n\r\n"), DEFAULT);
        assert_eq!(from_ini("other=20"), DEFAULT);
        assert_eq!(from_ini("log_font"), DEFAULT);
        assert_eq!(from_ini("log_font="), DEFAULT);
        assert_eq!(from_ini("log_font=abc"), DEFAULT);
        assert_eq!(from_ini("log_font=NaN"), DEFAULT);
        assert_eq!(from_ini("log_font=13px"), DEFAULT);
        assert_eq!(from_ini("xlog_font=20"), DEFAULT);
        assert_eq!(from_ini("\u{0}\u{ffff}=\u{1F600}"), DEFAULT);
    }
    #[test]
    fn out_of_range_ini() {
        assert_eq!(from_ini("log_font=3"), MIN);
        assert_eq!(from_ini("log_font=500"), MAX);
        assert_eq!(from_ini("log_font=-7"), MIN);
        assert_eq!(from_ini("log_font=inf"), MAX);
        assert_eq!(from_ini("log_font=1e99"), MAX);
    }
    #[test]
    fn first_key_wins() {
        assert_eq!(from_ini("log_font=11\nlog_font=20"), 11.0);
    }
}
