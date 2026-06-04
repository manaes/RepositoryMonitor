use crate::git_reader::read_status;
use crate::model::{RepoRef, RepoStatus};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// 여러 repo의 상태를 병렬(동시 상한 concurrency, per-repo timeout)로 읽는다.
/// read_status는 블로킹이므로 spawn_blocking으로 실행하고 timeout으로 감싼다.
/// timeout/실행 실패 repo는 error가 채워진 RepoStatus로 반환.
pub async fn run_batch(
    repos: Vec<RepoRef>,
    now: i64,
    concurrency: usize,
    timeout: Duration,
) -> Vec<RepoStatus> {
    let sem = Arc::new(Semaphore::new(concurrency.max(1)));
    let mut handles = Vec::with_capacity(repos.len());

    for repo in repos {
        let sem = sem.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore closed");
            let repo_for_err = repo.clone();
            let blocking = tokio::task::spawn_blocking(move || read_status(&repo, now));
            match tokio::time::timeout(timeout, blocking).await {
                Ok(Ok(st)) => st,
                _ => {
                    let mut st = RepoStatus::from_ref(&repo_for_err, now);
                    st.error = Some("git 읽기 timeout 또는 실행 실패".to_string());
                    st
                }
            }
        }));
    }

    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        if let Ok(st) = h.await {
            out.push(st);
        }
    }
    out
}
