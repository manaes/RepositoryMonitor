use gitmonitor::model::{RepoRef, VcsKind};
use gitmonitor::svn_reader::read_svn_status;
use std::path::Path;
use std::process::Command;

/// svn 서브커맨드 실행(current_dir 지정). 실패 시 패닉.
fn svn(cwd: &Path, args: &[&str]) {
    let out = Command::new("svn")
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "svn {:?} 실패: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

/// svnadmin 서브커맨드 실행. 실패 시 패닉.
fn svnadmin(args: &[&str]) {
    let out = Command::new("svnadmin").args(args).output().unwrap();
    assert!(
        out.status.success(),
        "svnadmin {:?} 실패: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn ref_for(p: &Path) -> RepoRef {
    RepoRef {
        path: p.to_string_lossy().into_owned(),
        name: "r".into(),
        category: "test".into(),
        vcs: VcsKind::Svn,
    }
}

#[test]
fn reads_svn_working_copy_dirty_on_trunk() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    // 1) 저장소 생성
    let repo_dir = base.join("repo");
    svnadmin(&["create", repo_dir.to_str().unwrap()]);
    // file:// URL은 절대경로로 구성
    let repo_url = format!("file://{}", repo_dir.to_string_lossy());

    // 2) 표준 레이아웃(trunk/branches/tags) 생성
    svn(
        base,
        &[
            "mkdir",
            "-m",
            "layout",
            &format!("{repo_url}/trunk"),
            &format!("{repo_url}/branches"),
            &format!("{repo_url}/tags"),
        ],
    );

    // 3) trunk 체크아웃
    let wc = base.join("wc");
    svn(
        base,
        &["checkout", &format!("{repo_url}/trunk"), wc.to_str().unwrap()],
    );

    // 4) 커밋된 파일 1개(이후 modify 대상) + untracked + add
    std::fs::write(wc.join("a.txt"), "a").unwrap();
    svn(&wc, &["add", "a.txt"]);
    svn(&wc, &["commit", "-m", "add a"]);
    // a.txt 수정(modified), b.txt add(modified로 카운트), untracked.txt는 add 안 함
    std::fs::write(wc.join("a.txt"), "a2").unwrap();
    std::fs::write(wc.join("b.txt"), "b").unwrap();
    svn(&wc, &["add", "b.txt"]);
    std::fs::write(wc.join("untracked.txt"), "u").unwrap();

    let st = read_svn_status(&ref_for(&wc), 12345);
    assert_eq!(st.error, None, "error: {:?}", st.error);
    assert_eq!(st.branch.as_deref(), Some("trunk"));
    assert!(st.has_upstream); // SVN은 항상 upstream 있음
    assert_eq!(st.modified, 2); // a.txt(M) + b.txt(A)
    assert_eq!(st.untracked, 1); // untracked.txt(?)
    assert_eq!(st.conflicts, 0);
    assert!(!st.is_clean);
    assert_eq!(st.last_checked, 12345);
    // SVN 개념 없는 필드 기본값 확인
    assert_eq!(st.staged, 0);
    assert_eq!(st.stash, 0);
    assert_eq!(st.ahead, None);
    assert_eq!(st.behind, None);
    assert_eq!(st.last_fetch, None);
}

#[test]
fn reads_clean_svn_working_copy() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();

    let repo_dir = base.join("repo");
    svnadmin(&["create", repo_dir.to_str().unwrap()]);
    let repo_url = format!("file://{}", repo_dir.to_string_lossy());
    svn(base, &["mkdir", "-m", "layout", &format!("{repo_url}/trunk")]);

    let wc = base.join("wc");
    svn(
        base,
        &["checkout", &format!("{repo_url}/trunk"), wc.to_str().unwrap()],
    );
    std::fs::write(wc.join("a.txt"), "a").unwrap();
    svn(&wc, &["add", "a.txt"]);
    svn(&wc, &["commit", "-m", "add a"]);

    let st = read_svn_status(&ref_for(&wc), 0);
    assert_eq!(st.error, None);
    assert!(st.is_clean);
    assert_eq!(st.modified, 0);
    assert_eq!(st.untracked, 0);
    assert_eq!(st.branch.as_deref(), Some("trunk"));
}
