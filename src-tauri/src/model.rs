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
}
