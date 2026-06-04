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
}
