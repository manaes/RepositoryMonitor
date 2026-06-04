# GitMonitor — 멀티 레포 Git 상태 대시보드 설계서

- **날짜**: 2026-06-04
- **상태**: 승인됨 (브레인스토밍 + 어드버서리얼 리뷰 완료, 구현 계획 대기)
- **작업 디렉토리**: `2_App/GitMonitor` (AIAgentMonitor와 형제 프로젝트)
- **제품명**: **GitMonitor** (확정)
- **번들 식별자**: `com.dgitx.gitmonitor` (tauri.conf.json `identifier`)

> 본 문서는 1차 설계 후 6-critic 어드버서리얼 리뷰(누락/일관성/스코프/모호성/git 기술 정확성/Tauri 실현가능성)를 거쳐 개정한 v2다. git 관련 주장은 실제 git 명령 실측으로, Tauri 패턴은 형제 프로젝트 AIAgentMonitor 소스 대조로 검증했다.

---

## 1. 개요

### 문제
`~/Desktop/@Projects` 아래 ~22개의 git 저장소가 카테고리별(`@ITXRtsp`, `0_Inbox`, `2_App`, `3_Library`, `4_Server` 등)로 흩어져 있어, 어떤 repo에 미커밋 변경이 있는지·푸시 안 한 커밋이 있는지·현재 어느 브랜치인지를 한눈에 파악하기 어렵다.

### 목표
여러 git 저장소의 **상태 요약**을 한 창의 카드 그리드로 보여주는 macOS 전용 데스크톱 앱. repo별로: 브랜치(또는 detached), 로컬 기준 ahead/behind, 미커밋 변경(staged/modified/untracked/conflict), stash, 진행 상태(merge/rebase/…), 마지막 fetch 시각, clean 여부.

### 비목표 (YAGNI — v1에서 하지 않는 것)
- 파일별 diff / 변경 내역 표시
- `fetch`/`pull`/`push`/`commit` 등 **쓰기성·네트워크 git 작업** (로컬 전용, 네트워크 0)
- 커밋 히스토리 / 그래프 시각화
- 멀티 계정 / 원격 인증 관리
- 메뉴바 트레이 (윈도우 그리드 전용)
- submodule 내부 상태 추적 (제외 글롭 권장 대상)

→ 핵심 가치: **"상태 관찰 + 외부 도구로 점프"**.

---

## 2. 아키텍처

**스택**: Tauri 2 (Rust 백엔드 + Svelte 5 + Vite + TypeScript + pnpm). AIAgentMonitor와 동일.

**데이터 흐름(단방향)**: 백엔드가 discovery로 repo 목록(`RepoRef`)을 만들고 → git_reader가 각 `RepoRef`에 휘발성 git 상태를 채워 `RepoStatus` 스냅샷 생성 → 백엔드가 직전 스냅샷과 비교(EmitGate)해 변경 시에만 `repos_updated` 이벤트 emit → Svelte 스토어 → 그리드 렌더. 프론트엔드엔 git 로직 0.

```
[FS: .git dirs] ──(git CLI)──┐
                              ▼
┌──────────────────────── Rust (src-tauri) ────────────────────────────┐
│ config ─ discovery ─ git_reader ─ scheduler ─ actions                  │
│   │         │            │           │           │                     │
│   │     RepoRef[]   RepoStatus[]  poll gate   open/spawn               │
│   │     (path/name/  (RepoRef +   (focus      (process::Command)       │
│   │      category)   git fields)   AtomicBool)                         │
│   └─ AppState(.manage): repo목록 + 직전 스냅샷 캐시 + in-flight 플래그   │
│        + 폴링 활성 AtomicBool + 세마포어 + emit seq                      │
│                         │ EmitGate(해시 비교)                           │
│   WindowEvent::Focused ─┤ WebviewWindow::emit("repos_updated", snap)    │
└─────────────────────────┼─────────────────────────────────────────────┘
        invoke ▲           ▼ listen
┌──────────────────────── Svelte (src) ────────────────────────────────┐
│ store ─ Grid(카테고리 그룹/카드) ─ Header(검색/필터/정렬/갱신) ─ Settings │
│ (RepoCard: 호버 액션, navigator.clipboard로 경로 복사)                  │
└───────────────────────────────────────────────────────────────────────┘
```

