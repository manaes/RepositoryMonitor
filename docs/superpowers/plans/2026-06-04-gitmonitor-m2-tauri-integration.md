# GitMonitor M2 — Tauri 통합 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** M1 백엔드 코어(config/discovery/git_reader)를 Tauri 2 앱으로 통합 — AppState 관리, 5개 IPC 커맨드, `repos_updated` 이벤트(EmitGate+seq), 포커스 게이팅 폴링 스케줄러, 외부 앱 열기 액션. UI는 M3.

**Architecture:** 기존 `src-tauri` 라이브러리 크레이트를 Tauri 2 앱으로 전환(형제 AIAgentMonitor 패턴 적응). GitMonitor는 **단일 윈도우 그리드 전용**(트레이/팝오버 없음). 검증 가능한 순수 로직(EmitGate·실패머지·스케줄 판단·액션 argv·배치러너)을 별도 모듈로 분리해 단위/통합 테스트하고, `#[tauri::command]`와 `setup()`/window-event 글루는 `cargo build`(컴파일)로 검증한다. GUI 실행 자체는 이 환경에서 자동 검증 불가 — `cargo build` 성공 = 통합 OK 기준.

**Tech Stack:** Tauri 2 (`macos` desktop), tauri-plugin-dialog, tauri-plugin-single-instance, tokio(full), serde/serde_json, + M1 의존(dirs-next/globset/walkdir). 프론트는 정적 `dist/index.html` 플레이스홀더(실제 Svelte는 M3).

**Spec:** `docs/superpowers/specs/2026-06-04-git-monitor-dashboard-design.md` (§2 IPC 계약, §6 갱신/스케줄러, §8 기술결정)
**참조 패턴:** 형제 프로젝트 `/Users/wannypark/Desktop/@Projects/2_App/AIAgentMonitor` (Tauri 2 + Svelte 5)

---

## File Structure

| 파일 | 책임 |
|---|---|
| `src-tauri/Cargo.toml` | tauri/tauri-build/plugins/tokio 추가, `[lib] crate-type` 확장 |
| `src-tauri/build.rs` | `tauri_build::build()` |
| `src-tauri/tauri.conf.json` | 단일 `main` 윈도우, identifier `com.dgitx.gitmonitor` |
| `src-tauri/capabilities/default.json` | main 윈도우 권한(core/window/dialog) |
| `src-tauri/icons/*` | AIAgentMonitor에서 복제(임시) |
| `src-tauri/src/main.rs` | `fn main() { gitmonitor::run() }` |
| `src-tauri/src/model.rs` | (M1 확장) `ActionKind`, `RepoSnapshot`, `RepoStatus::from_ref` |
| `src-tauri/src/app_state.rs` | `AppState`(config/repos/last_snapshot/polling/in_flight/seq) |
| `src-tauri/src/emit_gate.rs` | `should_emit`(last_checked 제외 비교) |
| `src-tauri/src/snapshot.rs` | `merge_failed_with_previous`(실패 repo 직전값 유지) |
| `src-tauri/src/scheduler.rs` | `should_run_poll`(폴링 판단) |
| `src-tauri/src/actions.rs` | `open_argv`/`run_action`(process::Command) |
| `src-tauri/src/batch.rs` | `run_batch`(tokio semaphore + 5s timeout) |
| `src-tauri/src/commands.rs` | 5개 `#[tauri::command]` + `do_scan`/`do_refresh`/`now_epoch` |
| `src-tauri/src/lib.rs` | 모듈 선언 + `run()`(Builder 전체 배선) |
| `dist/index.html` | 정적 플레이스홀더(repo 루트; frontendDist `../dist` 기준. M3에서 Vite 산출물로 교체) |

**IPC 직렬화:** 이벤트 payload·커맨드 결과 구조체는 serde 기본 **snake_case** 유지(프론트 TS도 snake_case). 커맨드 인자만 Tauri가 camelCase 변환하므로 프론트 invoke 시 인자명 주의(M3).

---

## Task 0: Tauri 2 앱으로 전환 (스캐폴딩)

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/dist/index.html`
- Copy: `src-tauri/icons/` (from AIAgentMonitor)
- Modify: `src-tauri/src/lib.rs` (run() 스텁 추가)

- [ ] **Step 1: Cargo.toml 갱신 (의존성 + crate-type)**

`[package]` 아래에 `[lib]`/`[build-dependencies]`를 추가하고 `[dependencies]`에 tauri류를 추가한다. 최종 형태:

```toml
[package]
name = "gitmonitor"
version = "0.1.0"
edition = "2021"

