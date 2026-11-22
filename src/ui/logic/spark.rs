#![allow(dead_code)]
//! Sparkline helpers. std-only.

/// Counts per bucket for the last `n` buckets, oldest first.
/// Line with age = now - t in [0, n*bucket_ms) lands in bucket n-1 - age/bucket_ms.
/// Future lines (t > now) and older lines are ignored.
pub fn buckets(line_times_ms: &[i64], now_ms: i64, n: usize, bucket_ms: i64) -> Vec<u32> {
    if n == 0 || bucket_ms <= 0 {
        return Vec::new();
    }
    let mut out = vec![0u32; n];
    let span = n as i128 * bucket_ms as i128;
    for &t in line_times_ms {
        let age = now_ms as i128 - t as i128;
        if age < 0 || age >= span {
            continue;
        }
        let idx = n - 1 - (age / bucket_ms as i128) as usize;
        out[idx] = out[idx].saturating_add(1);
    }
    out
}

/// Average events per minute over the whole window.
pub fn per_minute(b: &[u32], bucket_ms: i64) -> f32 {
    if b.is_empty() || bucket_ms <= 0 {
        return 0.0;
    }
    let total: u64 = b.iter().map(|&x| x as u64).sum();
    let window_ms = b.len() as f64 * bucket_ms as f64;
    (total as f64 * 60_000.0 / window_ms) as f32
}

/// Scale to 0..1 by max. All zero or empty -> zeros.
pub fn normalize(b: &[u32]) -> Vec<f32> {
    let max = b.iter().copied().max().unwrap_or(0);
    if max == 0 {
        return vec![0.0; b.len()];
    }
    b.iter().map(|&x| x as f32 / max as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degenerate() {
        assert!(buckets(&[1], 10, 0, 5).is_empty());
        assert!(buckets(&[1], 10, 3, 0).is_empty());
        assert!(buckets(&[1], 10, 3, -5).is_empty());
        assert_eq!(buckets(&[], 100, 3, 10), vec![0, 0, 0]);
    }

    #[test]
    fn boundaries() {
        // now=100, bucket 10, n=3: buckets cover (70,80],(80,90],(90,100] => ages 20..29,10..19,0..9
        let t = [100, 91, 90, 81, 80, 71, 70, 101];
        assert_eq!(buckets(&t, 100, 3, 10), vec![2, 2, 2]);
        // 91..100 newest has 100,91; 90,81 middle; 80,71 oldest; 70 and 101 out
    }

    #[test]
    fn unsorted_and_dupes() {
        let t = [95, 5, 99, 95, 85, 200, 75, -50];
        assert_eq!(buckets(&t, 100, 3, 10), vec![1, 1, 3]);
        let mut s = t.to_vec();
        s.sort();
        assert_eq!(buckets(&s, 100, 3, 10), vec![1, 1, 3]);
    }

    #[test]
    fn extremes_no_overflow() {
        let t = [i64::MIN, i64::MAX, 0];
        assert_eq!(buckets(&t, i64::MAX, 2, i64::MAX), vec![1, 1]);
        assert_eq!(buckets(&[i64::MIN], i64::MAX, 1, 1), vec![0]);
        assert_eq!(buckets(&[5], i64::MIN, 1, 1), vec![0]);
        assert_eq!(buckets(&[5], 5, 4, 1), vec![0, 0, 0, 1]);
    }

    #[test]
    fn per_minute_cases() {
        assert_eq!(per_minute(&[], 1000), 0.0);
        assert_eq!(per_minute(&[1], 0), 0.0);
        assert_eq!(per_minute(&[1], -1), 0.0);
        assert_eq!(per_minute(&[0, 0], 1000), 0.0);
        assert_eq!(per_minute(&[1, 1, 1, 1], 15_000), 4.0);
        assert_eq!(per_minute(&[5], 60_000), 5.0);
        assert!((per_minute(&[u32::MAX; 4], 1) - 6.0e4 * u32::MAX as f32).abs() < 1e9);
        assert!(per_minute(&[u32::MAX], i64::MAX).is_finite());
    }

    #[test]
    fn normalize_cases() {
        assert!(normalize(&[]).is_empty());
        assert_eq!(normalize(&[0, 0, 0]), vec![0.0, 0.0, 0.0]);
        assert_eq!(normalize(&[1, 2, 4]), vec![0.25, 0.5, 1.0]);
        assert_eq!(normalize(&[7]), vec![1.0]);
        let v = normalize(&[u32::MAX, 0, u32::MAX / 2]);
        assert!(v.iter().all(|x| (0.0..=1.0).contains(x)));
        assert_eq!(v[0], 1.0);
    }
}
