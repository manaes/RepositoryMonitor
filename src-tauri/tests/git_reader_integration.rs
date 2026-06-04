use repositorymonitor::git_reader::read_status;
use repositorymonitor::model::{RepoRef, RepoState, VcsKind};
use std::path::Path;
use std::process::Command;

fn git(repo: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {:?} 실패", args);
}

fn ref_for(p: &Path) -> RepoRef {
    RepoRef {
        path: p.to_string_lossy().into_owned(),
        name: "r".into(),
        category: "test".into(),
        vcs: VcsKind::Git,
    }
}

#[test]
fn reads_clean_repo_with_no_upstream() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    std::fs::write(repo.join("a.txt"), "a").unwrap();
    git(repo, &["add", "a.txt"]);
    git(repo, &["commit", "-qm", "init"]);

    let st = read_status(&ref_for(repo), 12345);
    assert_eq!(st.error, None);
    assert!(st.is_clean);
    assert!(!st.has_upstream);
    assert_eq!(st.ahead, None);
    assert_eq!(st.behind, None);
    assert_eq!(st.state, RepoState::Clean);
    assert_eq!(st.stash, 0); // stash 0개 — stash list가 빈 출력
    assert_eq!(st.worktrees, 1);
    assert_eq!(st.last_checked, 12345);
}

#[test]
fn counts_stash() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    std::fs::write(repo.join("a.txt"), "a").unwrap();
    git(repo, &["add", "a.txt"]);
    git(repo, &["commit", "-qm", "init"]);
    std::fs::write(repo.join("a.txt"), "b").unwrap();
    git(repo, &["stash"]);
    std::fs::write(repo.join("a.txt"), "c").unwrap();
    git(repo, &["stash"]);

    let st = read_status(&ref_for(repo), 0);
    assert_eq!(st.stash, 2);
}

#[test]
fn detects_merge_conflict_state() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("f"), "base\n").unwrap();
    git(repo, &["add", "f"]);
    git(repo, &["commit", "-qm", "base"]);
    git(repo, &["checkout", "-q", "-b", "feat"]);
    std::fs::write(repo.join("f"), "feat\n").unwrap();
    git(repo, &["commit", "-qam", "feat"]);
    git(repo, &["checkout", "-q", "main"]);
    std::fs::write(repo.join("f"), "mainc\n").unwrap();
    git(repo, &["commit", "-qam", "mainc"]);
    // 충돌 머지 (실패해도 무시)
    let _ = Command::new("git").arg("-C").arg(repo).args(["merge", "feat"]).output();

    let st = read_status(&ref_for(repo), 0);
    assert_eq!(st.conflicts, 1);
    assert_eq!(st.state, RepoState::Merging);
    assert!(!st.is_clean);
}

#[test]
fn counts_linked_worktrees() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("main");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("a"), "a").unwrap();
    git(&repo, &["add", "a"]);
    git(&repo, &["commit", "-qm", "init"]);
    git(&repo, &["worktree", "add", "-q", "../wt2"]);

    let st = read_status(&ref_for(&repo), 0);
    assert_eq!(st.worktrees, 2); // 메인 + 연결 1개
}

#[test]
fn missing_repo_sets_error() {
    let r = RepoRef {
        path: "/no/such/repo/xyz".into(),
        name: "x".into(),
        category: "test".into(),
        vcs: VcsKind::Git,
    };
    let st = read_status(&r, 0);
    assert!(st.error.is_some());
}

#[test]
fn reads_empty_repo() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    // 커밋 0개 빈 repo
    let st = read_status(&ref_for(repo), 0);
    assert_eq!(st.error, None);
    assert!(st.is_clean);
    assert!(st.branch.is_some());        // 기본 브랜치명(main/master는 환경 의존이라 값은 단언 안 함)
    assert_eq!(st.detached_sha, None);   // (initial)은 sha로 취급하지 않음
}

#[test]
fn reads_detached_head() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    std::fs::write(repo.join("a"), "a").unwrap();
    git(repo, &["add", "a"]);
    git(repo, &["commit", "-qm", "init"]);
    git(repo, &["checkout", "-q", "--detach", "HEAD"]);

    let st = read_status(&ref_for(repo), 0);
    assert_eq!(st.branch, None);
    assert!(st.detached_sha.is_some());
    assert!(st.is_clean);
}

#[test]
fn reads_staged_rename() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    std::fs::write(repo.join("old.txt"), "content\n").unwrap();
    git(repo, &["add", "old.txt"]);
    git(repo, &["commit", "-qm", "init"]);
    git(repo, &["mv", "old.txt", "new.txt"]); // staged rename

    let st = read_status(&ref_for(repo), 0);
    assert_eq!(st.staged, 1);
    assert_eq!(st.modified, 0);
    assert!(!st.is_clean);
}