[lib]
name = "gitmonitor"
crate-type = ["staticlib", "cdylib", "rlib"]
path = "src/lib.rs"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
tauri-plugin-single-instance = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
dirs-next = "2"
globset = "0.4"
walkdir = "2"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: build.rs 작성**

`src-tauri/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 3: tauri.conf.json 작성 (단일 main 윈도우)**

`src-tauri/tauri.conf.json`:
```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "GitMonitor",
  "version": "0.1.0",
  "identifier": "com.dgitx.gitmonitor",
  "build": {
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "GitMonitor",
        "width": 1000,
        "height": 700,
        "minWidth": 600,
        "minHeight": 400,
        "resizable": true,
        "visible": true
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 4: capabilities/default.json 작성**

`src-tauri/capabilities/default.json`:
```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Main window capability",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:default",
    "dialog:default"
  ]
}
```

- [ ] **Step 5: 아이콘 복제 (임시)**

```bash
cp -R "/Users/wannypark/Desktop/@Projects/2_App/AIAgentMonitor/src-tauri/icons" "/Users/wannypark/Desktop/@Projects/2_App/GitMonitor/src-tauri/icons"
ls "/Users/wannypark/Desktop/@Projects/2_App/GitMonitor/src-tauri/icons"
```
Expected: `32x32.png 128x128.png 128x128@2x.png icon.icns icon.ico ...` 가 보임. (실제 GitMonitor 아이콘은 후속 작업에서 교체)

- [ ] **Step 6: dist 플레이스홀더 생성 (repo 루트)**

`dist/index.html` — **repo 루트**에 둔다. `frontendDist: "../dist"`는 tauri.conf(src-tauri/) 기준이라 `GitMonitor/dist`를 가리킨다. `src-tauri/dist`가 아님. generate_context!()가 컴파일 시 이 경로 존재를 검증한다. (M3에서 Vite 산출물로 교체):
```html
<!doctype html>
<html lang="ko">
  <head><meta charset="UTF-8" /><title>GitMonitor</title></head>
  <body><div id="app">GitMonitor — M3에서 UI 구현 예정</div></body>
</html>
```

- [ ] **Step 7: main.rs 작성**

`src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    gitmonitor::run();
}
```

- [ ] **Step 8: lib.rs에 run() 스텁 추가**

`src-tauri/src/lib.rs` 를 다음으로 교체(기존 모듈 선언 유지 + run 스텁; 모듈/배선은 후속 태스크에서 확장):
```rust
pub mod config;
pub mod discovery;
pub mod git_reader;
pub mod model;

/// Tauri 앱 진입점. (커맨드/스케줄러 배선은 후속 태스크에서 채움)
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 9: 컴파일 + 기존 테스트 확인**

Run: `cd src-tauri && cargo build 2>&1 | tail -5`
Expected: 성공(첫 빌드는 tauri 의존 다운로드/컴파일로 수 분 소요).

Run: `cd src-tauri && cargo test --quiet 2>&1 | tail -3`
Expected: 기존 M1 테스트 35 passed 유지(통합 테스트는 rlib 링크로 그대로 동작).

- [ ] **Step 10: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/build.rs src-tauri/tauri.conf.json src-tauri/capabilities src-tauri/icons dist src-tauri/src/main.rs src-tauri/src/lib.rs
git commit -m "chore(m2): src-tauri를 Tauri 2 앱으로 전환 (단일 main 윈도우)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 1: model 확장 — ActionKind / RepoSnapshot / from_ref

**Files:**
- Modify: `src-tauri/src/model.rs`

- [ ] **Step 1: 실패 테스트 추가 (model.rs tests 모듈 안)**

```rust
    #[test]
    fn action_kind_serializes_snake_case() {
        assert_eq!(serde_json::to_string(&ActionKind::OpenFinder).unwrap(), "\"open_finder\"");
        assert_eq!(serde_json::to_string(&ActionKind::OpenSourceTree).unwrap(), "\"open_source_tree\"");
        let back: ActionKind = serde_json::from_str("\"open_terminal\"").unwrap();
        assert_eq!(back, ActionKind::OpenTerminal);
    }

    #[test]
    fn repo_snapshot_serializes() {
        let snap = RepoSnapshot { seq: 3, repos: vec![] };
        let j = serde_json::to_value(&snap).unwrap();
        assert_eq!(j["seq"], 3);
        assert!(j["repos"].is_array());
    }

    #[test]
    fn from_ref_builds_clean_placeholder() {
        let r = RepoRef { path: "/r".into(), name: "r".into(), category: "c".into() };
        let st = RepoStatus::from_ref(&r, 999);
        assert_eq!(st.path, "/r");
        assert_eq!(st.name, "r");
        assert_eq!(st.category, "c");
        assert_eq!(st.last_checked, 999);
        assert!(st.is_clean);
        assert_eq!(st.error, None);
        assert_eq!(st.ahead, None);
        assert_eq!(st.worktrees, 1);
        assert_eq!(st.state, RepoState::Clean);
    }
```

- [ ] **Step 2: 실패 확인**

Run: `cd src-tauri && cargo test --lib model`
Expected: 컴파일 에러(`ActionKind`/`RepoSnapshot`/`from_ref` 미정의).

- [ ] **Step 3: 구현 (model.rs 에 추가)**

`RepoStatus` 정의 아래에 추가:
```rust
/// 외부 앱 열기 액션 종류(프론트→백엔드 IPC). 경로 복사는 프론트 navigator.clipboard 담당이라 제외.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    OpenFinder,
    OpenTerminal,
    OpenSourceTree,
}

