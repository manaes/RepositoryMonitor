use crate::actions;
use crate::app_state::AppState;
use crate::batch::run_batch;
use crate::config::{self, Config};
use crate::discovery::{discover, DiscoveryConfig};
use crate::emit_gate::should_emit;
use crate::model::{ActionKind, RepoRef, RepoSnapshot};
use crate::snapshot::merge_failed_with_previous;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

const CONCURRENCY: usize = 8;
const GIT_TIMEOUT: Duration = Duration::from_secs(5);

/// in_flight 플래그용 RAII 가드.
/// Drop 시 무조건 false로 되돌려, 본문에서 panic/early-return 등 어떤 경로로 빠져나가도
/// in_flight 가 true로 영구히 남아 모든 후속 refresh가 coalesce되는(데드락) 상황을 막는다.
struct InFlightGuard<'a> {
    flag: &'a AtomicBool,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

/// 현재 epoch(초).
pub fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn get_config(state: State<'_, Arc<AppState>>) -> Result<Config, String> {
    Ok(state.config.lock().await.clone())
}

#[tauri::command]
pub async fn set_config(state: State<'_, Arc<AppState>>, config: Config) -> Result<(), String> {
    let path = config::config_path();
    config::save_to(&path, &config).map_err(|e| e.to_string())?;
    *state.config.lock().await = config;
    Ok(())
}

/// discovery 재실행 후 repo 목록을 AppState에 저장하고 반환.
pub async fn do_scan(state: &Arc<AppState>) -> Result<Vec<RepoRef>, String> {
    let cfg = state.config.lock().await.clone();
    let repos = tokio::task::spawn_blocking(move || {
        let dcfg = DiscoveryConfig {
            roots: &cfg.roots,
            manual_paths: &cfg.manual_paths,
            exclude_globs: &cfg.exclude_globs,
            scan_depth: cfg.scan_depth,
        };
        discover(&dcfg)
    })
    .await
    .map_err(|e| e.to_string())?;
    *state.repos.lock().await = repos.clone();
    Ok(repos)
}

#[tauri::command]
pub async fn scan_repos(state: State<'_, Arc<AppState>>) -> Result<Vec<RepoRef>, String> {
    do_scan(state.inner()).await
}

/// 현재 repo 목록의 상태를 읽어 머지하고, 변경 시 repos_updated 이벤트를 emit.
pub async fn do_refresh(app: &AppHandle, state: &Arc<AppState>) -> Result<(), String> {
    do_refresh_inner(state, |snap| {
        let _ = app.emit("repos_updated", snap);
    })
    .await
}

/// do_refresh의 핵심 로직. emit을 콜백으로 분리해 Tauri 런타임(AppHandle) 없이도 테스트 가능.
/// in-flight coalescing/머지/EmitGate/seq 증가를 담당한다.
async fn do_refresh_inner<F>(state: &Arc<AppState>, emit_fn: F) -> Result<(), String>
where
    F: FnOnce(RepoSnapshot),
{
    // 단일 in-flight 보장: 이미 진행 중이면 즉시 coalesce.
    if state.in_flight.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    // 어떤 경로(panic/early-return 포함)로 빠져나가도 in_flight를 false로 되돌린다.
    let _guard = InFlightGuard {
        flag: &state.in_flight,
    };

    let repos = state.repos.lock().await.clone();
    let now = now_epoch();
    let fresh = run_batch(repos, now, CONCURRENCY, GIT_TIMEOUT).await;
    let prev = state.last_snapshot.lock().await.clone();
    let merged = merge_failed_with_previous(&prev, fresh);
    let emit = should_emit(&prev, &merged);
    *state.last_snapshot.lock().await = merged.clone();

    if emit {
        let seq = state.seq.fetch_add(1, Ordering::SeqCst) + 1;
        emit_fn(RepoSnapshot { seq, repos: merged });
    }
    Ok(())
}

#[tauri::command]
pub async fn refresh_status(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    do_refresh(&app, state.inner()).await
}

