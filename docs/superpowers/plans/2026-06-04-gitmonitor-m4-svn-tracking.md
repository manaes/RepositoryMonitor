# GitMonitor M4 — SVN 추적 설계/구현 명세

**Goal:** git에 더해 SVN 작업복사본(`.svn`)도 추적. **로컬 전용(네트워크 0) 유지** — SVN의 out-of-date(behind)는 `svn status -u`가 네트워크를 타므로 **생략**.

**원칙:** VCS 추상화(`VcsKind`)를 도입해 discovery가 git/svn 모두 감지하고, repo별로 적절한 reader를 호출. RepoStatus는 git-shaped 그대로 두되 SVN에 없는 필드는 0/None. 프론트는 vcs에 따라 표시 적응.

**SVN ↔ RepoStatus 매핑(확정)**
- `svn status`(로컬, `-u` 안 씀) col0: `M/A/D/R/!/~` → **modified**++, `?` → **untracked**++, `C` → **conflicts**++. 또한 col1(prop)이 `M` → modified++, `C` → conflicts++. `I`/`X`/공백 → 스킵.
- `staged`/`stash`/`ahead`/`behind` = 0/None (SVN 개념 없음 — 로컬 전용이라 behind도 안 봄).
- `branch` = `svn info --show-item relative-url`(로컬, 네트워크 0)의 `^/trunk`→`trunk`, `^/branches/<b>`→`<b>`, `^/tags/<t>`→`tags/<t>`, 그 외 첫 세그먼트.
- `has_upstream` = true(SVN은 항상 repo URL 보유 → 카드의 "⊘ no upstream" 배지 미표시).
- `state` = Clean, `worktrees` = 1, `last_fetch` = None.
- `is_clean` = M/A/D/R/C/?/! 라인 없음.

**실측 픽스처**(`svn status` 로컬):
```
M       a.txt
A       b.txt
?       untracked.txt
```
(클린 WC → 빈 출력, exit 0). `svn info --show-item relative-url` → `^/trunk`.

---

## 변경 사항