/// repos_updated 이벤트 payload. seq로 프론트가 오래된 스냅샷을 폐기.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoSnapshot {
    pub seq: u64,
    pub repos: Vec<RepoStatus>,
}

impl RepoStatus {
    /// RepoRef로부터 기본(clean) RepoStatus를 만든다. git 읽기 전/실패 시 베이스로 사용.
    pub fn from_ref(repo: &RepoRef, now: i64) -> Self {
        RepoStatus {
            path: repo.path.clone(),
            name: repo.name.clone(),
            category: repo.category.clone(),
            branch: None,
            detached_sha: None,
            upstream: None,
            has_upstream: false,
            ahead: None,
            behind: None,
            staged: 0,
            modified: 0,
            untracked: 0,
            conflicts: 0,
            stash: 0,
            is_clean: true,
            state: RepoState::Clean,
            worktrees: 1,
            last_fetch: None,
            last_checked: now,
            error: None,
        }
    }
}
```

- [ ] **Step 4: 통과 확인**

Run: `cd src-tauri && cargo test --lib model`
Expected: 기존 2 + 신규 3 = 5 passed.

- [ ] **Step 5: git_reader가 from_ref를 쓰도록 정리(중복 제거, 선택적 리팩토링)**

`src-tauri/src/git_reader.rs`의 `read_status` 시작부에서 거대한 `RepoStatus { ... }` 리터럴을 `let mut st = RepoStatus::from_ref(repo, now);`로 교체한다(필드 값 동일하므로 동작 불변). 교체 후:

Run: `cd src-tauri && cargo test --lib git_reader && cargo test --test git_reader_integration`
Expected: 기존 git_reader 테스트 전부 통과(동작 동일).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/model.rs src-tauri/src/git_reader.rs
git commit -m "feat(m2): model — ActionKind/RepoSnapshot/RepoStatus::from_ref

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: AppState

**Files:**
- Create: `src-tauri/src/app_state.rs`
- Modify: `src-tauri/src/lib.rs` (모듈 선언 추가)

- [ ] **Step 1: lib.rs에 모듈 선언 추가**

`src-tauri/src/lib.rs`의 `pub mod model;` 아래에 `pub mod app_state;` 추가.

- [ ] **Step 2: 실패 테스트 작성 (app_state.rs 하단)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn new_has_expected_initial_state() {
        let st = AppState::new(crate::config::Config::default());
        assert!(st.repos.lock().await.is_empty());
        assert!(st.last_snapshot.lock().await.is_empty());
        assert_eq!(st.polling_active.load(Ordering::SeqCst), false);
        assert_eq!(st.in_flight.load(Ordering::SeqCst), false);
        assert_eq!(st.seq.load(Ordering::SeqCst), 0);
        assert_eq!(st.config.lock().await.poll_interval_secs, 30);
    }
}
```