### 백엔드 모듈 책임 (Rust)

| 모듈 | 책임 |
|---|---|
| `config` | `Config` 로드·저장. 경로는 **`dirs_next::config_dir().join("GitMonitor/config.json")`** (bundle-id 하드코딩 금지, AIAgentMonitor와 동일 방식). forward-compatible 역직렬화(§6 버저닝) |
| `discovery` | 루트 아래 `.git` **디렉토리** 재귀 탐색(파일 형태 `.git`/gitlink은 별도 repo로 보지 않음) + 수동 경로 합치고 제외 글롭 적용 → `RepoRef`(path·name·category) 목록 산출. `name`·`category`는 **여기서** 결정 |
| `git_reader` | 입력으로 받은 `RepoRef` **하나**에 git 휘발성 필드를 채워 `RepoStatus` 생성. category/name은 git_reader가 계산하지 않고 RepoRef에서 가져옴. per-call 타임아웃 5s |
| `scheduler` | 폴링 활성 `AtomicBool`(백엔드 `WindowEvent::Focused`가 토글)을 매 tick 확인 → 활성 시에만 상태 배치 실행. **단일 in-flight**만 허용(진행 중 트리거는 coalesce). 직전 성공 스냅샷을 AppState에 캐시하고 실패 repo는 이전 값+error로 머지 |
| `actions` | **`std::process::Command`(또는 `tokio::process::Command`)로 `open` 직접 spawn** — shell plugin/capability scope 불필요. Finder=`open <path>`, 터미널=`open -a <terminal_app> <path>`, SourceTree=`open -a SourceTree <path>`. 경로 복사는 백엔드가 아니라 **프론트 `navigator.clipboard`** 담당 |

### IPC 계약 (Tauri 2)

모든 커맨드는 `Result<T, String>` 반환(에러 직렬화 통일). 동시성 상태는 `.manage(AppState)`로 공유하며 커맨드는 `AppHandle`/`State<AppState>`를 주입받는다.

**Commands** (프론트 → 백엔드):
- `get_config(state) -> Result<Config, String>`
- `set_config(state, config) -> Result<(), String>` — 변경 항목에 따른 재적용은 §6 표 참조
- `scan_repos(app, state) -> Result<Vec<RepoRef>, String>` — discovery 재실행(무거움). discovery 레벨 실패는 여기 `Err`로
- `refresh_status(app, state) -> Result<(), String>` — 현재 repo 상태 재읽기(가벼움). 결과는 이벤트로 emit, repo 개별 실패는 `RepoStatus.error`로
- `open_action(state, repo_path, kind: ActionKind) -> Result<(), String>` — `CopyPath`는 프론트 처리이므로 백엔드 variant에서 제외(아래 §3)

**Events** (백엔드 → 프론트):
- `repos_updated(snapshot: Vec<RepoStatus>)` — 상태 배치 완료 시 전체 스냅샷. **단, EmitGate로 직전 스냅샷과 해시가 같으면 emit 생략.** 단일 메인 창이므로 `AppHandle::emit`(전역) 대신 `WebviewWindow::emit`로 좁힘. payload는 단조 증가 `seq`를 포함해 프론트가 오래된 스냅샷을 폐기.
- **직렬화 주의**: 이벤트 payload 구조체는 invoke 인자와 달리 **자동 camelCase 변환이 없다**(serde 기본 snake_case). 프론트 TS 타입을 Rust 필드명 그대로 **snake_case**로 정의(AIAgentMonitor와 동일 방침).

---

## 3. 데이터 타입

