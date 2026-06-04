/// porcelain v2 --branch 파싱 결과(브랜치/원격/워킹트리 카운트). 시간/IO 없음.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ParsedStatus {
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
    pub is_clean: bool,
}

/// `git status --porcelain=v2 --branch` 출력을 파싱.
pub fn parse_porcelain_v2(text: &str) -> ParsedStatus {
    let mut s = ParsedStatus::default();
    let mut oid: Option<String> = None;
    let mut detached = false;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# branch.oid ") {
            oid = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("# branch.head ") {
            let v = rest.trim();
            if v == "(detached)" {
                detached = true;
                s.branch = None;
            } else {
                s.branch = Some(v.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("# branch.upstream ") {
            s.upstream = Some(rest.trim().to_string());
            s.has_upstream = true;
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            let mut ahead = 0u32;
            let mut behind = 0u32;
            for tok in rest.split_whitespace() {
                if let Some(n) = tok.strip_prefix('+') {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(m) = tok.strip_prefix('-') {
                    behind = m.parse().unwrap_or(0);
                }
            }
            s.ahead = Some(ahead);
            s.behind = Some(behind);
        } else if line.starts_with("1 ") || line.starts_with("2 ") {
            // XY는 항상 두 번째 공백구분 토큰
            if let Some(xy) = line.split_whitespace().nth(1) {
                let b = xy.as_bytes();
                if b.len() >= 2 {
                    if b[0] != b'.' {
                        s.staged += 1;
                    }
                    if b[1] != b'.' {
                        s.modified += 1;
                    }
                }
            }
        } else if line.starts_with("u ") {
            s.conflicts += 1;
        } else if line.starts_with("? ") {
            s.untracked += 1;
        }
    }

    s.is_clean = s.staged == 0 && s.modified == 0 && s.untracked == 0 && s.conflicts == 0;

    // detached일 때만 oid를 sha로 채움. '(initial)'/'(detached)' 같은 괄호 토큰은 sha 아님.
    if detached {
        if let Some(o) = oid {
            if !o.starts_with('(') {
                s.detached_sha = Some(o);
            }
        }
    }

    s
}

use crate::model::{RepoRef, RepoState, RepoStatus};
use std::path::Path;
use std::process::Command;

/// repo에서 git 서브커맨드 실행(동기). 비-제로 종료/실패는 Err.
/// (타임아웃·동시성은 M2의 비동기 계층에서 추가)
fn run_git(repo: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("git 실행 실패: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {:?} 실패: {}",
            args,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// stash 개수. ⚠️ rev-list --count refs/stash 는 0개일 때 exit 128 fatal이므로
/// `git stash list` 줄 수로 센다(0개면 빈 출력).
pub fn stash_count(repo: &str) -> u32 {
    match run_git(repo, &["stash", "list"]) {
        Ok(out) => out.lines().filter(|l| !l.trim().is_empty()).count() as u32,
        Err(_) => 0,
    }
}

/// 연결 worktree 포함 총 worktree 수(메인 포함, >=1).
pub fn worktree_count(repo: &str) -> u32 {
    match run_git(repo, &["worktree", "list", "--porcelain"]) {
        Ok(out) => out.lines().filter(|l| l.starts_with("worktree ")).count() as u32,
        Err(_) => 1,
    }
}

/// `git rev-parse --git-path <marker>`로 경로 해석 후 존재 확인.
/// (worktree에서 .git이 파일이어도 올바른 경로를 돌려줌)
fn git_path_exists(repo: &str, marker: &str) -> bool {
    match run_git(repo, &["rev-parse", "--git-path", marker]) {
        Ok(p) => {
            let p = p.trim();
            let pb = Path::new(p);
            let full = if pb.is_absolute() {
                pb.to_path_buf()
            } else {
                Path::new(repo).join(pb)
            };
            full.exists()
        }
        Err(_) => false,
    }
}

/// 진행 중 상태 판정(우선순위: Merging > Rebasing > CherryPicking > Reverting > Bisecting).
pub fn detect_state(repo: &str) -> RepoState {
    if git_path_exists(repo, "MERGE_HEAD") {
        RepoState::Merging
    } else if git_path_exists(repo, "rebase-merge") || git_path_exists(repo, "rebase-apply") {
        RepoState::Rebasing
    } else if git_path_exists(repo, "CHERRY_PICK_HEAD") {
        RepoState::CherryPicking
    } else if git_path_exists(repo, "REVERT_HEAD") {
        RepoState::Reverting
    } else if git_path_exists(repo, "BISECT_LOG") {
        RepoState::Bisecting
    } else {
        RepoState::Clean
    }
}

/// FETCH_HEAD mtime(epoch). git-path 우선, 없으면 common-dir 폴백, 둘 다 없으면 None.
pub fn last_fetch(repo: &str) -> Option<i64> {
    fn mtime(p: &Path) -> Option<i64> {
        std::fs::metadata(p)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
    }
    fn resolve(repo: &str, rel_or_abs: &str) -> std::path::PathBuf {
        let pb = Path::new(rel_or_abs.trim());
        if pb.is_absolute() {
            pb.to_path_buf()
        } else {
            Path::new(repo).join(pb)
        }
    }

    if let Ok(p) = run_git(repo, &["rev-parse", "--git-path", "FETCH_HEAD"]) {
        if let Some(t) = mtime(&resolve(repo, &p)) {
            return Some(t);
        }
    }
    if let Ok(common) = run_git(repo, &["rev-parse", "--git-common-dir"]) {
        let base = resolve(repo, &common);
        if let Some(t) = mtime(&base.join("FETCH_HEAD")) {
            return Some(t);
        }
    }
    None
}

/// RepoRef에 휘발성 git 상태를 채워 RepoStatus 생성. now=배치 시각(epoch).
/// git status 실패 시 error만 세팅하고 나머지는 기본값.
pub fn read_status(repo: &RepoRef, now: i64) -> RepoStatus {
    let mut st = RepoStatus::from_ref(repo, now);

    match run_git(&repo.path, &["status", "--porcelain=v2", "--branch"]) {
        Ok(text) => {
            let p = parse_porcelain_v2(&text);
            st.branch = p.branch;
            st.detached_sha = p.detached_sha;
            st.upstream = p.upstream;
            st.has_upstream = p.has_upstream;
            st.ahead = p.ahead;
            st.behind = p.behind;
            st.staged = p.staged;
            st.modified = p.modified;
            st.untracked = p.untracked;
            st.conflicts = p.conflicts;
            st.is_clean = p.is_clean;
            st.stash = stash_count(&repo.path);
            st.state = detect_state(&repo.path);
            st.worktrees = worktree_count(&repo.path);
            st.last_fetch = last_fetch(&repo.path);
        }
        Err(e) => {
            st.error = Some(e);
        }
    }

    st
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_in_sync() {
        let txt = "\
# branch.oid 14bbe8869730251fa97f51cfae8ccdf81d6337ff
# branch.head master
# branch.upstream origin/main
# branch.ab +0 -0
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.branch.as_deref(), Some("master"));
        assert_eq!(p.detached_sha, None);
        assert_eq!(p.upstream.as_deref(), Some("origin/main"));
        assert!(p.has_upstream);
        assert_eq!(p.ahead, Some(0));
        assert_eq!(p.behind, Some(0));
        assert_eq!((p.staged, p.modified, p.untracked, p.conflicts), (0, 0, 0, 0));
        assert!(p.is_clean);
    }

    #[test]
    fn dirty_mm_staged_untracked() {
        let txt = "\
# branch.oid 14bbe8869730251fa97f51cfae8ccdf81d6337ff
# branch.head master
# branch.upstream origin/main
# branch.ab +0 -0
1 MM N... 100644 100644 100644 7898192261 9ad2ebbaff a.txt
1 A. N... 000000 100644 100644 0000000000 b4785957bc staged.txt
? untracked.txt
";
        let p = parse_porcelain_v2(txt);
        // MM: X=M(staged), Y=M(modified) / A.: X=A(staged), Y=.(아님)
        assert_eq!(p.staged, 2);
        assert_eq!(p.modified, 1);
        assert_eq!(p.untracked, 1);
        assert_eq!(p.conflicts, 0);
        assert!(!p.is_clean);
    }

    #[test]
    fn no_upstream_has_no_ab_line() {
        let txt = "\
# branch.oid 91269049e9456bed48ceef4a5d6e75df9b53f5f0
# branch.head master
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.branch.as_deref(), Some("master"));
        assert!(!p.has_upstream);
        assert_eq!(p.upstream, None);
        assert_eq!(p.ahead, None);   // ab 라인 없음 → None(미표시)
        assert_eq!(p.behind, None);
        assert!(p.is_clean);
    }

    #[test]
    fn detached_head() {
        let txt = "\
# branch.oid 8d42fe5d803a1ee92a51f252eea10c73629a85d3
# branch.head (detached)
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.branch, None);
        assert_eq!(p.detached_sha.as_deref(), Some("8d42fe5d803a1ee92a51f252eea10c73629a85d3"));
        assert!(p.is_clean);
    }

    #[test]
    fn empty_repo_initial_oid_is_not_sha() {
        let txt = "\
# branch.oid (initial)
# branch.head main
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.branch.as_deref(), Some("main"));
        assert_eq!(p.detached_sha, None);  // (initial)은 sha 아님
        assert!(p.is_clean);
    }

    #[test]
    fn conflict_u_line() {
        let txt = "\
# branch.oid 895821ddd416d350bc94fd0687f354581c6a50f8
# branch.head main
u UU N... 100644 100644 100644 100644 df967b c278a3 c77470 f
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.conflicts, 1);
        assert_eq!(p.staged, 0);   // 충돌은 u 라인 전용, 1/2와 겹치지 않음
        assert_eq!(p.modified, 0);
        assert!(!p.is_clean);
    }

    #[test]
    fn rename_2_line_counts_staged_only() {
        // git mv로 rename하면 staged rename(R.)으로 나옴: 2번째 토큰이 XY
        let txt = "\
# branch.oid 895821ddd416d350bc94fd0687f354581c6a50f8
# branch.head main
2 R. N... 100644 100644 100644 df967b df967b R100 new.txt\told.txt
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.staged, 1);   // R(=X) != '.'
        assert_eq!(p.modified, 0); // Y == '.'
        assert!(!p.is_clean);
    }
}
