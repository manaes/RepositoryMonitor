use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoRef {
    pub path: String,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoState {
    #[default]
    Clean,
    Merging,
    Rebasing,
    CherryPicking,
    Reverting,
    Bisecting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoStatus {
    pub path: String,
    pub name: String,
    pub category: String,
    pub branch: Option<String>,
    pub detached_sha: Option<String>,
    pub upstream: Option<String>,
    pub has_upstream: bool,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
    pub conflicts: u32,
    pub stash: u32,
    pub is_clean: bool,
    pub state: RepoState,
    pub worktrees: u32,
    pub last_fetch: Option<i64>,
    pub last_checked: i64,
    pub error: Option<String>,
}

/// 외부 앱 열기 액션 종류(프론트→백엔드 IPC). 경로 복사는 프론트 navigator.clipboard 담당이라 제외.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    OpenFinder,
    OpenTerminal,
    OpenSourceTree,
}

/// repos_updated 이벤트 payload. seq로 프론트가 오래된 스냅샷을 폐기.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoSnapshot {
    pub seq: u64,
    pub repos: Vec<RepoStatus>,
}

impl RepoStatus {
    /// RepoRef로부터 기본(clean) RepoStatus를 만든다. git 읽기 전/실패 시 베이스로 사용.
    pub fn from_ref(repo: &RepoRef, now: i64) -> Self {
        RepoStatus {
            path: repo.path.clone(),
            name: repo.name.clone(),
            category: repo.category.clone(),
            branch: None,
            detached_sha: None,
            upstream: None,
            has_upstream: false,
            ahead: None,
            behind: None,
            staged: 0,
            modified: 0,
            untracked: 0,
            conflicts: 0,
            stash: 0,
            is_clean: true,
            state: RepoState::Clean,
            worktrees: 1,
            last_fetch: None,
            last_checked: now,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repostatus_serializes_snake_case_fields() {
        let s = RepoStatus {
            path: "/r".into(), name: "r".into(), category: "2_App".into(),
            branch: Some("main".into()), detached_sha: None, upstream: Some("origin/main".into()),
            has_upstream: true, ahead: Some(1), behind: Some(0),
            staged: 2, modified: 1, untracked: 1, conflicts: 0, stash: 0,
            is_clean: false, state: RepoState::Clean, worktrees: 1,
            last_fetch: Some(1_700_000_000), last_checked: 1_700_000_100, error: None,
        };
        let j = serde_json::to_value(&s).unwrap();
        assert_eq!(j["has_upstream"], true);
        assert_eq!(j["detached_sha"], serde_json::Value::Null);
        assert_eq!(j["state"], "clean"); // enum snake_case
    }

    #[test]
    fn reporef_roundtrips() {
        let r = RepoRef { path: "/r".into(), name: "r".into(), category: "lib".into() };
        let back: RepoRef = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn action_kind_serializes_snake_case() {
        assert_eq!(serde_json::to_string(&ActionKind::OpenFinder).unwrap(), "\"open_finder\"");
        assert_eq!(serde_json::to_string(&ActionKind::OpenSourceTree).unwrap(), "\"open_source_tree\"");
        let back: ActionKind = serde_json::from_str("\"open_terminal\"").unwrap();
        assert_eq!(back, ActionKind::OpenTerminal);
    }

    #[test]
    fn repo_snapshot_serializes() {
        let snap = RepoSnapshot { seq: 3, repos: vec![] };
        let j = serde_json::to_value(&snap).unwrap();
        assert_eq!(j["seq"], 3);
        assert!(j["repos"].is_array());
    }

    #[test]
    fn from_ref_builds_clean_placeholder() {
        let r = RepoRef { path: "/r".into(), name: "r".into(), category: "c".into() };
        let st = RepoStatus::from_ref(&r, 999);
        assert_eq!(st.path, "/r");
        assert_eq!(st.name, "r");
        assert_eq!(st.category, "c");
        assert_eq!(st.last_checked, 999);
        assert!(st.is_clean);
        assert_eq!(st.error, None);
        assert_eq!(st.ahead, None);
        assert_eq!(st.worktrees, 1);
        assert_eq!(st.state, RepoState::Clean);
    }
}