```rust
// 영속 설정 — config.json
#[derive(Serialize, Deserialize)]
#[serde(default)]               // 누락 필드는 기본값(forward-compat)
struct Config {
    version: u32,               // 스키마 버전 (현재 1)
    roots: Vec<String>,         // 스캔 루트 절대경로
    manual_paths: Vec<String>,  // 개별 등록 repo 절대경로
    exclude_globs: Vec<String>, // 제외 글롭 (§5 규칙)
    poll_interval_secs: u32,    // 기본 30, 범위 10–300
    scan_depth: u32,            // 기본 4
    stale_fetch_days: u32,      // 기본 7 (이 이상이면 behind 흐림)
    terminal_app: TerminalApp,
}
// deny_unknown_fields 미사용(알 수 없는 필드 무시). 파싱 완전 실패 시
// config.json.bak로 백업 후 기본 Config 재생성.

enum TerminalApp { Terminal, ITerm, Custom(String) } // Custom = .app 경로

// discovery 산출물 — git_reader 입력
struct RepoRef {
    path: String,      // repo 루트 절대경로
    name: String,      // repo 디렉토리명
    category: String,  // §5 규칙으로 산출 ("2_App", "@ITXRtsp", "(manual)" 등)
}

// 백엔드 액션 종류 (CopyPath는 프론트 처리이므로 백엔드 IPC에 없음)
enum ActionKind { OpenFinder, OpenTerminal, OpenSourceTree }

enum RepoState { Clean, Merging, Rebasing, CherryPicking, Reverting, Bisecting }

// 스냅샷 단위 — RepoRef + 휘발성 git 상태
struct RepoStatus {
    // RepoRef에서 그대로
    path: String,
    name: String,
    category: String,
    // 브랜치/원격
    branch: Option<String>,        // None = detached
    detached_sha: Option<String>,  // branch.head == "(detached)"일 때만
    upstream: Option<String>,
    has_upstream: bool,            // branch.upstream 라인 존재 여부
    ahead: Option<u32>,            // branch.ab 존재할 때만 Some
    behind: Option<u32>,           // 〃
    // 워킹트리 (파일 수 기준, 한 파일이 staged·modified 동시 가능)
    staged: u32,                   // 1/2 라인에서 X != '.' 인 파일 수
    modified: u32,                 // 1/2 라인에서 Y != '.' 인 파일 수
    untracked: u32,                // '?' 라인 수
    conflicts: u32,                // 'u' 라인 수 (1/2와 겹치지 않음)
    stash: u32,
    is_clean: bool,                // 1/2/u/? 라인이 하나도 없음
    state: RepoState,
    worktrees: u32,                // git worktree list 총 개수(메인 포함, >=1)
    last_fetch: Option<i64>,       // epoch, None = 한 번도 fetch 안 함/미해석
    last_checked: i64,             // 이번 배치 시각 epoch
    error: Option<String>,         // 경로 소실/git 실패 시 사유(이때 수치 필드는 직전 값 유지)
}
```

`i64` epoch 필드는 JS `number`로 안전 직렬화된다.

---

## 4. git 읽기 상세 (porcelain v2) — 실측 검증 완료

핵심 명령: `git -C <repo> status --porcelain=v2 --branch`

### 파싱 규칙
| 라인 | 추출 | 주의(실측) |
|---|---|---|
| `# branch.oid <v>` | detached일 때 detached_sha | 빈 repo(커밋 0)는 값이 `(initial)` 리터럴 → sha로 저장 금지. `(detached)`/`(initial)` 같은 괄호 토큰은 sha 아님 |
| `# branch.head <v>` | `(detached)`면 branch=None, 아니면 branch명 | rebase/bisect 중에도 `(detached)`로 나옴 → state 배지가 detached 표시보다 **우선** |
| `# branch.upstream <v>` | upstream, `has_upstream=true` | **upstream 없으면 이 라인 자체가 출력 안 됨** |
| `# branch.ab +<n> -<m>` | `ahead=Some(n)`, `behind=Some(m)` | **upstream 없으면 이 라인도 없음**. 없으면 ahead/behind=None(↑↓ 숨김, `⊘no upstream` 배지). in-sync는 `+0 -0`로 나옴(upstream 있을 때만) |
| `1 <XY> …` / `2 <XY> …` | X≠`.`→staged++, Y≠`.`→modified++ | XY는 항상 2번째 공백 토큰. `2`(rename)는 score(`R100`) 추가 + 경로가 `<new>\t<orig>` **TAB 분리** — v1은 카운트만 하므로 XY 기반은 안전. 경로 사용 시 `-z`/TAB 인지 필요 |
| `u <xy> …` | conflicts++ | 충돌은 **오직 u 라인**으로만 나옴(1/2와 중복 없음) |
| `? …` | untracked++ | untracked는 디렉토리 단위로 접힘 |

