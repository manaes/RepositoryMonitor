# GitMonitor M1 — 백엔드 코어 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** GitMonitor의 UI-독립 백엔드 코어(설정 영속화 · git repo 발견 · git 상태 읽기)를 TDD로 구현하고 `cargo test`로 완전 검증한다.

**Architecture:** `src-tauri`에 **순수 Rust 라이브러리 크레이트**(Tauri 의존 없음)를 만든다. 모듈은 책임별로 분리: `model`(공유 직렬화 타입) · `config`(설정 로드/저장) · `discovery`(.git 디렉토리 탐색 + 제외 글롭 + 카테고리) · `git_reader`(`git status --porcelain=v2` 파싱 + stash/state/worktree/fetch). 파싱 로직은 순수 함수(시간/IO 없음)로 분리해 픽스처 단위테스트하고, 실제 git을 호출하는 부분은 임시 repo 통합테스트로 검증. **타임아웃·동시성·tokio·Tauri 커맨드/이벤트는 M2 범위**다(여기선 동기 함수로 정확성만 확보).

**Tech Stack:** Rust 2021, serde/serde_json(직렬화), dirs_next(설정 경로), globset(제외 글롭), walkdir(디렉토리 탐색), tempfile(테스트). git은 시스템 `git` CLI를 `std::process::Command`로 호출.

**Spec:** `docs/superpowers/specs/2026-06-04-git-monitor-dashboard-design.md` (§3 데이터 타입, §4 git 읽기, §5 발견/등록, §10 테스트)

---

## File Structure

| 파일 | 책임 |
|---|---|
| `src-tauri/Cargo.toml` | 크레이트 메타 + 의존성 (M1은 tauri 없음; M2에서 추가) |
| `src-tauri/src/lib.rs` | 모듈 선언만 (`pub mod ...`) |
| `src-tauri/src/model.rs` | `RepoRef`, `RepoState`, `RepoStatus` (프론트와 공유될 직렬화 타입, snake_case) |
| `src-tauri/src/config.rs` | `Config`, `TerminalApp` + 경로 산출 + 로드/저장(버저닝·백업) |
| `src-tauri/src/discovery.rs` | 제외 글롭 빌드 + 카테고리 산출 + `.git` 디렉토리 판정 + `discover()` |
| `src-tauri/src/git_reader.rs` | `parse_porcelain_v2()` 순수 파서 + stash/state/worktree/fetch 헬퍼 + `read_status()` |
| `src-tauri/tests/git_reader_integration.rs` | 실제 임시 git repo로 `read_status` 통합테스트 |
| `src-tauri/tests/discovery_integration.rs` | 실제 임시 디렉토리 트리로 `discover` 통합테스트 |
| `src-tauri/.gitignore` | `/target` |

**직렬화 방침(spec §2):** 이벤트 payload는 자동 camelCase 변환이 없으므로 구조체 필드는 Rust 그대로 **snake_case**로 직렬화한다(rename 속성 없음). enum은 `#[serde(rename_all = "snake_case")]`로 소문자 문자열 직렬화.

---

## Task 0: 크레이트 초기화

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/lib.rs`
- Create: `src-tauri/.gitignore`

- [ ] **Step 1: Cargo.toml 작성**

```toml
[package]
name = "gitmonitor"
version = "0.1.0"
edition = "2021"

[lib]
name = "gitmonitor"
path = "src/lib.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dirs_next = "2"
globset = "0.4"
walkdir = "2"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: lib.rs 작성 (빈 모듈 선언)**

```rust
pub mod config;
pub mod discovery;
pub mod git_reader;
pub mod model;
```

- [ ] **Step 3: .gitignore 작성**

```
/target
```

- [ ] **Step 4: 빈 모듈 파일 생성(컴파일만 통과용)**

```bash
cd src-tauri && : > src/model.rs && : > src/config.rs && : > src/discovery.rs && : > src/git_reader.rs
```

- [ ] **Step 5: 빌드 확인**

