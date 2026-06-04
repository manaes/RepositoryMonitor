use crate::config::Config;
use crate::model::{RepoRef, RepoStatus};
use std::sync::atomic::{AtomicBool, AtomicU64};
use tokio::sync::Mutex;

/// Tauri `.manage(Arc<AppState>)`로 공유되는 런타임 상태.
pub struct AppState {
    pub config: Mutex<Config>,
    pub repos: Mutex<Vec<RepoRef>>,
    pub last_snapshot: Mutex<Vec<RepoStatus>>,
    pub polling_active: AtomicBool, // 창 포커스 시 true
    pub in_flight: AtomicBool,      // 상태 배치 진행 중
    pub seq: AtomicU64,             // emit 단조 증가 시퀀스
}

impl AppState {
    pub fn new(config: Config) -> Self {
        AppState {
            config: Mutex::new(config),
            repos: Mutex::new(Vec::new()),
            last_snapshot: Mutex::new(Vec::new()),
            polling_active: AtomicBool::new(false),
            in_flight: AtomicBool::new(false),
            seq: AtomicU64::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn new_has_expected_initial_state() {
        let st = AppState::new(crate::config::Config::default());
        assert!(st.repos.lock().await.is_empty());
        assert!(st.last_snapshot.lock().await.is_empty());
        assert!(!st.polling_active.load(Ordering::SeqCst));
        assert!(!st.in_flight.load(Ordering::SeqCst));
        assert_eq!(st.seq.load(Ordering::SeqCst), 0);
        assert_eq!(st.config.lock().await.poll_interval_secs, 30);
    }
}
