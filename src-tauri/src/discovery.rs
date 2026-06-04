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

use crate::model::RepoRef;
use std::collections::BTreeSet;
use walkdir::WalkDir;

/// discover 입력. 참조만 들고 있어 호출부 소유권을 건드리지 않음.
pub struct DiscoveryConfig<'a> {
    pub roots: &'a [String],
    pub manual_paths: &'a [String],
    pub exclude_globs: &'a [String],
    pub scan_depth: u32,
}

/// 탐색 중 descent를 막을 무거운 디렉토리 이름.
const PRUNE_DIRS: &[&str] = &["node_modules", "target", "Pods", ".build", ".git"];

/// 등록 루트 스캔 + 수동 경로를 합쳐 RepoRef 목록 산출. 제외 글롭 적용, 경로 dedup.
pub fn discover(cfg: &DiscoveryConfig) -> Vec<RepoRef> {
    let excl = build_exclude_set(cfg.exclude_globs);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<RepoRef> = Vec::new();

    for root in cfg.roots {
        let root_path = Path::new(root);
        let walker = WalkDir::new(root_path)
            .max_depth(cfg.scan_depth as usize)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // 루트 자신은 통과, 그 외 PRUNE 디렉토리는 descent 차단
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                !PRUNE_DIRS.contains(&name.as_ref())
            });

        for entry in walker.filter_map(|e| e.ok()) {
            if !entry.file_type().is_dir() {
                continue;
            }
            let dir = entry.path();
            if is_git_repo_dir(dir) {
                let cat = category_for(dir, root_path);
                push_repo(dir, cat, &excl, &mut seen, &mut out);
            }
        }
    }

    for mp in cfg.manual_paths {
        let p = Path::new(mp);
        if !is_git_repo_dir(p) {
            continue;
        }
        // 어느 루트 하위면 그 루트 기준 카테고리, 아니면 (manual)
        let cat = cfg
            .roots
            .iter()
            .map(Path::new)
            .find(|r| p.starts_with(r))
            .map(|r| category_for(p, r))
            .unwrap_or_else(|| "(manual)".to_string());
        push_repo(p, cat, &excl, &mut seen, &mut out);
    }

    out
}

/// 제외 글롭 통과 + dedup 후 RepoRef 추가.
fn push_repo(
    dir: &Path,
    category: String,
    excl: &globset::GlobSet,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<RepoRef>,
) {
    let abs = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let key = abs.to_string_lossy().into_owned();
    if excl.is_match(&key) || !seen.insert(key.clone()) {
        return;
    }
    let name = abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    out.push(RepoRef {
        path: key,
        name,
        category,
    });
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