- **staged/modified 의미**: "파일 수"가 아니라 *인덱스에 staged된 파일 수* / *워킹트리에 미staged 변경이 있는 파일 수*. 한 파일이 `MM`이면 staged·modified 양쪽에 각 +1(축별 카운트).
- **is_clean**: 1/2/u/? 라인이 하나도 없으면 true (실측 일치).

### 보조 수집 (모두 읽기 전용, .git 미수정)
- **stash**: ⚠️ `rev-list --walk-reflogs --count refs/stash`는 **stash 0개일 때 exit 128 fatal**(`ambiguous argument 'refs/stash'`). → **`git -C <repo> stash list`의 출력 줄 수**로 카운트(0개면 빈 출력 exit 0). 또는 `show-ref --verify --quiet refs/stash` 가드 후 rev-list.
- **state 마커**: `.git/<file>` 리터럴 경로 금지(worktree에서 .git이 파일). 각 마커를 **`git -C <repo> rev-parse --git-path <FILE>`**로 절대경로 해석 후 존재 확인:
  - `MERGE_HEAD`(merging), `rebase-merge`/`rebase-apply`(rebasing, **디렉토리**), `CHERRY_PICK_HEAD`(cherry-pick), `REVERT_HEAD`(revert), `BISECT_LOG`(bisect).
  - **판정 우선순위**(여러 마커 동시 가능): `Merging > Rebasing > CherryPicking > Reverting > Bisecting`, 아무 마커도 없으면 `Clean`. 첫 매칭 채택.
- **last_fetch**: `git rev-parse --git-path FETCH_HEAD` 경로의 mtime. 없으면 `git rev-parse --git-common-dir`의 `FETCH_HEAD` mtime으로 폴백(연결 worktree는 per-wt FETCH_HEAD가 없을 수 있음). 둘 다 없으면 `None`("never fetched"). worktree 카드에서 last_fetch 신뢰도는 낮을 수 있음(엣지케이스 §10).
- **worktrees**: `git -C <repo> worktree list --porcelain`의 `worktree ` 라인 수(메인 포함, >=1).

---

## 5. 프로젝트 발견 & 등록

- **루트 스캔**: 등록 루트 아래 깊이 제한(기본 4) 내 `.git` **디렉토리**를 재귀 탐색하여 자동 추가. discovery는 디렉토리 prune(`node_modules`/`target`/`Pods`/`.build`/`.git` 내부) + 심링크 미추적 + 권한 오류 스킵.
- **`.git`이 파일인 경우(연결 worktree/gitlink)는 별도 repo로 등록하지 않음** → worktree 사용자의 중복 카드 방지.
- **수동 추가**: 폴더 선택(dialog plugin) 또는 드래그&드롭(백엔드 `WindowEvent::DragDrop`로 경로 취득). 루트 밖 경로도 가능.
- **제외 글롭** (단일 규칙으로 못박음):
  - 매칭 대상 = **repo 루트의 절대경로 전체 문자열**.
  - `globset`(globstar 활성): `*`=단일 세그먼트(`/` 미포함), `**`=다중 세그먼트.
  - 패턴이 `/`로 시작 → 절대경로 매칭. 아니면 자동으로 `**/` prefix(어느 위치든 매칭). 예: `node_modules` → `**/node_modules/**`.
  - macOS 기본 FS에 맞춰 **대소문자 무시**.
  - discovery의 디렉토리 prune과는 **별개 단계**(prune은 성능, 글롭은 사용자 제외).