- [ ] **Step 3: 실패 확인**

Run: `cd src-tauri && cargo test --lib app_state`
Expected: 컴파일 에러(`AppState` 미정의).

- [ ] **Step 4: 구현 (app_state.rs 상단)**

```rust
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
```

- [ ] **Step 5: 통과 확인**

Run: `cd src-tauri && cargo test --lib app_state`
Expected: 1 passed.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/app_state.rs src-tauri/src/lib.rs
git commit -m "feat(m2): AppState (config/repos/snapshot 캐시/폴링·in-flight 플래그/seq)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: 순수 로직 — EmitGate / 실패 머지 / 스케줄 판단

**Files:**
- Create: `src-tauri/src/emit_gate.rs`, `src-tauri/src/snapshot.rs`, `src-tauri/src/scheduler.rs`
- Modify: `src-tauri/src/lib.rs` (모듈 선언)

- [ ] **Step 1: lib.rs에 모듈 선언 추가**

`pub mod app_state;` 아래에 추가:
```rust
pub mod emit_gate;
pub mod scheduler;
pub mod snapshot;
```

- [ ] **Step 2: 실패 테스트 작성 — emit_gate.rs**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RepoState, RepoStatus};

    fn s(now: i64, staged: u32) -> RepoStatus {
        let mut x = RepoStatus::from_ref(
            &crate::model::RepoRef { path: "/r".into(), name: "r".into(), category: "c".into() },
            now,
        );
        x.staged = staged;
        x.is_clean = staged == 0;
        x
    }

    #[test]
    fn same_except_last_checked_does_not_emit() {
        let a = vec![s(100, 0)];
        let b = vec![s(200, 0)]; // last_checked만 다름
        assert!(!should_emit(&a, &b));
    }

    #[test]
    fn changed_count_emits() {
        let a = vec![s(100, 0)];
        let b = vec![s(100, 2)];
        assert!(should_emit(&a, &b));
    }

    #[test]
    fn different_len_emits() {
        let a = vec![s(100, 0)];
        let b: Vec<RepoStatus> = vec![];
        assert!(should_emit(&a, &b));
    }
}
```

- [ ] **Step 3: 실패 확인**

Run: `cd src-tauri && cargo test --lib emit_gate`
Expected: 컴파일 에러(`should_emit` 미정의).

- [ ] **Step 4: 구현 — emit_gate.rs (상단)**

```rust
use crate::model::RepoStatus;

/// last_checked만 무시하고 두 RepoStatus가 의미상 동일한지.
fn eq_ignoring_time(a: &RepoStatus, b: &RepoStatus) -> bool {
    let mut a2 = a.clone();
    let mut b2 = b.clone();
    a2.last_checked = 0;
    b2.last_checked = 0;
    a2 == b2
}

/// 직전 스냅샷 대비 변경이 있으면 true(emit). last_checked 변화만으로는 emit 안 함.
pub fn should_emit(prev: &[RepoStatus], next: &[RepoStatus]) -> bool {
    if prev.len() != next.len() {
        return true;
    }
    !prev.iter().zip(next).all(|(a, b)| eq_ignoring_time(a, b))
}
```

- [ ] **Step 5: 통과 확인**

Run: `cd src-tauri && cargo test --lib emit_gate`
Expected: 3 passed.

- [ ] **Step 6: 실패 테스트 작성 — snapshot.rs**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RepoRef, RepoStatus};

    fn rref(path: &str) -> RepoRef {
        RepoRef { path: path.into(), name: "n".into(), category: "c".into() }
    }

    #[test]
    fn failed_repo_keeps_previous_values() {
        // 직전: 정상(staged=3, branch=main)
        let mut prev_x = RepoStatus::from_ref(&rref("/x"), 100);
        prev_x.staged = 3;
        prev_x.branch = Some("main".into());
        let prev = vec![prev_x];

        // 신선: /x 가 error
        let mut fresh_x = RepoStatus::from_ref(&rref("/x"), 200);
        fresh_x.error = Some("git 실패".into());
        let fresh = vec![fresh_x];

        let merged = merge_failed_with_previous(&prev, fresh);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].staged, 3);                   // 직전 값 유지
        assert_eq!(merged[0].branch.as_deref(), Some("main"));
        assert_eq!(merged[0].error.as_deref(), Some("git 실패")); // error는 신선 값
        assert_eq!(merged[0].last_checked, 200);           // last_checked는 신선 값
    }

    #[test]
    fn ok_repo_passes_through() {
        let mut fresh_x = RepoStatus::from_ref(&rref("/x"), 200);
        fresh_x.staged = 5;
        let merged = merge_failed_with_previous(&[], vec![fresh_x]);
        assert_eq!(merged[0].staged, 5);
        assert_eq!(merged[0].error, None);
    }

    #[test]
    fn failed_repo_with_no_previous_stays_error() {
        let mut fresh_x = RepoStatus::from_ref(&rref("/new"), 200);
        fresh_x.error = Some("e".into());
        let merged = merge_failed_with_previous(&[], vec![fresh_x]);
        assert_eq!(merged[0].error.as_deref(), Some("e"));
    }
}
```

