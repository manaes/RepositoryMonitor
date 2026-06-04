use gitmonitor::batch::run_batch;
use gitmonitor::model::{RepoRef, VcsKind};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

fn git(repo: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C").arg(repo).args(args)
        .env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t")
        .status().unwrap().success();
    assert!(ok, "git {:?} 실패", args);
}

fn init_repo(dir: &Path) -> RepoRef {
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("a"), "a").unwrap();
    git(dir, &["add", "a"]);
    git(dir, &["commit", "-qm", "init"]);
    RepoRef { path: dir.to_string_lossy().into_owned(), name: "r".into(), category: "c".into(), vcs: VcsKind::Git }
}

#[tokio::test]
async fn runs_all_repos_and_marks_missing_as_error() {
    let d1 = tempfile::tempdir().unwrap();
    let d2 = tempfile::tempdir().unwrap();
    let r1 = init_repo(d1.path());
    let r2 = init_repo(d2.path());
    let missing = RepoRef { path: "/no/such/repo/zzz".into(), name: "x".into(), category: "c".into(), vcs: VcsKind::Git };

    let out = run_batch(vec![r1, r2, missing], 1000, 8, Duration::from_secs(5)).await;
    assert_eq!(out.len(), 3);
    let errors = out.iter().filter(|s| s.error.is_some()).count();
    assert_eq!(errors, 1); // 없는 repo만 error
    assert!(out.iter().all(|s| s.last_checked == 1000));
}

#[tokio::test]
async fn tiny_timeout_marks_timeout_error() {
    let d1 = tempfile::tempdir().unwrap();
    let r1 = init_repo(d1.path());
    // 1ns 타임아웃 → read_status가 못 끝내고 timeout error
    let out = run_batch(vec![r1], 0, 8, Duration::from_nanos(1)).await;
    assert_eq!(out.len(), 1);
    assert!(out[0].error.is_some());
}
