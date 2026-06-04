# 아키텍처

RepositoryMonitor는 단일 Tauri 2 프로세스 안에서 **Rust 백엔드**(저장소 발견·상태 읽기·집계)와 **Svelte 5 프론트엔드**(렌더)가 IPC로 연결된 구조다. 데이터는 백엔드 → 프론트 **단방향 push**(`repos_updated` 이벤트), 제어는 프론트 → 백엔드 **command invoke**로만 흐른다.

## 설계 원칙

- **로컬 전용 / 네트워크 0** — `git status` · `svn status` 등 read-only 로컬 명령만. fetch/pull/push 없음.
- **VCS 추상화** — `VcsKind { Git, Svn }`로 git/svn을 통일된 `RepoStatus`로 표현. reader만 분기.
- **단방향 데이터 흐름** — 프론트엔 git 로직이 없다. 백엔드가 진실의 원천(스냅샷)을 push.
- **순수 로직 분리** — 표시 규칙(clean 술어·정렬·필터·그룹)과 파싱(porcelain v2·svn status)을 순수 함수로 분리해 단위 테스트.

## 디렉토리

```
RepositoryMonitor/
├── src/                       # Svelte 5 프론트
│   ├── App.svelte             # 진입점: store 초기화, EmptyState/Grid 분기, 컨텍스트 메뉴
│   ├── lib/
│   │   ├── types.ts           # Rust 타입 미러(snake_case)
│   │   ├── tauri.ts           # invoke/listen 래퍼 (백엔드 의존 단일 지점)
│   │   ├── logic.ts           # 순수 표시 로직 (clean/rank/filter/format/group)
│   │   ├── store.svelte.ts    # reactive store (repos_updated 구독, seq 폐기)
│   │   └── theme.svelte.ts    # 다크/라이트 (system/light/dark, localStorage)
│   ├── components/            # RepoCard · Grid · Header · Settings · EmptyState
│   └── app.css                # CSS 변수 팔레트 (라이트/다크)
├── src-tauri/                 # Rust 백엔드 + Tauri 셸
│   ├── src/
│   │   ├── main.rs            # repositorymonitor::run() 호출
│   │   ├── lib.rs             # 모듈 선언 + run() (Builder 배선)
│   │   ├── model.rs           # VcsKind · RepoRef · RepoStatus · RepoState · ActionKind · RepoSnapshot
│   │   ├── config.rs          # Config · TerminalApp · 로드/저장(버저닝·백업)
│   │   ├── discovery.rs       # 루트 스캔 · 제외 글롭 · 카테고리 · .git/.svn 판정
│   │   ├── git_reader.rs      # porcelain v2 파서 + stash/state/worktree/fetch
│   │   ├── svn_reader.rs      # svn status/info 파서 (로컬 전용)
│   │   ├── batch.rs           # 비동기 상태 배치 (semaphore + timeout, vcs 디스패치)
│   │   ├── scheduler.rs       # 폴링 판단 (should_run_poll)
│   │   ├── emit_gate.rs       # 스냅샷 변경 감지 (should_emit)
│   │   ├── snapshot.rs        # 실패 repo 직전값 머지
│   │   ├── app_state.rs       # AppState (.manage)
│   │   └── commands.rs        # IPC 커맨드 5종 + do_scan/do_refresh
│   ├── tests/                 # 통합 테스트 (discovery/git_reader/batch/svn)
│   ├── capabilities/          # Tauri 권한 (window/event/dialog)
│   └── tauri.conf.json        # 단일 main 윈도우, identifier com.dgitx.repositorymonitor
└── docs/                      # 본 문서들
```

## 백엔드 모듈 책임 (Rust)