Run: `cd src-tauri && cargo build`
Expected: 성공(경고 가능). 빈 모듈이라 에러 없음.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/lib.rs src-tauri/src/*.rs src-tauri/.gitignore
git commit -m "chore(m1): src-tauri 라이브러리 크레이트 초기화"
```

---

## Task 1: model.rs — 공유 타입

**Files:**
- Modify: `src-tauri/src/model.rs`

- [ ] **Step 1: 실패 테스트 작성 (model.rs 하단)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repostatus_serializes_snake_case_fields() {
        let s = RepoStatus {
            path: "/r".into(), name: "r".into(), category: "2_App".into(),
            branch: Some("main".into()), detached_sha: None, upstream: Some("origin/main".into()),
            has_upstream: true, ahead: Some(1), behind: Some(0),
            staged: 2, modified: 1, untracked: 1, conflicts: 0, stash: 0,
            is_clean: false, state: RepoState::Clean, worktrees: 1,
            last_fetch: Some(1_700_000_000), last_checked: 1_700_000_100, error: None,
        };
        let j = serde_json::to_value(&s).unwrap();
        assert_eq!(j["has_upstream"], true);
        assert_eq!(j["detached_sha"], serde_json::Value::Null);
        assert_eq!(j["state"], "clean"); // enum snake_case
    }

    #[test]
    fn reporef_roundtrips() {
        let r = RepoRef { path: "/r".into(), name: "r".into(), category: "lib".into() };
        let back: RepoRef = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(r, back);
    }
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cd src-tauri && cargo test --lib model`
Expected: 컴파일 에러(`RepoStatus`/`RepoRef`/`RepoState` 미정의).

- [ ] **Step 3: 타입 정의 (model.rs 상단)**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoRef {
    pub path: String,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoState {
    #[default]
    Clean,
    Merging,
    Rebasing,
    CherryPicking,
    Reverting,
    Bisecting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoStatus {
    pub path: String,
    pub name: String,
    pub category: String,
    pub branch: Option<String>,
    pub detached_sha: Option<String>,
    pub upstream: Option<String>,
    pub has_upstream: bool,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
    pub conflicts: u32,
    pub stash: u32,
    pub is_clean: bool,
    pub state: RepoState,
    pub worktrees: u32,
    pub last_fetch: Option<i64>,
    pub last_checked: i64,
    pub error: Option<String>,
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test --lib model`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/model.rs
git commit -m "feat(m1): model 타입(RepoRef/RepoState/RepoStatus) + 직렬화 테스트"
```

---

## Task 2: config.rs — Config/TerminalApp 타입 + forward-compat

**Files:**
- Modify: `src-tauri/src/config.rs`

- [ ] **Step 1: 실패 테스트 작성 (config.rs 하단)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let c = Config::default();
        assert_eq!(c.version, 1);
        assert_eq!(c.poll_interval_secs, 30);
        assert_eq!(c.scan_depth, 4);
        assert_eq!(c.stale_fetch_days, 7);
        assert_eq!(c.terminal_app, TerminalApp::Terminal);
        assert!(c.roots.is_empty());
    }

    #[test]
    fn partial_json_fills_defaults() {
        // 일부 필드만 있는 JSON → 누락 필드는 기본값
        let json = r#"{ "roots": ["/a"], "poll_interval_secs": 60 }"#;
        let c: Config = serde_json::from_str(json).unwrap();
        assert_eq!(c.roots, vec!["/a".to_string()]);
        assert_eq!(c.poll_interval_secs, 60);
        assert_eq!(c.scan_depth, 4);   // 기본값으로 채워짐
        assert_eq!(c.version, 1);
    }

    #[test]
    fn unknown_fields_ignored() {
        let json = r#"{ "roots": [], "future_field": 123 }"#;
        let c: Config = serde_json::from_str(json).unwrap();
        assert!(c.roots.is_empty());
    }

    #[test]
    fn terminal_app_serializes_snake_case() {
        assert_eq!(serde_json::to_string(&TerminalApp::Iterm).unwrap(), "\"iterm\"");
        let custom = TerminalApp::Custom("/Applications/Foo.app".into());
        let j = serde_json::to_value(&custom).unwrap();
        assert_eq!(j["custom"], "/Applications/Foo.app");
    }
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cd src-tauri && cargo test --lib config`
Expected: 컴파일 에러(`Config`/`TerminalApp` 미정의).

- [ ] **Step 3: 타입 정의 (config.rs 상단)**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalApp {
    Terminal,
    Iterm,
    Custom(String),
}

impl Default for TerminalApp {
    fn default() -> Self {
        TerminalApp::Terminal
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)] // 누락 필드는 Default로 채움 + 알 수 없는 필드는 무시(deny_unknown_fields 미사용)
pub struct Config {
    pub version: u32,
    pub roots: Vec<String>,
    pub manual_paths: Vec<String>,
    pub exclude_globs: Vec<String>,
    pub poll_interval_secs: u32,
    pub scan_depth: u32,
    pub stale_fetch_days: u32,
    pub terminal_app: TerminalApp,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: 1,
            roots: Vec::new(),
            manual_paths: Vec::new(),
            exclude_globs: Vec::new(),
            poll_interval_secs: 30,
            scan_depth: 4,
            stale_fetch_days: 7,
            terminal_app: TerminalApp::Terminal,
        }
    }
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test --lib config`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/config.rs
git commit -m "feat(m1): Config/TerminalApp 타입 + forward-compat 역직렬화 테스트"
```

---

## Task 3: config.rs — 경로 산출 + 로드/저장(백업 재생성)

**Files:**
- Modify: `src-tauri/src/config.rs`

- [ ] **Step 1: 실패 테스트 추가 (config.rs tests 모듈 안)**

```rust
    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut c = Config::default();
        c.roots = vec!["/x".into()];
        c.poll_interval_secs = 45;
        save_to(&path, &c).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded, c);
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        assert_eq!(load_from(&path), Config::default());
    }

    #[test]
    fn corrupt_file_backs_up_and_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, b"{ this is not json").unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded, Config::default());
        // 손상 파일은 .bak로 백업됨
        assert!(path.with_extension("json.bak").exists());
    }

    #[test]
    fn config_path_ends_with_expected() {
        let p = config_path();
        assert!(p.ends_with("GitMonitor/config.json"));
    }
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cd src-tauri && cargo test --lib config`
Expected: 컴파일 에러(`save_to`/`load_from`/`config_path` 미정의).

- [ ] **Step 3: 함수 구현 (config.rs, 타입 정의 아래)**

```rust
use std::path::{Path, PathBuf};

/// 설정 파일 경로: 플랫폼 config 디렉토리 하위 GitMonitor/config.json
pub fn config_path() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("GitMonitor")
        .join("config.json")
}

/// 파일에서 Config 로드. 없으면 기본값. 파싱 실패 시 .bak 백업 후 기본값 재생성.
pub fn load_from(path: &Path) -> Config {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return Config::default(), // 파일 없음/읽기 실패 → 기본값
    };
    match serde_json::from_str::<Config>(&text) {
        Ok(c) => c,
        Err(_) => {
            // 손상 파일 백업 후 기본값
            let bak = path.with_extension("json.bak");
            let _ = std::fs::rename(path, &bak);
            Config::default()
        }
    }
}

/// Config를 파일에 저장(부모 디렉토리 자동 생성, pretty JSON).
pub fn save_to(path: &Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, text)
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test --lib config`
Expected: 8 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/config.rs
git commit -m "feat(m1): config 경로 산출 + 로드/저장(손상 시 백업 재생성)"
```

---

## Task 4: discovery.rs — 제외 글롭 빌드 + 매칭

**Files:**
- Modify: `src-tauri/src/discovery.rs`

- [ ] **Step 1: 실패 테스트 작성 (discovery.rs 하단)**

```rust
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
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cd src-tauri && cargo test --lib discovery`
Expected: 컴파일 에러(`build_exclude_set` 미정의).

- [ ] **Step 3: 구현 (discovery.rs 상단)**

```rust
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
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test --lib discovery`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/discovery.rs
git commit -m "feat(m1): 제외 글롭 빌드/매칭(절대·상대·대소문자무시)"
```

---

## Task 5: discovery.rs — 카테고리 산출 + .git 디렉토리 판정

**Files:**
- Modify: `src-tauri/src/discovery.rs`

- [ ] **Step 1: 실패 테스트 추가 (discovery.rs tests 모듈 안)**

```rust
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
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cd src-tauri && cargo test --lib discovery`
Expected: 컴파일 에러(`category_for`/`is_git_repo_dir` 미정의).

- [ ] **Step 3: 구현 (discovery.rs, build_exclude_set 아래)**

```rust
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
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test --lib discovery`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/discovery.rs
git commit -m "feat(m1): 카테고리 산출 + .git 디렉토리 판정"
```

---

## Task 6: discovery.rs — discover() 통합

**Files:**
- Modify: `src-tauri/src/discovery.rs`
- Create: `src-tauri/tests/discovery_integration.rs`

- [ ] **Step 1: 통합 테스트 작성 (tests/discovery_integration.rs)**

```rust
use gitmonitor::discovery::{discover, DiscoveryConfig};
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
    mk_repo(root, "2_App/GitMonitor");
    mk_repo(root, "@ITXRtsp/edge-client-swift");
    // node_modules 안의 .git은 prune되어 잡히면 안 됨
    fs::create_dir_all(root.join("2_App/GitMonitor/node_modules/dep/.git")).unwrap();
    // 깊이 초과 repo (depth=4 기준 5단계) — 잡히면 안 됨
    mk_repo(root, "a/b/c/d/tooDeep");

    let roots = vec![root.to_string_lossy().into_owned()];
    let cfg = DiscoveryConfig {
        roots: &roots,
        manual_paths: &[],
        exclude_globs: &[],
        scan_depth: 4,
    };
    let mut found: Vec<(String, String)> =
        discover(&cfg).into_iter().map(|r| (r.name, r.category)).collect();
    found.sort();

    assert!(found.contains(&("GitMonitor".to_string(), "2_App".to_string())));
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
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cd src-tauri && cargo test --test discovery_integration`
Expected: 컴파일 에러(`DiscoveryConfig`/`discover` 미정의).

- [ ] **Step 3: 구현 (discovery.rs, 하단 tests 모듈 위)**

```rust
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
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test --test discovery_integration`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/discovery.rs src-tauri/tests/discovery_integration.rs
git commit -m "feat(m1): discover() — 루트 스캔/수동경로/제외/카테고리/dedup"
```

---

## Task 7: git_reader.rs — parse_porcelain_v2 순수 파서 (실측 픽스처)

**Files:**
- Modify: `src-tauri/src/git_reader.rs`

> 아래 픽스처는 실제 `git status --porcelain=v2 --branch` 출력을 캡처한 것이다(설계 단계 실측). 그대로 사용한다.

- [ ] **Step 1: 실패 테스트 작성 (git_reader.rs 하단)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_in_sync() {
        let txt = "\
# branch.oid 14bbe8869730251fa97f51cfae8ccdf81d6337ff
# branch.head master
# branch.upstream origin/main
# branch.ab +0 -0
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.branch.as_deref(), Some("master"));
        assert_eq!(p.detached_sha, None);
        assert_eq!(p.upstream.as_deref(), Some("origin/main"));
        assert!(p.has_upstream);
        assert_eq!(p.ahead, Some(0));
        assert_eq!(p.behind, Some(0));
        assert_eq!((p.staged, p.modified, p.untracked, p.conflicts), (0, 0, 0, 0));
        assert!(p.is_clean);
    }

    #[test]
    fn dirty_mm_staged_untracked() {
        let txt = "\
# branch.oid 14bbe8869730251fa97f51cfae8ccdf81d6337ff
# branch.head master
# branch.upstream origin/main
# branch.ab +0 -0
1 MM N... 100644 100644 100644 7898192261 9ad2ebbaff a.txt
1 A. N... 000000 100644 100644 0000000000 b4785957bc staged.txt
? untracked.txt
";
        let p = parse_porcelain_v2(txt);
        // MM: X=M(staged), Y=M(modified) / A.: X=A(staged), Y=.(아님)
        assert_eq!(p.staged, 2);
        assert_eq!(p.modified, 1);
        assert_eq!(p.untracked, 1);
        assert_eq!(p.conflicts, 0);
        assert!(!p.is_clean);
    }

    #[test]
    fn no_upstream_has_no_ab_line() {
        let txt = "\
# branch.oid 91269049e9456bed48ceef4a5d6e75df9b53f5f0
# branch.head master
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.branch.as_deref(), Some("master"));
        assert!(!p.has_upstream);
        assert_eq!(p.upstream, None);
        assert_eq!(p.ahead, None);   // ab 라인 없음 → None(미표시)
        assert_eq!(p.behind, None);
        assert!(p.is_clean);
    }

    #[test]
    fn detached_head() {
        let txt = "\
# branch.oid 8d42fe5d803a1ee92a51f252eea10c73629a85d3
# branch.head (detached)
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.branch, None);
        assert_eq!(p.detached_sha.as_deref(), Some("8d42fe5d803a1ee92a51f252eea10c73629a85d3"));
        assert!(p.is_clean);
    }

    #[test]
    fn empty_repo_initial_oid_is_not_sha() {
        let txt = "\
# branch.oid (initial)
# branch.head main
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.branch.as_deref(), Some("main"));
        assert_eq!(p.detached_sha, None);  // (initial)은 sha 아님
        assert!(p.is_clean);
    }

    #[test]
    fn conflict_u_line() {
        let txt = "\
# branch.oid 895821ddd416d350bc94fd0687f354581c6a50f8
# branch.head main
u UU N... 100644 100644 100644 100644 df967b c278a3 c77470 f
";
        let p = parse_porcelain_v2(txt);
        assert_eq!(p.conflicts, 1);
        assert_eq!(p.staged, 0);   // 충돌은 u 라인 전용, 1/2와 겹치지 않음
        assert_eq!(p.modified, 0);
        assert!(!p.is_clean);
    }
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cd src-tauri && cargo test --lib git_reader`
Expected: 컴파일 에러(`parse_porcelain_v2`/`ParsedStatus` 미정의).

- [ ] **Step 3: 구현 (git_reader.rs 상단)**

```rust
/// porcelain v2 --branch 파싱 결과(브랜치/원격/워킹트리 카운트). 시간/IO 없음.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ParsedStatus {
    pub branch: Option<String>,
    pub detached_sha: Option<String>,
    pub upstream: Option<String>,
    pub has_upstream: bool,
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
    pub conflicts: u32,
    pub is_clean: bool,
}

/// `git status --porcelain=v2 --branch` 출력을 파싱.
pub fn parse_porcelain_v2(text: &str) -> ParsedStatus {
    let mut s = ParsedStatus::default();
    let mut oid: Option<String> = None;
    let mut detached = false;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# branch.oid ") {
            oid = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("# branch.head ") {
            let v = rest.trim();
            if v == "(detached)" {
                detached = true;
                s.branch = None;
            } else {
                s.branch = Some(v.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("# branch.upstream ") {
            s.upstream = Some(rest.trim().to_string());
            s.has_upstream = true;
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            let mut ahead = 0u32;
            let mut behind = 0u32;
            for tok in rest.split_whitespace() {
                if let Some(n) = tok.strip_prefix('+') {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(m) = tok.strip_prefix('-') {
                    behind = m.parse().unwrap_or(0);
                }
            }
            s.ahead = Some(ahead);
            s.behind = Some(behind);
        } else if line.starts_with("1 ") || line.starts_with("2 ") {
            // XY는 항상 두 번째 공백구분 토큰
            if let Some(xy) = line.split_whitespace().nth(1) {
                let b = xy.as_bytes();
                if b.len() >= 2 {
                    if b[0] != b'.' {
                        s.staged += 1;
                    }
                    if b[1] != b'.' {
                        s.modified += 1;
                    }
                }
            }
        } else if line.starts_with("u ") {
            s.conflicts += 1;
        } else if line.starts_with("? ") {
            s.untracked += 1;
        }
    }

    s.is_clean = s.staged == 0 && s.modified == 0 && s.untracked == 0 && s.conflicts == 0;

    // detached일 때만 oid를 sha로 채움. '(initial)'/'(detached)' 같은 괄호 토큰은 sha 아님.
    if detached {
        if let Some(o) = oid {
            if !o.starts_with('(') {
                s.detached_sha = Some(o);
            }
        }
    }

    s
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test --lib git_reader`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/git_reader.rs
git commit -m "feat(m1): porcelain v2 순수 파서 + 실측 픽스처 6종 테스트"
```

---

## Task 8: git_reader.rs — git 헬퍼 + read_status (실제 repo 통합)

**Files:**
- Modify: `src-tauri/src/git_reader.rs`
- Create: `src-tauri/tests/git_reader_integration.rs`

- [ ] **Step 1: 통합 테스트 작성 (tests/git_reader_integration.rs)**

```rust
use gitmonitor::git_reader::read_status;
use gitmonitor::model::{RepoRef, RepoState};
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
    };
    let st = read_status(&r, 0);
    assert!(st.error.is_some());
}
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `cd src-tauri && cargo test --test git_reader_integration`
Expected: 컴파일 에러(`read_status` 미정의).

- [ ] **Step 3: 헬퍼 + read_status 구현 (git_reader.rs, 파서 아래·tests 위)**

```rust
use crate::model::{RepoRef, RepoState, RepoStatus};
use std::path::Path;
use std::process::Command;

/// repo에서 git 서브커맨드 실행(동기). 비-제로 종료/실패는 Err.
/// (타임아웃·동시성은 M2의 비동기 계층에서 추가)
fn run_git(repo: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("git 실행 실패: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {:?} 실패: {}",
            args,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// stash 개수. ⚠️ rev-list --count refs/stash 는 0개일 때 exit 128 fatal이므로
/// `git stash list` 줄 수로 센다(0개면 빈 출력).
pub fn stash_count(repo: &str) -> u32 {
    match run_git(repo, &["stash", "list"]) {
        Ok(out) => out.lines().filter(|l| !l.trim().is_empty()).count() as u32,
        Err(_) => 0,
    }
}

/// 연결 worktree 포함 총 worktree 수(메인 포함, >=1).
pub fn worktree_count(repo: &str) -> u32 {
    match run_git(repo, &["worktree", "list", "--porcelain"]) {
        Ok(out) => out.lines().filter(|l| l.starts_with("worktree ")).count() as u32,
        Err(_) => 1,
    }
}

/// `git rev-parse --git-path <marker>`로 경로 해석 후 존재 확인.
/// (worktree에서 .git이 파일이어도 올바른 경로를 돌려줌)
fn git_path_exists(repo: &str, marker: &str) -> bool {
    match run_git(repo, &["rev-parse", "--git-path", marker]) {
        Ok(p) => {
            let p = p.trim();
            let pb = Path::new(p);
            let full = if pb.is_absolute() {
                pb.to_path_buf()
            } else {
                Path::new(repo).join(pb)
            };
            full.exists()
        }
        Err(_) => false,
    }
}

/// 진행 중 상태 판정(우선순위: Merging > Rebasing > CherryPicking > Reverting > Bisecting).
pub fn detect_state(repo: &str) -> RepoState {
    if git_path_exists(repo, "MERGE_HEAD") {
        RepoState::Merging
    } else if git_path_exists(repo, "rebase-merge") || git_path_exists(repo, "rebase-apply") {
        RepoState::Rebasing
    } else if git_path_exists(repo, "CHERRY_PICK_HEAD") {
        RepoState::CherryPicking
    } else if git_path_exists(repo, "REVERT_HEAD") {
        RepoState::Reverting
    } else if git_path_exists(repo, "BISECT_LOG") {
        RepoState::Bisecting
    } else {
        RepoState::Clean
    }
}

/// FETCH_HEAD mtime(epoch). git-path 우선, 없으면 common-dir 폴백, 둘 다 없으면 None.
pub fn last_fetch(repo: &str) -> Option<i64> {
    fn mtime(p: &Path) -> Option<i64> {
        std::fs::metadata(p)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
    }
    fn resolve(repo: &str, rel_or_abs: &str) -> std::path::PathBuf {
        let pb = Path::new(rel_or_abs.trim());
        if pb.is_absolute() {
            pb.to_path_buf()
        } else {
            Path::new(repo).join(pb)
        }
    }

    if let Ok(p) = run_git(repo, &["rev-parse", "--git-path", "FETCH_HEAD"]) {
        if let Some(t) = mtime(&resolve(repo, &p)) {
            return Some(t);
        }
    }
    if let Ok(common) = run_git(repo, &["rev-parse", "--git-common-dir"]) {
        let base = resolve(repo, &common);
        if let Some(t) = mtime(&base.join("FETCH_HEAD")) {
            return Some(t);
        }
    }
    None
}

/// RepoRef에 휘발성 git 상태를 채워 RepoStatus 생성. now=배치 시각(epoch).
/// git status 실패 시 error만 세팅하고 나머지는 기본값.
pub fn read_status(repo: &RepoRef, now: i64) -> RepoStatus {
    let mut st = RepoStatus {
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
    };

    match run_git(&repo.path, &["status", "--porcelain=v2", "--branch"]) {
        Ok(text) => {
            let p = parse_porcelain_v2(&text);
            st.branch = p.branch;
            st.detached_sha = p.detached_sha;
            st.upstream = p.upstream;
            st.has_upstream = p.has_upstream;
            st.ahead = p.ahead;
            st.behind = p.behind;
            st.staged = p.staged;
            st.modified = p.modified;
            st.untracked = p.untracked;
            st.conflicts = p.conflicts;
            st.is_clean = p.is_clean;
            st.stash = stash_count(&repo.path);
            st.state = detect_state(&repo.path);
            st.worktrees = worktree_count(&repo.path);
            st.last_fetch = last_fetch(&repo.path);
        }
        Err(e) => {
            st.error = Some(e);
        }
    }

    st
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri && cargo test --test git_reader_integration`
Expected: 5 passed.

- [ ] **Step 5: 전체 테스트 + lint 확인**

Run: `cd src-tauri && cargo test && cargo clippy -- -D warnings`
Expected: 모든 테스트 통과(단위 + 통합). clippy 경고 0(있으면 수정).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/git_reader.rs src-tauri/tests/git_reader_integration.rs
git commit -m "feat(m1): git 헬퍼(stash/state/worktree/fetch) + read_status 통합테스트"
```

---

## M1 완료 기준 (Definition of Done)
- `cargo test`로 단위(model/config/discovery/git_reader 파서) + 통합(discovery/git_reader) **전부 통과**.
- `cargo clippy -- -D warnings` 통과.
- `config`/`discovery`/`git_reader`가 Tauri 없이 CLI/테스트로 `RepoStatus`를 산출 가능(spec §13 M1 수용 기준).
- M2(Tauri 통합: `.manage(AppState)`, 커맨드 5종, `repos_updated`+EmitGate, scheduler 포커스 게이팅+단일 in-flight, actions process::Command, 타임아웃 5s)는 별도 계획서로 진행.

---

## Self-Review (작성자 점검 결과)
- **Spec 커버리지(M1 범위)**: config 버저닝/백업(§3,§6 ✅ Task 2-3), discovery 글롭·카테고리·.git 디렉토리 판정(§5 ✅ Task 4-6), porcelain v2 파싱·no-upstream·detached·empty·conflict(§4 ✅ Task 7), stash list 카운트·rev-parse --git-path 마커·worktree·fetch 폴백(§4 ✅ Task 8). 타임아웃/동시성/IPC/UI는 M1 범위 밖으로 명시 이월.
- **Placeholder 스캔**: 모든 코드 스텝에 실제 코드·실제 명령·기대 출력 포함. TODO/TBD 없음.
- **타입 일관성**: `RepoRef`/`RepoStatus`/`RepoState`(model.rs)와 `Config`/`TerminalApp`(config.rs), `ParsedStatus`(git_reader.rs), `DiscoveryConfig`(discovery.rs) 시그니처가 태스크 전반에서 일치. `read_status(&RepoRef, i64)`·`category_for(&Path,&Path)`·`build_exclude_set(&[String])`·`discover(&DiscoveryConfig)` 호출부/정의부 동일.