### 1. `src-tauri/src/model.rs`
- 추가:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum VcsKind { #[default] Git, Svn }
  ```
- `RepoRef`에 `pub vcs: VcsKind` 추가. `RepoStatus`에 `pub vcs: VcsKind` 추가.
- `RepoStatus::from_ref`에서 `vcs: repo.vcs` 세팅.
- **기존 테스트 literal 수정**: model.rs tests의 RepoRef/RepoStatus 리터럴에 `vcs: VcsKind::Git` 추가.

### 2. `src-tauri/src/discovery.rs`
- `pub fn is_svn_repo_dir(path: &Path) -> bool { path.join(".svn").is_dir() }`
- `PRUNE_DIRS`에 `".svn"` 추가.
- discover() 루프: dir이 `is_git_repo_dir` → vcs=Git, else `is_svn_repo_dir` → vcs=Svn 로 repo 추가. `push_repo`에 `vcs: VcsKind` 인자 추가해 RepoRef에 세팅. manual_paths도 git/svn 판정.
- 단위테스트: `.svn` 디렉토리만 있는 임시 트리 → discover가 vcs=Svn으로 잡는지. 기존 `.git` 테스트는 vcs=Git 확인 추가(선택).

### 3. `src-tauri/src/svn_reader.rs` (신규)
```rust
use crate::model::{RepoRef, RepoStatus};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SvnParsed { pub modified: u32, pub untracked: u32, pub conflicts: u32, pub is_clean: bool }

/// `svn status`(로컬) 출력 파싱.
pub fn parse_svn_status(text: &str) -> SvnParsed {
    let mut s = SvnParsed::default();
    for line in text.lines() {
        if line.trim().is_empty() { continue; }
        let b = line.as_bytes();
        let c0 = *b.first().unwrap_or(&b' ');
        let c1 = if b.len() > 1 { b[1] } else { b' ' };
        match c0 {
            b'M' | b'A' | b'D' | b'R' | b'!' | b'~' => s.modified += 1,
            b'?' => s.untracked += 1,
            b'C' => s.conflicts += 1,
            b'I' | b'X' => {} // ignored/external
            _ => {
                // col0 공백이어도 prop 변경(col1)이 있으면 반영
                if c1 == b'M' { s.modified += 1; }
                else if c1 == b'C' { s.conflicts += 1; }
            }
        }
    }
    s.is_clean = s.modified == 0 && s.untracked == 0 && s.conflicts == 0;
    s
}

/// `svn info --show-item relative-url`(예: "^/trunk") → 브랜치명.
pub fn svn_branch(relurl: &str) -> Option<String> {
    let s = relurl.trim().trim_start_matches("^/").trim_start_matches('/');
    if s.is_empty() { return None; }
    let parts: Vec<&str> = s.split('/').collect();
    match parts.as_slice() {
        ["trunk", ..] => Some("trunk".to_string()),
        ["branches", b, ..] => Some((*b).to_string()),
        ["tags", t, ..] => Some(format!("tags/{t}")),
        [first, ..] => Some((*first).to_string()),
        [] => None,
    }
}

fn run_svn(repo: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("svn").arg("-C"); // ← 주의: svn은 -C 없음! 아래 참고
    unreachable!()
}
```
> **주의(중요)**: `svn`은 git의 `-C <dir>` 옵션이 없다. `std::process::Command::new("svn").current_dir(repo).args(...)`로 작업 디렉토리를 지정해야 한다. run_svn은 다음과 같이:
```rust
fn run_svn(repo: &str, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("svn")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|e| format!("svn 실행 실패: {e}"))?;
    if !out.status.success() {
        return Err(format!("svn {:?} 실패: {}", args, String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// RepoRef(vcs=Svn)에 SVN 상태를 채워 RepoStatus 생성. 로컬 전용(네트워크 0).
pub fn read_svn_status(repo: &RepoRef, now: i64) -> RepoStatus {
    let mut st = RepoStatus::from_ref(repo, now); // vcs=Svn 포함
    st.has_upstream = true; // SVN은 항상 repo URL 보유
    match run_svn(&repo.path, &["status"]) {
        Ok(text) => {
            let p = parse_svn_status(&text);
            st.modified = p.modified;
            st.untracked = p.untracked;
            st.conflicts = p.conflicts;
            st.is_clean = p.is_clean;
            // 브랜치(로컬): svn info --show-item relative-url
            if let Ok(relurl) = run_svn(&repo.path, &["info", "--show-item", "relative-url"]) {
                st.branch = svn_branch(&relurl);
            }
        }
        Err(e) => { st.error = Some(e); }
    }
    st
}
```
- 단위테스트: `parse_svn_status`(실측 픽스처 + 클린 빈 문자열 + conflict 'C' + prop-only ' M'), `svn_branch`(`^/trunk`/`^/branches/feat`/`^/tags/v1`/`^/` → None).
- 통합테스트(`tests/svn_integration.rs`, svn 설치됨): `svnadmin create` + layout(`svn mkdir trunk/branches/tags`) + `svn checkout file://.../trunk wc` + add/commit/modify/untracked → `read_svn_status` 단언(modified/untracked, branch=="trunk", error None, is_clean=false). **주의**: 임시 디렉토리 경로로 `file://` URL 구성. svn 명령은 `current_dir` 또는 절대 URL 사용.

### 4. `src-tauri/src/lib.rs`
- `pub mod svn_reader;` 추가.

### 5. `src-tauri/src/batch.rs`
- `read_status` 호출을 vcs 디스패치로 교체:
  ```rust
  use crate::model::VcsKind;
  // spawn_blocking 내부:
  let st = match repo.vcs {
      VcsKind::Git => crate::git_reader::read_status(&repo, now),
      VcsKind::Svn => crate::svn_reader::read_svn_status(&repo, now),
  };
  ```
- timeout/실패 시 from_ref(vcs 포함) 기반 error status는 그대로.

### 6. 기존 Rust 테스트 literal 수정(필수)
RepoRef/RepoStatus를 **구조체 리터럴로 만드는 모든 테스트**에 `vcs: VcsKind::Git` 추가:
- `model.rs` (tests)
- `tests/git_reader_integration.rs` (`ref_for`)
- `tests/batch_integration.rs` (`init_repo`, missing RepoRef)
- `src/snapshot.rs` (tests의 `rref`)
- `src/commands.rs` (tests에서 RepoStatus/RepoRef 리터럴 있으면)
- `src/emit_gate.rs` (from_ref 사용이면 자동 — 확인)
`from_ref`를 쓰는 곳은 자동으로 vcs가 채워지므로 수정 불필요. **`cargo test`로 컴파일 에러를 전부 잡아 수정.**

### 7. 프론트 `src/lib/types.ts`
- `export type VcsKind = "git" | "svn";`
- `RepoRef`와 `RepoStatus`에 `vcs: VcsKind` 추가.

### 8. 프론트 `src/components/RepoCard.svelte`
- head 또는 signals에 SVN 배지: `{#if repo.vcs === "svn"}<span class="badge vcs">SVN</span>{/if}`.
- fetched meta 라인을 git에만: `{#if repo.vcs === "git"}<div class="meta">{formatFetched(repo.last_fetch, now)}</div>{/if}` (SVN은 fetch 개념 없음 → 숨김).
- ahead/behind/staged/stash/worktree는 SVN에서 0/None이라 자연 미표시. no-upstream 배지는 has_upstream=true라 미표시.
- `.badge.vcs` 스타일(예: `background: var(--accent); color: var(--accent-fg);` 작은 배지).

---

## 완료 기준 (DoD)
- `cargo test` 전체 통과(기존 + svn 단위/통합 신규). `cargo clippy --all-targets -- -D warnings` 무경고. `cargo build` 성공.
- `pnpm check` 0 errors/0 warnings, `pnpm test` 24 통과(로직 불변), `pnpm build` 성공.
- 네트워크 0 유지(`svn status -u`/원격 호출 금지 — `svn status`·`svn info`는 로컬).
- GUI: SVN repo가 카드에 "SVN" 배지 + dirty 신호 + 브랜치(trunk 등) 표시, fetched 라인 없음. (시각 확인은 사용자 몫.)
