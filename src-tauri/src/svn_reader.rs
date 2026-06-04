use crate::model::{RepoRef, RepoStatus};

/// `svn status`(로컬) 파싱 결과. git의 ParsedStatus와 대응하되 SVN에 없는 필드는 제외.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SvnParsed {
    pub modified: u32,
    pub untracked: u32,
    pub conflicts: u32,
    pub is_clean: bool,
}

/// `svn status`(로컬, `-u` 안 씀) 출력 파싱.
/// col0: M/A/D/R/!/~ → modified, ? → untracked, C → conflicts, I/X/공백 → 스킵.
/// col0이 공백이어도 col1(prop)이 M → modified, C → conflicts.
pub fn parse_svn_status(text: &str) -> SvnParsed {
    let mut s = SvnParsed::default();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let b = line.as_bytes();
        let c0 = *b.first().unwrap_or(&b' ');
        let c1 = if b.len() > 1 { b[1] } else { b' ' };
        match c0 {
            b'M' | b'A' | b'D' | b'R' | b'!' | b'~' => s.modified += 1,
            b'?' => s.untracked += 1,
            b'C' => s.conflicts += 1,
            b'I' | b'X' => {} // ignored/external
            _ => {
                // col0 공백이어도 prop 변경(col1)이 있으면 반영
                if c1 == b'M' {
                    s.modified += 1;
                } else if c1 == b'C' {
                    s.conflicts += 1;
                }
            }
        }
    }
    s.is_clean = s.modified == 0 && s.untracked == 0 && s.conflicts == 0;
    s
}

/// `svn info --show-item relative-url`(예: "^/trunk") → 브랜치명.
/// `^/trunk`→`trunk`, `^/branches/<b>`→`<b>`, `^/tags/<t>`→`tags/<t>`, 그 외 첫 세그먼트.
pub fn svn_branch(relurl: &str) -> Option<String> {
    let s = relurl
        .trim()
        .trim_start_matches("^/")
        .trim_start_matches('/');
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split('/').collect();
    match parts.as_slice() {
        ["trunk", ..] => Some("trunk".to_string()),
        ["branches", b, ..] => Some((*b).to_string()),
        ["tags", t, ..] => Some(format!("tags/{t}")),
        [first, ..] => Some((*first).to_string()),
        [] => None,
    }
}

/// repo에서 svn 서브커맨드 실행(동기). 비-제로 종료/실패는 Err.
/// ⚠️ svn은 git의 `-C <dir>` 옵션이 없으므로 current_dir(repo)로 작업 디렉토리를 지정한다.
fn run_svn(repo: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("svn")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|e| format!("svn 실행 실패: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "svn {:?} 실패: {}",
            args,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// RepoRef(vcs=Svn)에 SVN 상태를 채워 RepoStatus 생성. 로컬 전용(네트워크 0).
/// staged/stash/ahead/behind/worktrees/last_fetch는 SVN 개념이 없어 from_ref 기본값 유지.
pub fn read_svn_status(repo: &RepoRef, now: i64) -> RepoStatus {
    let mut st = RepoStatus::from_ref(repo, now); // vcs=Svn 포함
    st.has_upstream = true; // SVN은 항상 repo URL 보유
    match run_svn(&repo.path, &["status"]) {
        Ok(text) => {
            let p = parse_svn_status(&text);
            st.modified = p.modified;
            st.untracked = p.untracked;
            st.conflicts = p.conflicts;
            st.is_clean = p.is_clean;
            // 브랜치(로컬): svn info --show-item relative-url
            if let Ok(relurl) = run_svn(&repo.path, &["info", "--show-item", "relative-url"]) {
                st.branch = svn_branch(&relurl);
            }
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
    fn parses_dirty_fixture() {
        // 실측 픽스처: M/A/?
        let txt = "\
M       a.txt
A       b.txt
?       untracked.txt
";
        let p = parse_svn_status(txt);
        assert_eq!(p.modified, 2); // M + A
        assert_eq!(p.untracked, 1); // ?
        assert_eq!(p.conflicts, 0);
        assert!(!p.is_clean);
    }

    #[test]
    fn parses_clean_empty() {
        let p = parse_svn_status("");
        assert_eq!((p.modified, p.untracked, p.conflicts), (0, 0, 0));
        assert!(p.is_clean);
    }

    #[test]
    fn parses_conflict() {
        let txt = "C       c.txt\n";
        let p = parse_svn_status(txt);
        assert_eq!(p.conflicts, 1);
        assert_eq!(p.modified, 0);
        assert!(!p.is_clean);
    }

    #[test]
    fn parses_prop_only_modified() {
        // col0 공백 + col1(prop)=M → modified
        let txt = " M      props.txt\n";
        let p = parse_svn_status(txt);
        assert_eq!(p.modified, 1);
        assert_eq!(p.untracked, 0);
        assert!(!p.is_clean);
    }

    #[test]
    fn parses_prop_only_conflict() {
        // col0 공백 + col1(prop)=C → conflict
        let txt = " C      props.txt\n";
        let p = parse_svn_status(txt);
        assert_eq!(p.conflicts, 1);
        assert!(!p.is_clean);
    }

    #[test]
    fn skips_ignored_and_external() {
        let txt = "I       ignored.txt\nX       ext\n";
        let p = parse_svn_status(txt);
        assert_eq!((p.modified, p.untracked, p.conflicts), (0, 0, 0));
        assert!(p.is_clean);
    }

    #[test]
    fn counts_delete_replace_missing() {
        let txt = "D       d.txt\nR       r.txt\n!       missing.txt\n";
        let p = parse_svn_status(txt);
        assert_eq!(p.modified, 3); // D + R + !
        assert!(!p.is_clean);
    }

    #[test]
    fn branch_trunk() {
        assert_eq!(svn_branch("^/trunk"), Some("trunk".to_string()));
        assert_eq!(svn_branch("^/trunk/sub"), Some("trunk".to_string()));
    }

    #[test]
    fn branch_branches() {
        assert_eq!(svn_branch("^/branches/feat"), Some("feat".to_string()));
        assert_eq!(svn_branch("^/branches/feat/deep"), Some("feat".to_string()));
    }

    #[test]
    fn branch_tags() {
        assert_eq!(svn_branch("^/tags/v1"), Some("tags/v1".to_string()));
    }

    #[test]
    fn branch_root_is_none() {
        // SVN 저장소 루트를 체크아웃하면 relative-url이 "^/" → 브랜치 없음
        assert_eq!(svn_branch("^/"), None);
        assert_eq!(svn_branch(""), None);
    }

    #[test]
    fn branch_other_first_segment() {
        assert_eq!(svn_branch("^/custom/x"), Some("custom".to_string()));
    }
}