- **category 산출 규칙** (단일 정의):
  - `category` = repo 경로의 **소속 루트 기준 상대경로의 첫 세그먼트**.
    - 루트=`@Projects`, repo=`@Projects/@ITXRtsp/edge-client-swift` → `@ITXRtsp`.
    - repo=`@Projects/2_App/GitMonitor` → `2_App`.
    - 상대경로 세그먼트가 1개(루트 직속)면 category=루트 폴더명.
  - 중첩 git 여부는 무관(순수 FS 경로 기반).
  - 수동 추가 repo가 어느 루트 하위면 그 루트 기준, 아니면 `category="(manual)"`.
- **첫 실행(first-run)**: `config.json` 부재 시 루트 0개로 정상 기동. 그리드 자리에 **empty-state 카드**("스캔할 폴더를 추가하세요" CTA)를 렌더. 사용자 환경의 `~/Desktop/@Projects`를 **"추가하시겠습니까?" 1-click 제안**으로만 노출(자동 등록 금지). 루트 0개면 discovery/폴링은 no-op.

---

## 6. 갱신 전략 & 설정 재적용

- **앱 시작**: discovery 스캔 → 초기 상태 1회.
- **폴링 게이팅**: 프론트 `set_focus` 커맨드 **사용 안 함**(표준 `WebviewWindow::set_focus()`와 충돌). 백엔드 `setup()`에서 메인 창의 `on_window_event`로 `WindowEvent::Focused(bool)`를 직접 듣고 `AtomicBool` 토글. scheduler 루프는 매 tick 그 값 확인 → `false`면 skip, `false→true` 전환 순간 즉시 1회 폴링. (최소화/오클루전은 OS가 blur를 발생시키므로 focus 이벤트에 위임.)
- **주기**: `poll_interval_secs`(기본 30, 10–300). 상태만 재읽기.
- **수동 갱신**: 버튼 + `⌘R`.
- **동시성/재진입**: 상태 배치는 **단일 in-flight**만 허용(진행 중이면 신규 트리거 coalesce/무시). 이벤트에 단조 증가 `seq`를 실어 프론트가 오래된 스냅샷 폐기. Rescan(discovery)은 폴링 일시중지 후 실행, 완료 후 재개.
- **실패 repo 머지**: 백엔드가 직전 성공 스냅샷을 AppState에 캐시. 특정 repo의 git 읽기 실패 시 그 repo는 **이전 수치 필드 유지 + `error` 세팅**해서 머지(전체 스냅샷 교체 모델이지만 값은 보존). → §2 EmitGate/캐시가 담당(상태는 git이 진실의 원천이되, 일시 실패 시 직전 값 표시용 캐시는 메모리에 보관).
- **config 버저닝**: `Config.version`(현재 1). serde `#[serde(default)]`로 누락 필드 기본값, 알 수 없는 필드 무시. 파싱 실패 시 `.bak` 백업 후 기본 재생성. v1에 마이그레이션 코드는 불요(YAGNI)지만 version 필드 + forward-compat 역직렬화는 포함.

### 설정 변경 → 재적용 트리거
| 변경 항목 | 효과 |
|---|---|
| `poll_interval_secs` | scheduler 인터벌 즉시 재시작 |
| `roots` / `exclude_globs` / `scan_depth` | 저장 시 discovery 자동 재실행(=암묵 Rescan) |
| `terminal_app` | 다음 액션부터 적용 |
| `stale_fetch_days` | 다음 렌더부터 적용 |

---

## 7. UI / UX

