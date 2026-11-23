//! Log page tab membership. Live = sessions that are still alive (running or waiting);
//! Finished = process gone, only while it ended within the last FINISHED_KEEP_MS. std-only.

/// Finished sessions are kept this long (3 hours), then dropped from the Finished tab.
pub const FINISHED_KEEP_MS: i64 = 3 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bucket {
    Live,
    Finished,
    /// Finished too long ago: shown nowhere.
    Gone,
}

/// `working` = the session is really working now (see rightnow::is_working);
/// `finished_at_ms` = when it stopped (process gone, or its last line). Not working = Finished
/// in the very same frame (no fade, no grace), until it is older than `keep_ms`.
pub fn bucket(working: bool, finished_at_ms: Option<i64>, now_ms: i64, keep_ms: i64) -> Bucket {
    if working {
        return Bucket::Live;
    }
    match finished_at_ms {
        Some(t) if now_ms.saturating_sub(t) > keep_ms.max(0) => Bucket::Gone,
        _ => Bucket::Finished,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const T: i64 = 1_000_000;

    #[test] fn alive_is_live() { assert_eq!(bucket(true, None, T, FINISHED_KEEP_MS), Bucket::Live); }
    #[test] fn alive_ignores_stale_finish_mark() { assert_eq!(bucket(true, Some(0), i64::MAX, FINISHED_KEEP_MS), Bucket::Live); }
    #[test] fn dead_moves_to_finished_in_same_ms() {
        assert_eq!(bucket(true, None, T, FINISHED_KEEP_MS), Bucket::Live);
        assert_eq!(bucket(false, Some(T), T, FINISHED_KEEP_MS), Bucket::Finished);
    }
    #[test] fn recent_finished_shown() { assert_eq!(bucket(false, Some(T), T + 2 * 3_600_000, FINISHED_KEEP_MS), Bucket::Finished); }
    #[test] fn boundary_exact_keep_still_shown() { assert_eq!(bucket(false, Some(T), T + FINISHED_KEEP_MS, FINISHED_KEEP_MS), Bucket::Finished); }
    #[test] fn old_finished_hidden() {
        assert_eq!(bucket(false, Some(T), T + FINISHED_KEEP_MS + 1, FINISHED_KEEP_MS), Bucket::Gone);
        assert_eq!(bucket(false, Some(T), T + 5 * 3_600_000, FINISHED_KEEP_MS), Bucket::Gone);
    }
    #[test] fn unknown_finish_time_counts_as_just_finished() { assert_eq!(bucket(false, None, T, FINISHED_KEEP_MS), Bucket::Finished); }
    #[test] fn clock_backwards_is_finished() { assert_eq!(bucket(false, Some(T), T - 10, FINISHED_KEEP_MS), Bucket::Finished); }
    #[test] fn extreme_clock_no_overflow() { assert_eq!(bucket(false, Some(i64::MIN), i64::MAX, FINISHED_KEEP_MS), Bucket::Gone); }
}