- [ ] **Step 7: 실패 확인**

Run: `cd src-tauri && cargo test --lib snapshot`
Expected: 컴파일 에러(`merge_failed_with_previous` 미정의).

- [ ] **Step 8: 구현 — snapshot.rs (상단)**

```rust
use crate::model::RepoStatus;

/// 신선 스냅샷에서 error가 난 repo는, 직전 스냅샷에 같은 path의 정상 항목이 있으면
/// 그 수치 필드를 유지하되 error/last_checked는 신선 값으로 덮어 머지한다.
pub fn merge_failed_with_previous(prev: &[RepoStatus], fresh: Vec<RepoStatus>) -> Vec<RepoStatus> {
    fresh
        .into_iter()
        .map(|f| {
            if f.error.is_some() {
                if let Some(p) = prev.iter().find(|p| p.path == f.path && p.error.is_none()) {
                    let mut merged = p.clone();
                    merged.error = f.error;
                    merged.last_checked = f.last_checked;
                    return merged;
                }
            }
            f
        })
        .collect()
}
```

- [ ] **Step 9: 통과 확인**

Run: `cd src-tauri && cargo test --lib snapshot`
Expected: 3 passed.

- [ ] **Step 10: 실패 테스트 작성 — scheduler.rs**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polls_only_when_active_and_idle() {
        assert!(should_run_poll(true, false));   // 포커스 O, in-flight X → 폴링
        assert!(!should_run_poll(false, false));  // 비포커스 → 스킵
        assert!(!should_run_poll(true, true));    // 이미 진행 중 → 스킵
        assert!(!should_run_poll(false, true));
    }
}
```

- [ ] **Step 11: 실패 확인 → 구현 — scheduler.rs (상단)**

Run 먼저: `cd src-tauri && cargo test --lib scheduler` → 컴파일 에러 확인. 그 후 구현:
```rust
/// 폴링 tick에서 상태 배치를 돌려야 하는가: 창이 포커스 상태이고 진행 중 배치가 없을 때만.
pub fn should_run_poll(polling_active: bool, in_flight: bool) -> bool {
    polling_active && !in_flight
}
```

- [ ] **Step 12: 통과 확인**

Run: `cd src-tauri && cargo test --lib emit_gate --lib snapshot --lib scheduler`
Expected: emit_gate 3 + snapshot 3 + scheduler 1 = 7 passed (또는 개별 실행 합산).

- [ ] **Step 13: Commit**

```bash
git add src-tauri/src/emit_gate.rs src-tauri/src/snapshot.rs src-tauri/src/scheduler.rs src-tauri/src/lib.rs
git commit -m "feat(m2): 순수 로직 — EmitGate(시각 제외 비교)/실패 머지/폴링 판단 + 테스트

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: actions — 외부 앱 열기

**Files:**
- Create: `src-tauri/src/actions.rs`
- Modify: `src-tauri/src/lib.rs` (모듈 선언)

- [ ] **Step 1: lib.rs에 `pub mod actions;` 추가**

