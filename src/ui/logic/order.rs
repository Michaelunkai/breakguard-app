#![allow(dead_code)]
use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Running,
    Idle,
    Finished,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SortMode {
    RunningFirst,
    Name,
    Newest,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Row {
    pub id: String,
    pub name: String,
    pub state: State,
    pub last_ms: i64,
}

fn rank(s: State) -> u8 {
    match s {
        State::Running => 0,
        State::Idle => 1,
        State::Finished => 2,
    }
}

fn newest(a: &Row, b: &Row) -> Ordering {
    b.last_ms.cmp(&a.last_ms).then_with(|| a.id.cmp(&b.id))
}

pub fn sort_rows(rows: &mut Vec<Row>, mode: SortMode) {
    match mode {
        SortMode::RunningFirst => {
            rows.sort_by(|a, b| rank(a.state).cmp(&rank(b.state)).then_with(|| newest(a, b)))
        }
        SortMode::Name => rows.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        }),
        SortMode::Newest => rows.sort_by(newest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(id: &str, name: &str, state: State, last_ms: i64) -> Row {
        Row { id: id.into(), name: name.into(), state, last_ms }
    }
    fn ids(v: &[Row]) -> Vec<&str> {
        v.iter().map(|x| x.id.as_str()).collect()
    }

    #[test]
    fn running_first_groups_and_recency() {
        let mut v = vec![
            r("a", "x", State::Finished, 99),
            r("b", "x", State::Idle, 5),
            r("c", "x", State::Running, 1),
            r("d", "x", State::Running, 10),
            r("e", "x", State::Idle, 6),
        ];
        sort_rows(&mut v, SortMode::RunningFirst);
        assert_eq!(ids(&v), ["d", "c", "e", "b", "a"]);
    }

    #[test]
    fn running_first_ties_by_id() {
        let mut v = vec![
            r("z", "x", State::Idle, 5),
            r("a", "x", State::Idle, 5),
            r("m", "x", State::Idle, 5),
        ];
        sort_rows(&mut v, SortMode::RunningFirst);
        assert_eq!(ids(&v), ["a", "m", "z"]);
    }

    #[test]
    fn name_case_insensitive_then_id() {
        let mut v = vec![
            r("3", "bravo", State::Idle, 0),
            r("2", "Alpha", State::Idle, 0),
            r("1", "alpha", State::Idle, 0),
            r("4", "ALPHA", State::Idle, 0),
        ];
        sort_rows(&mut v, SortMode::Name);
        assert_eq!(ids(&v), ["1", "2", "4", "3"]);
    }

    #[test]
    fn name_ignores_state_and_time() {
        let mut v = vec![
            r("1", "b", State::Running, 100),
            r("2", "a", State::Finished, 0),
        ];
        sort_rows(&mut v, SortMode::Name);
        assert_eq!(ids(&v), ["2", "1"]);
    }

    #[test]
    fn newest_desc_ties_id() {
        let mut v = vec![
            r("b", "x", State::Finished, 7),
            r("a", "x", State::Finished, 7),
            r("c", "x", State::Running, 9),
            r("d", "x", State::Idle, 1),
        ];
        sort_rows(&mut v, SortMode::Newest);
        assert_eq!(ids(&v), ["c", "a", "b", "d"]);
    }

    #[test]
    fn negative_and_extreme_times() {
        let mut v = vec![
            r("a", "x", State::Idle, i64::MIN),
            r("b", "x", State::Idle, -1),
            r("c", "x", State::Idle, i64::MAX),
            r("d", "x", State::Idle, 0),
        ];
        sort_rows(&mut v, SortMode::Newest);
        assert_eq!(ids(&v), ["c", "d", "b", "a"]);
    }

    #[test]
    fn empty_and_single() {
        for m in [SortMode::RunningFirst, SortMode::Name, SortMode::Newest] {
            let mut e: Vec<Row> = vec![];
            sort_rows(&mut e, m);
            assert!(e.is_empty());
            let mut s = vec![r("a", "a", State::Idle, 1)];
            sort_rows(&mut s, m);
            assert_eq!(ids(&s), ["a"]);
        }
    }

    #[test]
    fn deterministic_any_input_order() {
        let base = vec![
            r("a", "Zed", State::Idle, 3),
            r("b", "zed", State::Running, 3),
            r("c", "amy", State::Finished, 3),
            r("d", "Amy", State::Running, 8),
            r("e", "amy", State::Idle, 3),
        ];
        for m in [SortMode::RunningFirst, SortMode::Name, SortMode::Newest] {
            let mut want = base.clone();
            sort_rows(&mut want, m);
            let n = base.len();
            // all rotations and reversals
            for k in 0..n {
                let mut v = base.clone();
                v.rotate_left(k);
                sort_rows(&mut v, m);
                assert_eq!(v, want);
                v.reverse();
                sort_rows(&mut v, m);
                assert_eq!(v, want);
            }
        }
    }

    #[test]
    fn idempotent() {
        let mut v = vec![r("b", "B", State::Idle, 1), r("a", "a", State::Running, 1)];
        sort_rows(&mut v, SortMode::RunningFirst);
        let once = v.clone();
        sort_rows(&mut v, SortMode::RunningFirst);
        assert_eq!(v, once);
    }

    #[test]
    fn unicode_names_no_panic() {
        let mut v = vec![r("2", "Éa", State::Idle, 0), r("1", "éa", State::Idle, 0)];
        sort_rows(&mut v, SortMode::Name);
        assert_eq!(ids(&v), ["1", "2"]);
    }
}