```
┌──────────────────────────────────────────────────────────┐
│ GitMonitor   ⟳ 갱신:방금   ⚙   🔍search   [□문제만] [정렬▾] │
│ 22 repos · 5 dirty · 2 behind · 1 ahead                    │
├─ 2_App ──────────────────────────────────────────────────┤
│ ┌Seqnex──────┐ ┌KPS─────────┐ ┌Webrtc──────┐ ┌nViewer───┐│
│ │main ↑2     │ │feat/x      │ │main ✓      │ │main ✓    ││
│ │●3 +1 ?2    │ │+1 ⚑1       │ │            │ │          ││
│ │fetched 1d  │ │fetched 4h  │ │            │ │          ││
│ └[F][T][S][⧉]┘ └ …         ┘                              │
├─ 4_Server ───────────────────────────────────────────────┤
│ ┌AdminServer─┐ ┌Sequrinet───┐ ...                          │
│ │dev ↓5 ⚠2   │ │⊘no upstream│                              │
└──────────────────────────────────────────────────────────┘
```

- **그리드**: 카테고리별 접이식 섹션, 카드 1개 = repo 1개. 반응형 열 수.
- **헤더 "갱신:방금"**: `max(last_checked)` 기반.

### `clean` 단일 술어 (정렬·필터·렌더 공통)
```
clean := is_clean && state==Clean && conflicts==0 && stash==0
         && (ahead.unwrap_or(0)==0) && (behind.unwrap_or(0)==0)
         && worktrees<=1 && error==None
```

### 정렬 ("문제 우선") — rank 함수 + tie-break
각 repo에 단일 rank(최고 심각도) 부여:
```
rank = conflicts>0 ? 0
     : state!=Clean ? 1
     : behind.unwrap_or(0)>0 ? 2
     : !is_clean ? 3            // 워킹트리 dirty
     : ahead.unwrap_or(0)>0 ? 4
     : 5                        // clean
```
동일 rank tie-break(순서대로): (a) 변경 파일 총합(staged+modified+untracked+conflicts) 내림차순 → (b) behind 내림차순 → (c) ahead 내림차순 → (d) category 사전순 → (e) name 사전순. 정렬은 **카테고리 그룹 내부**에 적용. (대안 정렬: 이름순/카테고리순)

### 필터
`문제만 보기` 토글 = `!clean`인 repo만 표시(위 술어 재사용). 이름 검색(debounce).

### 카드 신호 ↔ 데이터 필드 (1:1)
| 신호 | 조건 |
|---|---|
| `main` / `feat/x` | `branch` |
| `detached @a1b2c3` | `branch==None` → `detached_sha` 단축 표시 (state 배지가 있으면 그 배지 우선) |
| `⊘no upstream` | `has_upstream==false` (정상 브랜치이나 upstream 없음). detached와 **별개 배지** |
| `↑n` `↓m` | `ahead`/`behind`가 `Some`일 때만. `None`이면 숨김 |
| `+n`(staged) `●n`(modified) `?n`(untracked) `⚠n`(conflict) `⚑n`(stash) | 각 카운트>0 |
| `✓` clean | `clean` 술어 true |
| merging/rebasing/… 배지 | `state` |
| `+N worktree` | `worktrees>=2`일 때 **N = worktrees-1**(메인 제외 추가 worktree 수) |
| `fetched …` | None→`never fetched`(+behind 흐림), <1h→`just now`, <24h→`Nh ago`, >=24h→`Nd ago`. `last_fetch`가 `stale_fetch_days` 이상 오래되면 behind 수치 흐림 |
| 에러 배지/카드 | `error.is_some()` |

### 카드 액션 (호버 버튼 / 우클릭)
`Finder에서 열기`(F) · `터미널에서 열기`(T) · `SourceTree에서 열기`(S) · `경로 복사`(⧉, **프론트 navigator.clipboard**). 에디터 연결 제외.

### 설정 패널
루트 추가/삭제(폴더 선택), 수동 경로 관리, 제외 글롭, 폴링 주기, 스캔 깊이, stale 기준 일수, 터미널 앱 선택(Terminal/iTerm/Custom .app).