- [ ] **Step 2: 실패 테스트 작성 — actions.rs 하단**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TerminalApp;
    use crate::model::ActionKind;

    #[test]
    fn finder_argv() {
        let (prog, args) = open_argv(&ActionKind::OpenFinder, "/repo", &TerminalApp::Terminal);
        assert_eq!(prog, "open");
        assert_eq!(args, vec!["/repo".to_string()]);
    }

    #[test]
    fn sourcetree_argv() {
        let (prog, args) = open_argv(&ActionKind::OpenSourceTree, "/repo", &TerminalApp::Terminal);
        assert_eq!(prog, "open");
        assert_eq!(args, vec!["-a".to_string(), "SourceTree".to_string(), "/repo".to_string()]);
    }

    #[test]
    fn terminal_argv_variants() {
        let (_, a1) = open_argv(&ActionKind::OpenTerminal, "/r", &TerminalApp::Terminal);
        assert_eq!(a1, vec!["-a".to_string(), "Terminal".to_string(), "/r".to_string()]);
        let (_, a2) = open_argv(&ActionKind::OpenTerminal, "/r", &TerminalApp::Iterm);
        assert_eq!(a2, vec!["-a".to_string(), "iTerm".to_string(), "/r".to_string()]);
        let (_, a3) = open_argv(&ActionKind::OpenTerminal, "/r", &TerminalApp::Custom("/Applications/Foo.app".into()));
        assert_eq!(a3, vec!["-a".to_string(), "/Applications/Foo.app".to_string(), "/r".to_string()]);
    }
}
```

- [ ] **Step 3: 실패 확인 → 구현 — actions.rs (상단)**

Run 먼저: `cd src-tauri && cargo test --lib actions` → 컴파일 에러. 그 후:
```rust
use crate::config::TerminalApp;
use crate::model::ActionKind;

/// macOS `open` 명령의 (program, args)를 구성. 순수 함수라 단위테스트 가능.
pub fn open_argv(kind: &ActionKind, path: &str, terminal: &TerminalApp) -> (String, Vec<String>) {
    match kind {
        ActionKind::OpenFinder => ("open".to_string(), vec![path.to_string()]),
        ActionKind::OpenSourceTree => (
            "open".to_string(),
            vec!["-a".to_string(), "SourceTree".to_string(), path.to_string()],
        ),
        ActionKind::OpenTerminal => {
            let app = match terminal {
                TerminalApp::Terminal => "Terminal",
                TerminalApp::Iterm => "iTerm",
                TerminalApp::Custom(s) => s.as_str(),
            };
            (
                "open".to_string(),
                vec!["-a".to_string(), app.to_string(), path.to_string()],
            )
        }
    }
}

/// 실제 외부 앱 실행. 실패(앱 미설치 등)는 Err(String).
pub fn run_action(kind: &ActionKind, path: &str, terminal: &TerminalApp) -> Result<(), String> {
    let (prog, args) = open_argv(kind, path, terminal);
    std::process::Command::new(prog)
        .args(args)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("외부 앱 실행 실패: {e}"))
}
```

- [ ] **Step 4: 통과 확인**

Run: `cd src-tauri && cargo test --lib actions`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/actions.rs src-tauri/src/lib.rs
git commit -m "feat(m2): actions — open_argv/run_action(process::Command, Finder/터미널/SourceTree)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: batch — 비동기 상태 배치 (semaphore + timeout)

**Files:**
- Create: `src-tauri/src/batch.rs`
- Create: `src-tauri/tests/batch_integration.rs`
- Modify: `src-tauri/src/lib.rs` (모듈 선언)

- [ ] **Step 1: lib.rs에 `pub mod batch;` 추가**

- [ ] **Step 2: 통합 테스트 작성 — tests/batch_integration.rs**

```rust
use gitmonitor::batch::run_batch;
use gitmonitor::model::RepoRef;
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
    RepoRef { path: dir.to_string_lossy().into_owned(), name: "r".into(), category: "c".into() }
}

