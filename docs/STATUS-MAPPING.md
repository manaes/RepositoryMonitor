# 상태 매핑 (Status Mapping)

VCS의 명령 출력이 카드 신호로 어떻게 변환되는지 정리한다. git/svn의 출력 형식은 **외부 의존성**이므로(버전에 따라 바뀔 수 있음), 파서를 수정할 때 이 문서를 함께 갱신한다. 모든 명령은 **read-only 로컬**이며 네트워크를 타지 않는다.

## 공통 데이터 모델 (`RepoStatus`)

```
path, name, category, vcs(git|svn),
branch?, detached_sha?, upstream?, has_upstream,
ahead?, behind?,            # Option — 없으면 미표시
staged, modified, untracked, conflicts, stash,
is_clean, state(clean|merging|rebasing|cherry_picking|reverting|bisecting),
worktrees, last_fetch?, last_checked, error?
```

---

## git — `git status --porcelain=v2 --branch`

단일 호출로 브랜치·upstream·ahead/behind·파일별 XY 코드를 모두 얻는다.

| 출력 라인 | 매핑 | 주의 (실측) |
|---|---|---|
| `# branch.oid <v>` | detached일 때 `detached_sha` | 빈 repo(커밋 0)는 `(initial)` — sha로 저장 금지 |
| `# branch.head <v>` | `(detached)`면 `branch=None`, 아니면 브랜치명 | rebase/bisect 중에도 `(detached)` |
| `# branch.upstream <v>` | `upstream`, `has_upstream=true` | **upstream 없으면 이 라인 자체가 없음** |
| `# branch.ab +<n> -<m>` | `ahead=n`, `behind=m` | **upstream 없으면 이 라인도 없음** → ahead/behind=None. in-sync는 `+0 -0` |
| `1 <XY> …` / `2 <XY> …` | X≠`.`→staged++, Y≠`.`→modified++ | XY는 2번째 토큰. `2`(rename)는 score(`R100`)+TAB 분리 경로 — 카운트만 하므로 안전 |
| `u <xy> …` | conflicts++ | 충돌은 **오직 u 라인** (1/2와 겹치지 않음) |
| `? …` | untracked++ | 디렉토리 단위로 접힘 |

**staged/modified는 "파일 수"가 아니라 축별 카운트** — 한 파일이 `MM`이면 staged·modified 양쪽에 +1.

### 보조 명령 (모두 로컬)

| 항목 | 명령 | 비고 |
|---|---|---|
| stash | `git stash list` 줄 수 | ⚠️ `rev-list --count refs/stash`는 stash 0개일 때 **exit 128 fatal** → 사용 금지 |
| state | `git rev-parse --git-path <marker>` 경로 존재 | 우선순위: Merging(`MERGE_HEAD`) > Rebasing(`rebase-merge`/`rebase-apply`) > CherryPicking > Reverting > Bisecting > Clean. **worktree에서 `.git`은 파일**이므로 리터럴 경로 대신 `rev-parse --git-path` 필수 |
| worktree | `git worktree list --porcelain`의 `worktree ` 라인 수 | 메인 포함(>=1). 카드는 `worktrees-1`을 `+N worktree`로 |
| last_fetch | `rev-parse --git-path FETCH_HEAD` mtime (없으면 common-dir 폴백) | 없으면 `None`("never fetched") |

---

## svn — `svn status` + `svn info` (로컬 전용)

> ⚠️ `svn`은 git의 `-C <dir>` 옵션이 없다 → `Command::new("svn").current_dir(repo)`로 작업 디렉토리 지정. **`svn status -u`는 네트워크를 타므로 사용 금지**(behind 생략).

`svn status` 출력 col0:

| col0 | 매핑 |
|---|---|
| `M` `A` `D` `R` `!` `~` | modified++ |
| `?` | untracked++ |
| `C` | conflicts++ |
| `I` `X` 공백 | 스킵 (단, col1(prop)이 `M`→modified, `C`→conflict) |

브랜치는 `svn info --show-item relative-url`(로컬):

| relative-url | branch |
|---|---|
| `^/trunk` | `trunk` |
| `^/branches/<b>` | `<b>` |
| `^/tags/<t>` | `tags/<t>` |
| 그 외 | 첫 세그먼트 |

svn은 **staging·stash·ahead 개념이 없어** 해당 필드는 0/None. `has_upstream=true`(항상 repo URL 보유 → "no upstream" 배지 미표시). `state=Clean`, `worktrees=1`, `last_fetch=None`(fetched 라인 미표시).

---

## 표시 규칙 (프론트 `logic.ts`)

### `clean` 단일 술어 (정렬·필터·렌더 공통)

```
clean := is_clean && state==Clean && conflicts==0 && stash==0
         && (ahead ?? 0)==0 && (behind ?? 0)==0 && worktrees<=1 && error==None
```

> ⚠️ stash·worktree>1도 비클린에 포함된다 — 커밋/푸시가 끝났어도 stash가 남아있으면 카드가 붉게 강조된다(의도된 동작이나 혼동될 수 있음).

### 정렬 — rank(낮을수록 위) + tie-break

```
rank = conflicts>0 ? 0 : state!=Clean ? 1 : behind>0 ? 2 : !is_clean ? 3 : ahead>0 ? 4 : 5
tie-break: 변경파일 총합↓ → behind↓ → ahead↓ → category↑ → name↑
```
정렬은 카테고리 그룹 내부에 적용.

### 필터 / 강조 / 시각

- **`문제만 보기`**: `!clean`인 repo만 표시(같은 술어 재사용).
- **강조**: 비클린 카드 = 붉은 배경 + 좌측 스트라이프(conflict는 더 강), clean 카드 = `opacity 0.6`.
- **fetched 표기**(git만): `None`→`never fetched`, `<1h`→`just now`, `<24h`→`Nh ago`, `≥24h`→`Nd ago`. `last_fetch`가 `stale_fetch_days`(기본 7) 이상이면 behind를 흐리게.

---

## behind의 정직성 (왜 로컬 전용인가)

이 앱은 **네트워크를 타지 않는다**(fetch/svn -u 없음). 따라서 git `behind`는 *마지막으로 fetch한 시점* 기준의 로컬 계산이다. 카드의 `fetched Nd ago`가 그 신선도를 알려주고, 오래되면 수치를 흐리게 해 "이 값은 최신이 아닐 수 있음"을 시각적으로 전달한다. svn은 out-of-date 확인 자체가 서버 왕복이라 **표시하지 않는다**. 정확한 원격 동기 상태가 필요하면 해당 repo에서 직접 `git fetch` / `svn status -u`를 실행한다.
