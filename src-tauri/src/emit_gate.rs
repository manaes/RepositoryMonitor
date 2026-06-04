use crate::model::RepoStatus;

/// last_checked만 무시하고 두 RepoStatus가 의미상 동일한지.
fn eq_ignoring_time(a: &RepoStatus, b: &RepoStatus) -> bool {
    let mut a2 = a.clone();
    let mut b2 = b.clone();
    a2.last_checked = 0;
    b2.last_checked = 0;
    a2 == b2
}

/// 직전 스냅샷 대비 변경이 있으면 true(emit). last_checked 변화만으로는 emit 안 함.
pub fn should_emit(prev: &[RepoStatus], next: &[RepoStatus]) -> bool {
    if prev.len() != next.len() {
        return true;
    }
    !prev.iter().zip(next).all(|(a, b)| eq_ignoring_time(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RepoStatus;

    fn s(now: i64, staged: u32) -> RepoStatus {
        let mut x = RepoStatus::from_ref(
            &crate::model::RepoRef { path: "/r".into(), name: "r".into(), category: "c".into() },
            now,
        );
        x.staged = staged;
        x.is_clean = staged == 0;
        x
    }

    #[test]
    fn same_except_last_checked_does_not_emit() {
        let a = vec![s(100, 0)];
        let b = vec![s(200, 0)]; // last_checked만 다름
        assert!(!should_emit(&a, &b));
    }

    #[test]
    fn changed_count_emits() {
        let a = vec![s(100, 0)];
        let b = vec![s(100, 2)];
        assert!(should_emit(&a, &b));
    }

    #[test]
    fn different_len_emits() {
        let a = vec![s(100, 0)];
        let b: Vec<RepoStatus> = vec![];
        assert!(should_emit(&a, &b));
    }
}