#[tokio::test]
async fn runs_all_repos_and_marks_missing_as_error() {
    let d1 = tempfile::tempdir().unwrap();
    let d2 = tempfile::tempdir().unwrap();
    let r1 = init_repo(d1.path());
    let r2 = init_repo(d2.path());
    let missing = RepoRef { path: "/no/such/repo/zzz".into(), name: "x".into(), category: "c".into() };

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
```

- [ ] **Step 3: 실패 확인 → 구현 — batch.rs (상단)**

Run 먼저: `cd src-tauri && cargo test --test batch_integration` → 컴파일 에러. 그 후:
```rust
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
```

- [ ] **Step 4: 통과 확인**

Run: `cd src-tauri && cargo test --test batch_integration`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/batch.rs src-tauri/tests/batch_integration.rs src-tauri/src/lib.rs
git commit -m "feat(m2): batch — 비동기 상태 배치(semaphore 동시상한 + per-repo 5s timeout)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: commands — IPC 커맨드 + do_scan/do_refresh

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (모듈 선언)

> 이 태스크의 함수 대부분은 `#[tauri::command]` 글루다. 핵심 로직(discover/run_batch/merge/should_emit)은 Task 3·5에서 이미 테스트됨. 검증 기준은 **`cargo build` 컴파일 통과**다(커맨드는 Tauri 런타임 없이는 직접 호출 테스트 어려움).

- [ ] **Step 1: lib.rs에 `pub mod commands;` 추가**

- [ ] **Step 2: 구현 — commands.rs**

```rust
use crate::app_state::AppState;
use crate::batch::run_batch;
use crate::config::{self, Config};
use crate::discovery::{discover, DiscoveryConfig};
use crate::emit_gate::should_emit;
use crate::model::{ActionKind, RepoRef, RepoSnapshot};
use crate::snapshot::merge_failed_with_previous;
use crate::actions;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

const CONCURRENCY: usize = 8;
const GIT_TIMEOUT: Duration = Duration::from_secs(5);

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
    // 단일 in-flight 보장
    if state.in_flight.swap(true, Ordering::SeqCst) {
        return Ok(()); // 이미 진행 중 → coalesce
    }
    let repos = state.repos.lock().await.clone();
    let now = now_epoch();
    let fresh = run_batch(repos, now, CONCURRENCY, GIT_TIMEOUT).await;
    let prev = state.last_snapshot.lock().await.clone();
    let merged = merge_failed_with_previous(&prev, fresh);
    let emit = should_emit(&prev, &merged);
    *state.last_snapshot.lock().await = merged.clone();
    state.in_flight.store(false, Ordering::SeqCst);

    if emit {
        let seq = state.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = app.emit("repos_updated", RepoSnapshot { seq, repos: merged });
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
```

> 주의: `commands.rs`는 `scheduler`를 import하지 않는다(should_run_poll은 lib.rs 폴링 루프에서만 사용). import는 실제 사용 모듈에만 둔다.

- [ ] **Step 3: 컴파일 확인**

Run: `cd src-tauri && cargo build 2>&1 | tail -5`
Expected: 성공. (커맨드 시그니처가 Tauri 2 State/AppHandle 주입과 일치)

Run: `cd src-tauri && cargo test --quiet 2>&1 | tail -3`
Expected: 기존 테스트 전부 통과 유지.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(m2): IPC 커맨드 5종 + do_scan/do_refresh(in-flight·머지·EmitGate·seq)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: lib.rs run() 전체 배선 + 최종 검증

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: lib.rs run() 전체 구현으로 교체**

`src-tauri/src/lib.rs` 전체를 다음으로 교체(모듈 선언 + 완성된 run):
```rust
pub mod actions;
pub mod app_state;
pub mod batch;
pub mod commands;
pub mod config;
pub mod discovery;
pub mod emit_gate;
pub mod git_reader;
pub mod model;
pub mod scheduler;
pub mod snapshot;

use app_state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// Tauri 앱 진입점: AppState 관리, 커맨드 등록, 창 포커스 게이팅 + 폴링 루프 배선.
pub fn run() {
    let cfg = config::load_from(&config::config_path());
    let state = Arc::new(AppState::new(cfg));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::scan_repos,
            commands::refresh_status,
            commands::open_action,
        ])
        .setup(move |app| {
            use tauri::Manager;
            let handle = app.handle().clone();

            // 창 포커스/블러 → 폴링 활성 토글
            if let Some(win) = app.get_webview_window("main") {
                let st = state.clone();
                win.on_window_event(move |ev| {
                    if let tauri::WindowEvent::Focused(focused) = ev {
                        st.polling_active.store(*focused, Ordering::SeqCst);
                    }
                });
            }

            // 백그라운드 폴링 루프
            let st = state.clone();
            tauri::async_runtime::spawn(async move {
                // 시작 시 1회: 스캔 + 상태 읽기 (포커스 게이팅과 무관하게 초기 표시)
                let _ = commands::do_scan(&st).await;
                let _ = commands::do_refresh(&handle, &st).await;

                loop {
                    let interval = st.config.lock().await.poll_interval_secs.clamp(5, 300) as u64;
                    tokio::time::sleep(Duration::from_secs(interval)).await;
                    if scheduler::should_run_poll(
                        st.polling_active.load(Ordering::SeqCst),
                        st.in_flight.load(Ordering::SeqCst),
                    ) {
                        let _ = commands::do_refresh(&handle, &st).await;
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 2: (확인) 미사용 import 없음**

lib.rs 교체 후 `cd src-tauri && cargo build`로 미사용 import 경고가 없는지 확인. 경고가 있으면 해당 import만 정리(동작 변경 금지).

- [ ] **Step 3: 전체 빌드 + 테스트 + clippy**

Run: `cd src-tauri && cargo build 2>&1 | tail -5`
Expected: 성공(Tauri 앱 전체 컴파일).

Run: `cd src-tauri && cargo test --quiet 2>&1 | tail -3`
Expected: 전체 테스트 통과(M1 35 + M2 신규: model 3, app_state 1, emit_gate 3, snapshot 3, scheduler 1, actions 3, batch 2 = 약 51).

Run: `cd src-tauri && cargo clippy --all-targets -- -D warnings 2>&1 | tail -6`
Expected: No issues found, exit 0.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands.rs
git commit -m "feat(m2): run() 전체 배선 — manage/커맨드 등록/포커스 게이팅/폴링 루프

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## M2 완료 기준 (Definition of Done)
- `cargo build` 성공(Tauri 2 앱 전체 컴파일 — IPC 커맨드/이벤트/setup 글루 통합 검증).
- `cargo test` 전부 통과(M1 + M2 순수 로직/배치 신규 테스트).
- `cargo clippy --all-targets -- -D warnings` 무경고.
- AppState/EmitGate/실패머지/폴링판단/액션argv/배치러너가 단위·통합 테스트로 검증됨.
- ⚠️ GUI 실행은 이 환경에서 자동 검증 불가 — 사용자가 `pnpm install` 후 `cargo tauri dev`(M3에서 vite 배선 완료 시) 또는 임시로 `cargo run`(정적 dist 플레이스홀더 창)으로 수동 확인.
- M3(Svelte 그리드 UI: store/Grid/RepoCard/Header/Settings/EmptyState/액션·dialog/카드 신호 렌더)는 별도 계획서.

---

## Self-Review (작성자 점검 결과)
- **Spec 커버리지(M2)**: §2 IPC 5커맨드+repos_updated(EmitGate+seq) ✅(Task 6), AppState ✅(Task 2), 포커스 게이팅 폴링+단일 in-flight ✅(Task 3 판단/Task 7 배선), 실패 repo 직전값 유지 ✅(Task 3), actions process::Command ✅(Task 4), 5s timeout+동시성 ✅(Task 5), Tauri 앱 전환(단일 윈도우/capabilities/plugins) ✅(Task 0). 타입 ActionKind/RepoSnapshot ✅(Task 1).
- **Placeholder 스캔**: 모든 코드 스텝에 실제 코드/명령/기대값. Task 0의 dist/icons는 임시 명시(M3/후속 교체). Task 6의 `#[allow(unused_imports)]`는 Task 7에서 제거되는 임시 장치로 명시.
- **타입 일관성**: `AppState`(Arc로 manage), 커맨드는 `State<'_, Arc<AppState>>`+`AppHandle` 주입, `do_scan(&Arc<AppState>)`/`do_refresh(&AppHandle,&Arc<AppState>)` 시그니처가 Task 6 정의와 Task 7 호출부 일치. `run_batch(Vec<RepoRef>, i64, usize, Duration)`, `merge_failed_with_previous(&[RepoStatus], Vec<RepoStatus>)`, `should_emit(&[RepoStatus],&[RepoStatus])`, `should_run_poll(bool,bool)`, `open_argv(&ActionKind,&str,&TerminalApp)`, `RepoStatus::from_ref(&RepoRef,i64)` 호출부/정의부 일치. 이벤트명 `repos_updated`/payload `RepoSnapshot{seq,repos}` 일관.
