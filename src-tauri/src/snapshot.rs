use crate::model::RepoStatus;

/// 신선 스냅샷에서 error가 난 repo는, 직전 스냅샷에 같은 path의 정상 항목이 있으면
/// 그 수치 필드를 유지하되 error/last_checked는 신선 값으로 덮어 머지한다.
pub fn merge_failed_with_previous(prev: &[RepoStatus], fresh: Vec<RepoStatus>) -> Vec<RepoStatus> {
    fresh
        .into_iter()
        .map(|f| {
            if f.error.is_some() {
                if let Some(p) = prev.iter().find(|p| p.path == f.path && p.error.is_none()) {
                    let mut merged = p.clone();
                    merged.error = f.error;
                    merged.last_checked = f.last_checked;
                    return merged;
                }
            }
            f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RepoRef, RepoStatus};

    fn rref(path: &str) -> RepoRef {
        RepoRef { path: path.into(), name: "n".into(), category: "c".into() }
    }

    #[test]
    fn failed_repo_keeps_previous_values() {
        // 직전: 정상(staged=3, branch=main)
        let mut prev_x = RepoStatus::from_ref(&rref("/x"), 100);
        prev_x.staged = 3;
        prev_x.branch = Some("main".into());
        let prev = vec![prev_x];

        // 신선: /x 가 error
        let mut fresh_x = RepoStatus::from_ref(&rref("/x"), 200);
        fresh_x.error = Some("git 실패".into());
        let fresh = vec![fresh_x];

        let merged = merge_failed_with_previous(&prev, fresh);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].staged, 3);                   // 직전 값 유지
        assert_eq!(merged[0].branch.as_deref(), Some("main"));
        assert_eq!(merged[0].error.as_deref(), Some("git 실패")); // error는 신선 값
        assert_eq!(merged[0].last_checked, 200);           // last_checked는 신선 값
    }

    #[test]
    fn ok_repo_passes_through() {
        let mut fresh_x = RepoStatus::from_ref(&rref("/x"), 200);
        fresh_x.staged = 5;
        let merged = merge_failed_with_previous(&[], vec![fresh_x]);
        assert_eq!(merged[0].staged, 5);
        assert_eq!(merged[0].error, None);
    }

    #[test]
    fn failed_repo_with_no_previous_stays_error() {
        let mut fresh_x = RepoStatus::from_ref(&rref("/new"), 200);
        fresh_x.error = Some("e".into());
        let merged = merge_failed_with_previous(&[], vec![fresh_x]);
        assert_eq!(merged[0].error.as_deref(), Some("e"));
    }
}
