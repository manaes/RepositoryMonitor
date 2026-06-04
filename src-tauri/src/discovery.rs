use globset::{GlobSet, GlobSetBuilder};

/// 제외 글롭 집합 빌드.
/// - 매칭 대상 = repo 절대경로 전체 문자열.
/// - '/'로 시작하면 절대 패턴 그대로. 그 외는 `**/<raw>`(그 경로 자체) + `**/<raw>/**`(그 이하)
///   두 글롭을 추가해 "디렉토리 자신"과 "그 내부"를 모두 매칭.
/// - 대소문자 무시(macOS 기본 FS).
pub fn build_exclude_set(globs: &[String]) -> GlobSet {
    // 패턴 문자열을 먼저 모은다.
    let mut patterns: Vec<String> = Vec::new();
    for raw in globs {
        let raw = raw.trim().trim_end_matches('/');
        if raw.is_empty() {
            continue;
        }
        if raw.starts_with('/') {
            patterns.push(raw.to_string());
        } else {
            patterns.push(format!("**/{raw}"));
            patterns.push(format!("**/{raw}/**"));
        }
    }
    // 대소문자 무시 + literal_separator: '*'는 '/'를 넘지 않고 '**'만 넘음
    let mut builder = GlobSetBuilder::new();
    for p in &patterns {
        if let Ok(g) = globset::GlobBuilder::new(p)
            .case_insensitive(true)
            .literal_separator(true)
            .build()
        {
            builder.add(g);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

use std::path::Path;

/// .git이 **디렉토리**인 경로만 정규 repo로 인정(연결 worktree의 .git 파일/gitlink 제외).
pub fn is_git_repo_dir(path: &Path) -> bool {
    path.join(".git").is_dir()
}

/// 카테고리 = repo의 (소속 루트 기준) 상대경로 첫 세그먼트.
/// 세그먼트가 1개(루트 직속)면 루트 폴더명을 사용.
pub fn category_for(repo_path: &Path, root: &Path) -> String {
    let rel = repo_path.strip_prefix(root).unwrap_or(repo_path);
    let mut comps = rel.components();
    match comps.next() {
        // 세그먼트가 2개 이상이면 첫 세그먼트가 카테고리
        Some(first) if comps.next().is_some() => {
            first.as_os_str().to_string_lossy().into_owned()
        }
        // 세그먼트 0~1개(루트 직속) → 루트 폴더명
        _ => root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "(root)".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_name_matches_anywhere() {
        // 'node_modules' → **/node_modules/** 로 확장, 절대경로 어디서나 매칭
        let set = build_exclude_set(&["node_modules".to_string()]);
        assert!(set.is_match("/Users/me/proj/node_modules/pkg"));
        assert!(!set.is_match("/Users/me/proj/src/main.rs"));
    }

    #[test]
    fn leading_slash_is_absolute() {
        let set = build_exclude_set(&["/Users/me/secret/*".to_string()]);
        assert!(set.is_match("/Users/me/secret/repo"));
        assert!(!set.is_match("/Users/other/secret/repo"));
    }

    #[test]
    fn case_insensitive() {
        let set = build_exclude_set(&["Pods".to_string()]);
        assert!(set.is_match("/a/PODS/x"));
    }

    #[test]
    fn empty_globs_match_nothing() {
        let set = build_exclude_set(&[]);
        assert!(!set.is_match("/anything/at/all"));
    }

    use std::path::Path;

    #[test]
    fn category_is_first_segment_under_root() {
        let root = Path::new("/Users/me/@Projects");
        assert_eq!(category_for(Path::new("/Users/me/@Projects/2_App/GitMonitor"), root), "2_App");
        assert_eq!(category_for(Path::new("/Users/me/@Projects/@ITXRtsp/edge-client-swift"), root), "@ITXRtsp");
    }

    #[test]
    fn category_root_direct_uses_root_name() {
        let root = Path::new("/Users/me/@Projects");
        // repo가 루트 바로 아래면 상대경로 세그먼트가 1개 → 카테고리는 루트 폴더명
        assert_eq!(category_for(Path::new("/Users/me/@Projects/loneRepo"), root), "@Projects");
    }

    #[test]
    fn is_git_repo_dir_requires_git_directory() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("r");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        assert!(is_git_repo_dir(&repo));

        // .git이 파일(연결 worktree/gitlink)이면 repo 아님
        let wt = dir.path().join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join(".git"), b"gitdir: /somewhere").unwrap();
        assert!(!is_git_repo_dir(&wt));
    }
}