#[tauri::command]
pub async fn open_action(
    state: State<'_, Arc<AppState>>,
    repo_path: String,
    kind: ActionKind,
) -> Result<(), String> {
    let terminal = state.config.lock().await.terminal_app.clone();
    actions::run_action(&kind, &repo_path, &terminal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::model::{RepoRef, RepoStatus};
    use std::cell::Cell;

    fn state_with_repos(repos: Vec<RepoRef>) -> Arc<AppState> {
        let st = AppState::new(Config::default());
        // 동기적으로 repos를 채운다(런타임 시작 전이라 try_lock으로 충분).
        *st.repos.try_lock().unwrap() = repos;
        Arc::new(st)
    }

    fn rref(path: &str) -> RepoRef {
        RepoRef {
            path: path.into(),
            name: "n".into(),
            category: "c".into(),
        }
    }

    /// in_flight=true이면 do_refresh_inner는 배치를 돌리지 않고 즉시 Ok(())로 coalesce되어야 한다.
    /// (emit_fn 미호출 + repos가 비어있지 않아도 last_snapshot 변화 없음)
    #[tokio::test]
    async fn coalesces_when_in_flight() {
        // 존재하지 않는 경로라도 in_flight 가드로 인해 배치 자체가 돌지 않아야 한다.
        let state = state_with_repos(vec![rref("/nonexistent/repo")]);
        state.in_flight.store(true, Ordering::SeqCst);

        let emitted = Cell::new(0u32);
        let res = do_refresh_inner(&state, |_snap| {
            emitted.set(emitted.get() + 1);
        })
        .await;

        assert!(res.is_ok());
        assert_eq!(emitted.get(), 0, "coalesce 시 emit이 호출되면 안 된다");
        // 배치를 돌지 않았으므로 last_snapshot은 그대로 비어 있어야 한다.
        assert!(state.last_snapshot.try_lock().unwrap().is_empty());
        // coalesce 경로는 in_flight를 건드리지 않으므로 여전히 true.
        assert!(state.in_flight.load(Ordering::SeqCst));
        // seq도 증가하지 않는다.
        assert_eq!(state.seq.load(Ordering::SeqCst), 0);
    }

    /// repos가 비어 있으면 신선 스냅샷도 비어 있고 직전(빈)과 동일 → emit 없음, seq 그대로.
    /// 또한 실행 후 in_flight는 RAII 가드로 false로 되돌아와야 한다.
    #[tokio::test]
    async fn no_emit_and_no_seq_when_unchanged() {
        let state = state_with_repos(vec![]);

        let emitted = Cell::new(0u32);
        let res = do_refresh_inner(&state, |_snap| {
            emitted.set(emitted.get() + 1);
        })
        .await;

        assert!(res.is_ok());
        assert_eq!(emitted.get(), 0, "변화가 없으면 emit 없음");
        assert_eq!(state.seq.load(Ordering::SeqCst), 0, "emit 없으면 seq 불변");
        assert!(
            !state.in_flight.load(Ordering::SeqCst),
            "정상 종료 후 in_flight는 가드로 false 복원"
        );
    }

    /// 변화가 있으면 emit이 일어나고 seq가 emit 시에만 +1 된다.
    /// 직전 스냅샷에 항목이 있는데 신선이 비어(len 변화) emit 발생 케이스로 검증한다.
    #[tokio::test]
    async fn seq_increments_only_on_emit() {
        let state = state_with_repos(vec![]);
        // 직전 스냅샷에 1개 항목을 심어두면, 빈 신선 스냅샷과 len이 달라 should_emit=true.
        *state.last_snapshot.try_lock().unwrap() = vec![RepoStatus::from_ref(&rref("/x"), 1)];

        let emitted_seq: Cell<Option<u64>> = Cell::new(None);
        let res = do_refresh_inner(&state, |snap| {
            emitted_seq.set(Some(snap.seq));
        })
        .await;

        assert!(res.is_ok());
        assert_eq!(emitted_seq.get(), Some(1), "첫 emit의 seq는 1");
        assert_eq!(state.seq.load(Ordering::SeqCst), 1, "emit 시에만 seq +1");
        assert!(
            !state.in_flight.load(Ordering::SeqCst),
            "정상 종료 후 in_flight false"
        );

        // 두 번째 호출: 이제 last_snapshot은 빈 상태로 갱신됐고 신선도 빈 상태 → 변화 없음 → emit/seq 불변.
        let emitted2 = Cell::new(0u32);
        let res2 = do_refresh_inner(&state, |_snap| {
            emitted2.set(emitted2.get() + 1);
        })
        .await;
        assert!(res2.is_ok());
        assert_eq!(emitted2.get(), 0, "변화 없으면 두 번째 emit 없음");
        assert_eq!(state.seq.load(Ordering::SeqCst), 1, "emit 없으면 seq 유지");
    }

    /// RAII 가드가 정상 경로에서 in_flight를 false로 되돌린 뒤, 후속 refresh가 다시 진행 가능한지.
    #[tokio::test]
    async fn guard_allows_subsequent_refresh() {
        let state = state_with_repos(vec![]);

        // 1회차
        let _ = do_refresh_inner(&state, |_| {}).await;
        assert!(!state.in_flight.load(Ordering::SeqCst));

        // 2회차: 가드로 풀렸으므로 coalesce되지 않고 실제로 진입해야 한다.
        // (진입 여부는 in_flight가 종료 후 false라는 점 + Ok로 확인)
        let res = do_refresh_inner(&state, |_| {}).await;
        assert!(res.is_ok());
        assert!(!state.in_flight.load(Ordering::SeqCst));
    }
}