### 상태별 표시
- **로딩**: 스켈레톤 카드.
- **empty-state**(루트 0개): 추가 CTA + `@Projects` 1-click 제안.
- **clean 카드**: 1줄(브랜치명 + ✓) 고정 높이 축소 렌더. ahead/behind/dirty 행 생략. 카드 합치기(collapse)는 하지 않음.
- **에러 카드**: 경로 소실 → "제거" 버튼.

---

## 8. 핵심 기술 결정 & 대안

| 결정 | 선택(추천) | 대안 | 근거 |
|---|---|---|---|
| git 읽기 | `git` CLI `--porcelain=v2 --branch` + 보조 명령 | libgit2(`git2`)/`gix` | config·ignore·엣지케이스 100% 일치(실측 검증) |
| 스캐폴딩 | AIAgentMonitor 스켈레톤 복제 후 AI 로직 제거 | `create-tauri-app` | CI·아이콘·single-instance·IPC 패턴 재사용 |
| 외부 앱 열기 | 백엔드 `std/tokio::process::Command("open")` | shell plugin + capability scope | 임의 repo 경로 인자 전달이 안전·간단, 권한 불요(형제 프로젝트 동일) |
| 경로 복사 | 프론트 `navigator.clipboard` | clipboard-manager plugin | 단순 텍스트엔 plugin 과함, 권한 0 |
| config 경로 | `dirs_next::config_dir()` | Tauri `app_config_dir()` | 형제 프로젝트 일관성, 단위테스트 용이, bundle-id 결합 가정 회피 |
| 폴더 선택 | `tauri-plugin-dialog` 3종 세트 | HTML input | Tauri 표준, 형제 프로젝트 사용 패턴 |
| 폴링 게이팅 | 백엔드 `WindowEvent::Focused` + AtomicBool | 프론트 set_focus 커맨드 | 표준 API명 충돌 회피, JS focus 신뢰도 문제 회피 |
| 동시성 | tokio + 세마포어(상한 8), 단일 in-flight | 순차/무제한 | 응답성 + 경합 방지 |
| 상태 저장 | 메모리(직전 스냅샷 캐시) + config.json(설정만) | 자체 DB | git이 진실의 원천, 일시 실패 표시용 캐시만 |

---

## 9. 에러 처리 & 엣지케이스
- 경로 소실/비-git → 에러 카드 + "제거".
- git 타임아웃(5s)/`index.lock` 충돌 → 직전 수치 유지 + `error` 배지, 다음 폴링 재시도.
- discovery 권한 오류/심링크 루프 → 스킵, 깊이 상한.
- **빈 repo(커밋 0)**: `branch.oid (initial)` + `branch.head main`만, upstream/ab/파일 라인 없음 → is_clean=true 정상 처리. `(initial)`을 sha로 저장 금지.
- **detached / no-upstream / conflict / merge·rebase·bisect 진행 중** → 모두 명시 표시(state 배지 우선).
- **worktree의 `.git`은 파일** → 마커/FETCH_HEAD는 `rev-parse --git-path`로 해석. per-wt FETCH_HEAD 부재 시 common-dir 폴백 또는 None.
- **외부 앱 미설치**(`open -a` non-zero exit) → 토스트로 "앱 미설치" 안내 후 액션 무시. SourceTree 미설치 시 버튼은 노출하되 실패 토스트.

---

## 10. 테스트 전략
- **Rust 단위**: porcelain v2 파서(정상/no-upstream/detached/빈 repo/`MM`/rename(2)/conflict 샘플 출력 → `RepoStatus` 단언), discovery 워커(임시 트리 + `.git` 파일 vs 디렉토리 구분), 제외 글롭 매칭, category 산출, config 직렬화/역직렬화(**누락 필드/구버전/손상 파일** 케이스), state 마커 우선순위.
- **Rust 통합**: 임시 git repo 생성(init/commit/stage/modify/untracked/stash/detached/no-upstream/worktree 추가) → `git_reader` 결과 단언. `stash list` 0개 케이스 회귀 포함. `src-tauri/tests` 활용.
- **프론트**: 카드 상태 렌더(clean/dirty/detached/no-upstream/error/conflict/worktree), 정렬 rank+tie-break, `clean` 술어 기반 필터, fetched 시간 표기 규칙.