| 모듈 | 책임 |
|---|---|
| `config` | `Config`(roots·manual_paths·exclude_globs·poll_interval_secs·scan_depth·stale_fetch_days·terminal_app) 로드/저장. 경로는 `dirs_next::config_dir()/RepositoryMonitor/config.json`. forward-compat 역직렬화(`#[serde(default)]`), 손상 시 `.bak` 백업 후 기본값 재생성 |
| `discovery` | 루트를 walk(깊이 제한·`node_modules`/`target`/`Pods`/`.build`/`.git`/`.svn` prune·심링크 미추적)하며 `.git`(디렉토리)=git, `.svn`=svn 으로 `RepoRef{path,name,category,vcs}` 산출. 제외 글롭은 repo 절대경로에 globset(globstar·대소문자무시) 매칭. 카테고리=루트 기준 상대경로 첫 세그먼트 |
| `git_reader` | `git status --porcelain=v2 --branch` 단일 파싱(브랜치·upstream·ahead/behind·XY 코드). 보조: stash(`stash list` 줄 수), state 마커(`rev-parse --git-path`), worktree(`worktree list`), last_fetch(`FETCH_HEAD` mtime) |
| `svn_reader` | `svn status`(로컬) 파싱(M/A/D/R/!/~→modified, ?→untracked, C→conflict) + `svn info --show-item relative-url`로 브랜치 도출. ahead/behind/staged/stash 없음 |
| `batch` | `RepoRef[]`를 tokio 블로킹 태스크로 병렬 실행(`Semaphore` 동시 상한 8, repo당 5초 `timeout`). `repo.vcs`로 git/svn reader 디스패치. 타임아웃/실패는 error `RepoStatus` |
| `scheduler` | `should_run_poll(polling_active, in_flight)` — 창 포커스 중이고 진행 배치가 없을 때만 폴링. 실제 루프는 `lib.rs` setup()이 `WindowEvent::Focused`로 `polling_active`를 토글하며 구동 |
| `emit_gate` | `should_emit(prev, next)` — `last_checked`를 제외한 의미 비교로 무의미 emit 차단 |
| `snapshot` | `merge_failed_with_previous` — 일시 실패 repo는 직전 스냅샷 수치를 유지하고 error/last_checked만 갱신 |
| `app_state` | `AppState{ config, repos, last_snapshot, polling_active, in_flight, seq }` — `.manage(Arc<AppState>)`로 공유 |
| `commands` | IPC 5종 + `do_scan`/`do_refresh`(테스트 가능하도록 emit을 콜백으로 분리한 `do_refresh_inner`) |

### 동시성·갱신 규약

- **단일 in-flight** — `do_refresh`는 `in_flight`를 `swap(true)`로 획득(이미 진행 중이면 coalesce). RAII 가드(`Drop`)로 어떤 경로로 빠져도 `false` 복원 → "in_flight 영구 true" 데드락 방지.
- **포커스 게이팅** — `WindowEvent::Focused(true)` → 폴링 ON + 즉시 1회, `Focused(false)` → OFF. 주기는 `poll_interval_secs`(clamp 10–300).
- **seq** — emit마다 `seq` 증가. 프론트는 `seq < lastSeq`인 오래된 스냅샷을 폐기.

## 프론트 모듈 (Svelte 5)

| 파일 | 책임 |
|---|---|
| `lib/types.ts` | Rust serde 직렬화와 1:1 미러(snake_case 필드, enum snake_case). 이벤트 payload는 camelCase 자동변환이 없으므로 snake_case 유지 |
| `lib/tauri.ts` | `invoke`/`listen` 래퍼 — 백엔드 의존 단일 지점. 커맨드 인자는 Tauri가 camelCase 변환(`repo_path`→`repoPath`) |
| `lib/logic.ts` | `isClean` 단일 술어, `rank`+`compareRepos`(정렬), `filterProblems`, `formatFetched`/`isStale`, `groupByCategory`, `summarize` — 전부 순수, vitest 커버 |
| `lib/store.svelte.ts` | `$state` repos/config/lastSeq, `init`(구독·config 로드)/`dispose`, `refresh`/`rescan`/`saveConfig`/`excludeRepo` |
| `lib/theme.svelte.ts` | `system/light/dark` 선택을 localStorage 영속 + `<html data-theme>` 적용 |
| `components/RepoCard` | §7 신호 렌더 + 호버 액션 + 우클릭(onContext) + clean/dirty/conflict 강조 |
| `components/Grid` | `filterProblems` → 검색 → `groupByCategory`(내부 `compareRepos` 정렬) |
| `components/Header` | 검색·문제만·테마토글·새로고침·설정·요약 |
| `components/Settings` | 루트(dialog 폴더선택)·제외글롭·주기·깊이·stale·터미널 앱 |
| `components/EmptyState` | 루트 0개 first-run CTA |

## 마일스톤 이력

- **M1** 백엔드 코어(config/discovery/git_reader)
- **M2** Tauri 통합(AppState·IPC·이벤트·스케줄러·배치·액션)
- **M3** Svelte 5 프론트(그리드 UI)
- **M4** SVN 추적(VcsKind 추상화·svn_reader)
- 이후 UI 반복: 다크/라이트, 비클린 강조, 우클릭 컨텍스트 메뉴, 리스트 거터

> 본 프로젝트는 M1~M4 기간 동안 `GitMonitor`라는 이름이었고, SVN 추가 이후 `RepositoryMonitor`로 개명되었다. `docs/superpowers/`의 설계서·계획서는 그 시점의 기록이라 옛 이름을 보존한다.
