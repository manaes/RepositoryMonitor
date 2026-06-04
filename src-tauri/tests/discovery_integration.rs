use repositorymonitor::discovery::{discover, DiscoveryConfig};
use std::fs;
use std::path::Path;

fn mk_repo(base: &Path, rel: &str) {
    let p = base.join(rel);
    fs::create_dir_all(p.join(".git")).unwrap();
}

#[test]
fn discovers_repos_with_category_and_prunes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    mk_repo(root, "2_App/RepositoryMonitor");
    mk_repo(root, "@ITXRtsp/edge-client-swift");
    // node_modules 안의 .git은 prune되어 잡히면 안 됨
    fs::create_dir_all(root.join("2_App/RepositoryMonitor/node_modules/dep/.git")).unwrap();
    // 깊이 초과 repo (depth=4 기준 5단계) — 잡히면 안 됨
    mk_repo(root, "a/b/c/d/tooDeep");

    let roots = vec![root.to_string_lossy().into_owned()];
    let cfg = DiscoveryConfig {
        roots: &roots,
        manual_paths: &[],
        exclude_globs: &[],
        scan_depth: 4,
    };
    let mut found: Vec<(String, String)> = discover(&cfg)
        .into_iter()
        .map(|r| (r.name, r.category))
        .collect();
    found.sort();

    assert!(found.contains(&("RepositoryMonitor".to_string(), "2_App".to_string())));
    assert!(found.contains(&("edge-client-swift".to_string(), "@ITXRtsp".to_string())));
    // node_modules 안 repo는 제외
    assert!(!found.iter().any(|(n, _)| n == "dep"));
    // 너무 깊은 repo는 제외
    assert!(!found.iter().any(|(n, _)| n == "tooDeep"));
}

#[test]
fn exclude_glob_removes_repo() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    mk_repo(root, "keep/repoA");
    mk_repo(root, "skip/repoB");

    let roots = vec![root.to_string_lossy().into_owned()];
    let excludes = vec!["repoB".to_string()];
    let cfg = DiscoveryConfig {
        roots: &roots,
        manual_paths: &[],
        exclude_globs: &excludes,
        scan_depth: 4,
    };
    let names: Vec<String> = discover(&cfg).into_iter().map(|r| r.name).collect();
    assert!(names.contains(&"repoA".to_string()));
    assert!(!names.contains(&"repoB".to_string()));
}

#[test]
fn manual_path_outside_root_gets_manual_category() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir_all(&root).unwrap();
    let manual = dir.path().join("elsewhere/manualRepo");
    fs::create_dir_all(manual.join(".git")).unwrap();

    let roots = vec![root.to_string_lossy().into_owned()];
    let manuals = vec![manual.to_string_lossy().into_owned()];
    let cfg = DiscoveryConfig {
        roots: &roots,
        manual_paths: &manuals,
        exclude_globs: &[],
        scan_depth: 4,
    };
    let found = discover(&cfg);
    let m = found.iter().find(|r| r.name == "manualRepo").unwrap();
    assert_eq!(m.category, "(manual)");
}