---

## 11. 성능
22개는 사소, 설계는 ~100+ repo 고려: git 호출 병렬(세마포어 상한)로 폴링 1회 수백 ms, discovery는 폴링과 분리, **EmitGate로 변경 없으면 emit 생략**(짧은 주기에서 webview 리렌더 절약), 프론트 증분 렌더 + 검색 debounce.

---

## 12. 디렉토리 / 스캐폴딩

```
2_App/GitMonitor/
├── docs/superpowers/specs/2026-06-04-git-monitor-dashboard-design.md  ← 본 문서
├── src/                       # Svelte 5
│   ├── App.svelte             # 진입점: repos_updated listen, 스토어 초기화
│   ├── lib/                   # store, types(snake_case), ipc 래퍼
│   └── components/            # Grid, RepoCard(navigator.clipboard), Header, Settings, EmptyState, ActionMenu
├── src-tauri/                 # Rust
│   ├── src/{main,lib,config,discovery,git_reader,scheduler,actions}.rs
│   ├── capabilities/default.json   # window/event 기본 + dialog:default. shell/clipboard scope 불요
│   ├── tests/
│   ├── Cargo.toml             # tauri 2, tauri-plugin-dialog, dirs_next, globset, serde, tokio
│   └── tauri.conf.json        # identifier="com.dgitx.gitmonitor", single-instance
├── package.json               # @tauri-apps/api, @tauri-apps/plugin-dialog
├── vite/svelte/tsconfig
└── .github/workflows/         # AIAgentMonitor 재사용
```
AIAgentMonitor 설정 스켈레톤 복제 후 AI 전용 코드 제거, 위 모듈로 교체. `lib.rs`에 `.plugin(tauri_plugin_dialog::init())`, single-instance, `WindowEvent::Focused` 리스너 등록.

---

## 13. 구현 마일스톤 (구현 계획 분해 가이드)
검증 게이트 확보를 위해 후속 구현 계획서를 최소 3개로 분해:
- **M1 — 백엔드 코어(UI 0)**: `config`(버저닝 포함) + `discovery`(글롭/category/.git 디렉토리 판정) + `git_reader`(porcelain v2 파서 + stash/state/last_fetch/worktree, 모든 엣지케이스). Rust 단위/통합 테스트가 수용 기준. CLI/테스트로 `RepoStatus` 산출 검증.
- **M2 — Tauri 통합**: `.manage(AppState)`, 5개 커맨드, `repos_updated`+EmitGate+seq, `scheduler`(WindowEvent::Focused 게이팅, 단일 in-flight), `actions`(process::Command).
- **M3 — 프론트**: Svelte 그리드/카드/정렬·필터(clean 술어)/설정 패널/empty-state/액션(navigator.clipboard)/dialog 폴더 선택/드래그&드롭.

각 마일스톤 끝에 회귀 검증 게이트.

---

## 14. 확인된 결정 사항
- **워크트리 배지**: "워킹트리"는 워킹트리(미커밋) 상태로 해석(staged/modified/untracked로 커버). git **worktree** 표기(`+N worktree`, N=worktrees-1)는 **v1 stretch(여유 시)** 로 강등 — 핵심 수용 기준 제외, M3 완료 후 시간이 남으면 추가.
- **fetch 정책**: 로컬 전용, 네트워크 0.
- **UI 형태**: 윈도우 그리드 전용, 메뉴바 트레이 없음.

## 15. 향후 (v1 범위 밖)
선택적 수동 fetch 버튼 / 메뉴바 트레이 요약 배지 / repo dirty 알림. v1 완료 후 별도 검토.
