# GitMonitor M3 — Svelte 5 프론트엔드 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use checkbox(`- [ ]`).

**Goal:** M2의 Tauri 백엔드(IPC 5커맨드 + repos_updated 이벤트) 위에 Svelte 5 그리드 UI를 올려 실제 동작하는 GitMonitor 앱을 완성한다.

**Architecture:** Svelte 5(runes) + Vite + TS. AIAgentMonitor 프론트 패턴 적응(types/tauri 래퍼 + reactive store class + 컴포넌트). 백엔드 `repos_updated` 이벤트를 store가 구독(seq로 오래된 스냅샷 폐기)하고, 카드 그리드를 카테고리별로 렌더. 표시 규칙(clean 술어·정렬 rank·필터·시간 포맷·그룹·요약)은 **순수 TS 모듈 `logic.ts`로 분리해 vitest 단위 테스트**한다. 컴포넌트는 `svelte-check`(타입) + `vite build` + `cargo build`로 검증. **GUI 시각 검증은 이 환경에서 불가 — 사용자가 `cargo tauri dev`로 수동 확인.**

**Tech Stack:** svelte ^5, vite ^5, @tauri-apps/api ^2, @tauri-apps/plugin-dialog ^2, @tauri-apps/cli ^2, typescript ~5.6, svelte-check ^4, vitest ^2, @testing 없음(렌더 테스트 생략). pnpm.

**Spec:** §5(first-run/empty-state), §6(설정 재적용), §7(UI/카드 신호/정렬/필터). **타입은 Rust serde snake_case 그대로 미러링**(이벤트 payload는 camelCase 자동변환 없음).

**참조:** `/Users/wannypark/Desktop/@Projects/2_App/AIAgentMonitor` (src/lib/tauri.ts·store.svelte.ts·App.svelte 패턴)

---

## File Structure

| 파일 | 책임 |
|---|---|
| `package.json` | 프론트 deps/scripts(dev/build/check/test/tauri) |
| `vite.config.ts` | svelte 플러그인 + vitest 설정 + dev 서버 1420 |
| `svelte.config.js`, `tsconfig.json` | Svelte 5 + TS 설정 |
| `index.html` | (repo 루트) Vite 진입 HTML |
| `src/main.ts` | Svelte mount |
| `src/app.css` | 글로벌 스타일(최소) |
| `src/lib/types.ts` | Rust 타입 미러(snake_case) |
| `src/lib/tauri.ts` | invoke/listen 래퍼 |
| `src/lib/logic.ts` | 순수 표시 로직(clean/rank/정렬/필터/포맷/그룹/요약) ← vitest |
| `src/lib/logic.test.ts` | vitest 단위 테스트 |
| `src/lib/store.svelte.ts` | reactive store(repos/config/seq, init/dispose) |
| `src/components/RepoCard.svelte` | repo 카드(§7 신호 + 액션) |
| `src/components/Grid.svelte` | 카테고리 그룹 + 정렬/필터된 카드 그리드 |
| `src/components/Header.svelte` | 검색/문제만/정렬/새로고침/요약 |
| `src/components/Settings.svelte` | 루트/제외/주기/터미널/스캔깊이/stale |
| `src/components/EmptyState.svelte` | 루트 0개 first-run CTA |
| `src/App.svelte` | 루트 유무에 따라 EmptyState/메인 렌더, store init/dispose |
| `src-tauri/tauri.conf.json` | (수정) beforeDevCommand/beforeBuildCommand/devUrl 추가 |
| `.gitignore` | (수정) node_modules, dist 추가; 커밋된 dist 플레이스홀더 제거 |

**검증(매 태스크):** `pnpm check`(svelte-check 타입). 로직 태스크는 `pnpm test`(vitest). 최종: `pnpm build`(→dist) + `cd src-tauri && cargo build`.

---

## Task 0: 프론트 스캐폴딩 (컨트롤러가 수행) — 참고용 기록

> 이 태스크는 pnpm install 네트워크/시간 때문에 **컨트롤러(메인 세션)가 미리 수행**한다. 워크플로우는 Task 1부터. 아래는 무엇이 만들어졌는지 기록.

**package.json** (repo 루트):
```json
{
  "name": "gitmonitor",
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "test": "vitest run",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^4",
    "@tauri-apps/cli": "^2",
    "@tsconfig/svelte": "^5",
    "svelte": "^5",
    "svelte-check": "^4",
    "typescript": "~5.6",
    "vite": "^5",
    "vitest": "^2"
  }
}
```

**vite.config.ts**:
```ts
/// <reference types="vitest" />
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  test: { environment: "node", include: ["src/**/*.test.ts"] },
});
```

**svelte.config.js**:
```js
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
export default { preprocess: vitePreprocess() };
```

**tsconfig.json**:
```json
{
  "extends": "@tsconfig/svelte/tsconfig.json",
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"]
  },
  "include": ["src/**/*.ts", "src/**/*.svelte"]
}
```

**index.html** (repo 루트):
```html
<!doctype html>
<html lang="ko">
  <head><meta charset="UTF-8" /><title>GitMonitor</title></head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

**src/main.ts**:
```ts
import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

const app = mount(App, { target: document.getElementById("app")! });
export default app;
```

**src/app.css**: 최소 글로벌(폰트/리셋/색 변수).

**src-tauri/tauri.conf.json** `build` 섹션을 다음으로 갱신:
```json
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
```

**.gitignore** (repo 루트)에 추가: `node_modules/`, `/dist`. 그리고 M2에서 커밋했던 `dist/index.html` 플레이스홀더를 `git rm`(이제 vite가 dist를 생성).

**pnpm-workspace.yaml** (repo 루트, 필수): pnpm v11은 의존성 빌드 스크립트(esbuild의 네이티브 바이너리 셋업)를 기본 차단(`ERR_PNPM_IGNORED_BUILDS`)하고 `package.json`의 `pnpm` 필드도 안 읽는다. 다음을 둬야 `pnpm install`/`build`가 동작:
```yaml
packages:
  - .

allowBuilds:
  esbuild: true
```

App.svelte는 Task 1~7에서 채워지므로, T0에서는 **임시 스텁** `src/App.svelte`(`<script lang="ts"></script><main>GitMonitor</main>`)을 두어 `pnpm build`가 통과하도록 한다.

**T0 검증(컨트롤러)**: `pnpm install` → `pnpm build`(dist 생성) → `cd src-tauri && cargo build`(Tauri가 실제 dist로 빌드) 성공.

---

## Task 1: types.ts + tauri.ts (IPC 계약)

**Files:** Create `src/lib/types.ts`, `src/lib/tauri.ts`

- [ ] **Step 1: types.ts 작성 (Rust serde snake_case 미러)**

```ts
export type RepoState =
  | "clean" | "merging" | "rebasing" | "cherry_picking" | "reverting" | "bisecting";

export interface RepoRef {
  path: string;
  name: string;
  category: string;
}

export interface RepoStatus {
  path: string;
  name: string;
  category: string;
  branch: string | null;
  detached_sha: string | null;
  upstream: string | null;
  has_upstream: boolean;
  ahead: number | null;
  behind: number | null;
  staged: number;
  modified: number;
  untracked: number;
  conflicts: number;
  stash: number;
  is_clean: boolean;
  state: RepoState;
  worktrees: number;
  last_fetch: number | null;
  last_checked: number;
  error: string | null;
}

export type TerminalApp = "terminal" | "iterm" | { custom: string };

export interface Config {
  version: number;
  roots: string[];
  manual_paths: string[];
  exclude_globs: string[];
  poll_interval_secs: number;
  scan_depth: number;
  stale_fetch_days: number;
  terminal_app: TerminalApp;
}

export interface RepoSnapshot {
  seq: number;
  repos: RepoStatus[];
}

export type ActionKind = "open_finder" | "open_terminal" | "open_source_tree";
```

- [ ] **Step 2: tauri.ts 작성**

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Config, RepoRef, RepoSnapshot, ActionKind } from "./types";

export const getConfig = (): Promise<Config> => invoke("get_config");
export const setConfig = (config: Config): Promise<void> => invoke("set_config", { config });
export const scanRepos = (): Promise<RepoRef[]> => invoke("scan_repos");
export const refreshStatus = (): Promise<void> => invoke("refresh_status");
// Rust open_action(repo_path, kind) → invoke 인자는 camelCase(repoPath)
export const openAction = (repoPath: string, kind: ActionKind): Promise<void> =>
  invoke("open_action", { repoPath, kind });
export const listenReposUpdated = (cb: (s: RepoSnapshot) => void): Promise<UnlistenFn> =>
  listen<RepoSnapshot>("repos_updated", (e) => cb(e.payload));
```

- [ ] **Step 3: 타입체크**

Run: `pnpm check`
Expected: 0 errors(types/tauri만으로는 미사용 경고 없음 — export라서).

- [ ] **Step 4: Commit**

```bash
git add src/lib/types.ts src/lib/tauri.ts
git commit -m "feat(m3): types(Rust 미러) + tauri IPC 래퍼

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: logic.ts + vitest (표시 로직 — 테스트 핵심)

**Files:** Create `src/lib/logic.ts`, `src/lib/logic.test.ts`

- [ ] **Step 1: 실패 테스트 작성 — logic.test.ts**

```ts
import { describe, it, expect } from "vitest";
import { isClean, rank, compareRepos, filterProblems, formatFetched, isStale, groupByCategory, summarize } from "./logic";
import type { RepoStatus } from "./types";

function mk(over: Partial<RepoStatus>): RepoStatus {
  return {
    path: "/r", name: "r", category: "c", branch: "main", detached_sha: null,
    upstream: "origin/main", has_upstream: true, ahead: 0, behind: 0,
    staged: 0, modified: 0, untracked: 0, conflicts: 0, stash: 0,
    is_clean: true, state: "clean", worktrees: 1, last_fetch: null, last_checked: 0, error: null,
    ...over,
  };
}

describe("isClean", () => {
  it("clean repo", () => expect(isClean(mk({}))).toBe(true));
  it("dirty by staged", () => expect(isClean(mk({ staged: 1, is_clean: false }))).toBe(false));
  it("ahead breaks clean", () => expect(isClean(mk({ ahead: 2 }))).toBe(false));
  it("merging breaks clean", () => expect(isClean(mk({ state: "merging" }))).toBe(false));
  it("error breaks clean", () => expect(isClean(mk({ error: "x" }))).toBe(false));
  it("extra worktree breaks clean", () => expect(isClean(mk({ worktrees: 2 }))).toBe(false));
});

describe("rank", () => {
  it("conflict highest", () => expect(rank(mk({ conflicts: 1 }))).toBe(0));
  it("state next", () => expect(rank(mk({ state: "rebasing" }))).toBe(1));
  it("behind", () => expect(rank(mk({ behind: 1 }))).toBe(2));
  it("dirty", () => expect(rank(mk({ is_clean: false, modified: 1 }))).toBe(3));
  it("ahead", () => expect(rank(mk({ ahead: 1 }))).toBe(4));
  it("clean lowest", () => expect(rank(mk({}))).toBe(5));
});

describe("compareRepos", () => {
  it("sorts by rank then change count", () => {
    const a = mk({ name: "a", conflicts: 1 });
    const b = mk({ name: "b", modified: 5, is_clean: false });
    const c = mk({ name: "c" });
    const sorted = [c, b, a].sort(compareRepos).map((r) => r.name);
    expect(sorted).toEqual(["a", "b", "c"]);
  });
  it("tie-break by change count desc", () => {
    const a = mk({ name: "a", modified: 1, is_clean: false });
    const b = mk({ name: "b", modified: 5, is_clean: false });
    expect([a, b].sort(compareRepos).map((r) => r.name)).toEqual(["b", "a"]);
  });
});

describe("filterProblems", () => {
  it("hides clean when on", () => {
    const repos = [mk({ name: "clean" }), mk({ name: "dirty", staged: 1, is_clean: false })];
    expect(filterProblems(repos, true).map((r) => r.name)).toEqual(["dirty"]);
    expect(filterProblems(repos, false).length).toBe(2);
  });
});

describe("formatFetched / isStale", () => {
  it("never", () => expect(formatFetched(null, 1000)).toBe("never fetched"));
  it("just now", () => expect(formatFetched(1000, 1000 + 60)).toBe("fetched just now"));
  it("hours", () => expect(formatFetched(0, 3600 * 5)).toBe("fetched 5h ago"));
  it("days", () => expect(formatFetched(0, 86400 * 3)).toBe("fetched 3d ago"));
  it("stale when null", () => expect(isStale(null, 1000, 7)).toBe(true));
  it("stale after threshold", () => expect(isStale(0, 86400 * 8, 7)).toBe(true));
  it("not stale within", () => expect(isStale(0, 86400 * 2, 7)).toBe(false));
});

describe("groupByCategory", () => {
  it("groups and sorts categories + repos", () => {
    const repos = [
      mk({ name: "z", category: "B" }),
      mk({ name: "a", category: "A", conflicts: 1 }),
      mk({ name: "b", category: "A" }),
    ];
    const g = groupByCategory(repos);
    expect(g.map((x) => x.category)).toEqual(["A", "B"]);
    expect(g[0].repos.map((r) => r.name)).toEqual(["a", "b"]); // a(conflict) 우선
  });
});

describe("summarize", () => {
  it("counts dirty/behind/ahead", () => {
    const repos = [mk({}), mk({ staged: 1, is_clean: false }), mk({ behind: 2 }), mk({ ahead: 1 })];
    expect(summarize(repos)).toEqual({ total: 4, dirty: 3, behind: 1, ahead: 1 });
  });
});
```

- [ ] **Step 2: 실패 확인**

Run: `pnpm test`
Expected: 실패(`logic.ts` 미구현 — import 에러).

- [ ] **Step 3: 구현 — logic.ts**

```ts
import type { RepoStatus } from "./types";

/** spec §7 단일 clean 술어(정렬·필터·렌더 공통). */
export function isClean(s: RepoStatus): boolean {
  return (
    s.is_clean &&
    s.state === "clean" &&
    s.conflicts === 0 &&
    s.stash === 0 &&
    (s.ahead ?? 0) === 0 &&
    (s.behind ?? 0) === 0 &&
    s.worktrees <= 1 &&
    s.error === null
  );
}

/** 문제 심각도 rank(낮을수록 위). spec §7. */
export function rank(s: RepoStatus): number {
  if (s.conflicts > 0) return 0;
  if (s.state !== "clean") return 1;
  if ((s.behind ?? 0) > 0) return 2;
  if (!s.is_clean) return 3;
  if ((s.ahead ?? 0) > 0) return 4;
  return 5;
}

/** rank → 변경수 → behind → ahead → category → name. */
export function compareRepos(a: RepoStatus, b: RepoStatus): number {
  const ra = rank(a);
  const rb = rank(b);
  if (ra !== rb) return ra - rb;
  const ca = a.staged + a.modified + a.untracked + a.conflicts;
  const cb = b.staged + b.modified + b.untracked + b.conflicts;
  if (ca !== cb) return cb - ca;
  const bh = (b.behind ?? 0) - (a.behind ?? 0);
  if (bh !== 0) return bh;
  const ah = (b.ahead ?? 0) - (a.ahead ?? 0);
  if (ah !== 0) return ah;
  if (a.category !== b.category) return a.category.localeCompare(b.category);
  return a.name.localeCompare(b.name);
}

export function filterProblems(repos: RepoStatus[], problemsOnly: boolean): RepoStatus[] {
  return problemsOnly ? repos.filter((r) => !isClean(r)) : repos;
}

export function formatFetched(lastFetch: number | null, now: number): string {
  if (lastFetch === null) return "never fetched";
  const sec = now - lastFetch;
  if (sec < 3600) return "fetched just now";
  if (sec < 86400) return `fetched ${Math.floor(sec / 3600)}h ago`;
  return `fetched ${Math.floor(sec / 86400)}d ago`;
}

export function isStale(lastFetch: number | null, now: number, staleDays: number): boolean {
  if (lastFetch === null) return true;
  return now - lastFetch >= staleDays * 86400;
}

export interface CategoryGroup {
  category: string;
  repos: RepoStatus[];
}

export function groupByCategory(repos: RepoStatus[]): CategoryGroup[] {
  const map = new Map<string, RepoStatus[]>();
  for (const r of repos) {
    const list = map.get(r.category) ?? [];
    list.push(r);
    map.set(r.category, list);
  }
  const groups: CategoryGroup[] = [];
  for (const [category, list] of map) {
    groups.push({ category, repos: [...list].sort(compareRepos) });
  }
  groups.sort((a, b) => a.category.localeCompare(b.category));
  return groups;
}

export function summarize(repos: RepoStatus[]): { total: number; dirty: number; behind: number; ahead: number } {
  let dirty = 0;
  let behind = 0;
  let ahead = 0;
  for (const r of repos) {
    if (!isClean(r)) dirty++;
    if ((r.behind ?? 0) > 0) behind++;
    if ((r.ahead ?? 0) > 0) ahead++;
  }
  return { total: repos.length, dirty, behind, ahead };
}
```

- [ ] **Step 4: 통과 확인**

Run: `pnpm test`
Expected: 전체 테스트 통과(약 25+ assertions).

Run: `pnpm check`
Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add src/lib/logic.ts src/lib/logic.test.ts
git commit -m "feat(m3): logic.ts — clean술어/정렬rank/필터/시간포맷/그룹/요약 + vitest

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: store.svelte.ts (reactive 상태)

**Files:** Create `src/lib/store.svelte.ts`

- [ ] **Step 1: 구현**

```ts
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { Config, RepoStatus } from "./types";
import { getConfig, listenReposUpdated, refreshStatus, scanRepos } from "./tauri";

/** repos_updated 이벤트를 구독하는 reactive store. seq로 오래된 스냅샷 폐기. */
class GitMonitorStore {
  repos = $state<RepoStatus[]>([]);
  config = $state<Config | null>(null);
  lastSeq = $state(0);

  #unlisten: UnlistenFn | null = null;
  #initialized = false;

  async init(): Promise<void> {
    if (this.#initialized) return;
    this.#initialized = true;
    this.config = await getConfig();
    this.#unlisten = await listenReposUpdated((snap) => {
      if (snap.seq < this.lastSeq) return; // 오래된 스냅샷 폐기
      this.lastSeq = snap.seq;
      this.repos = snap.repos;
    });
  }

  dispose(): void {
    this.#unlisten?.();
    this.#unlisten = null;
    this.#initialized = false;
  }

  async refresh(): Promise<void> {
    await refreshStatus();
  }

  async rescan(): Promise<void> {
    await scanRepos();
    await refreshStatus();
  }

  async saveConfig(config: Config): Promise<void> {
    const { setConfig } = await import("./tauri");
    await setConfig(config);
    this.config = config;
    await this.rescan(); // 루트/제외 변경 반영
  }

  get hasRoots(): boolean {
    return (this.config?.roots.length ?? 0) > 0;
  }
}

export const store = new GitMonitorStore();
```

- [ ] **Step 2: 타입체크 + Commit**

Run: `pnpm check` → 0 errors.
```bash
git add src/lib/store.svelte.ts
git commit -m "feat(m3): store.svelte.ts — repos_updated 구독(seq 폐기)/config/init·dispose

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: RepoCard.svelte (§7 카드 신호 + 액션)

**Files:** Create `src/components/RepoCard.svelte`

- [ ] **Step 1: 구현 (Svelte 5 runes)**

```svelte
<script lang="ts">
  import type { RepoStatus, ActionKind } from "../lib/types";
  import { openAction } from "../lib/tauri";
  import { formatFetched, isStale, isClean } from "../lib/logic";

  let { repo, now, staleDays }: { repo: RepoStatus; now: number; staleDays: number } = $props();

  const clean = $derived(isClean(repo));
  const stale = $derived(isStale(repo.last_fetch, now, staleDays));

  async function act(kind: ActionKind) {
    try { await openAction(repo.path, kind); } catch (e) { console.error("openAction", e); }
  }
  async function copyPath() {
    try { await navigator.clipboard.writeText(repo.path); } catch (e) { console.error("copy", e); }
  }
  const stateLabel: Record<string, string> = {
    merging: "merging", rebasing: "rebasing", cherry_picking: "cherry-pick",
    reverting: "reverting", bisecting: "bisecting", clean: "",
  };
</script>

<div class="card" class:clean class:error={!!repo.error}>
  <div class="head">
    <span class="name">{repo.name}</span>
    {#if repo.error}
      <span class="badge err">⚠ error</span>
    {:else if repo.branch}
      <span class="branch">{repo.branch}</span>
    {:else if repo.detached_sha}
      <span class="branch detached">detached @{repo.detached_sha.slice(0, 7)}</span>
    {/if}
  </div>

  {#if !repo.error}
    <div class="signals">
      {#if repo.state !== "clean"}<span class="badge state">{stateLabel[repo.state]}</span>{/if}
      {#if !repo.has_upstream && repo.branch}<span class="badge">⊘ no upstream</span>{/if}
      {#if repo.ahead}<span class="badge ahead">↑{repo.ahead}</span>{/if}
      {#if repo.behind}<span class="badge behind" class:stale>↓{repo.behind}</span>{/if}
      {#if repo.staged}<span class="badge staged">+{repo.staged}</span>{/if}
      {#if repo.modified}<span class="badge modified">●{repo.modified}</span>{/if}
      {#if repo.untracked}<span class="badge untracked">?{repo.untracked}</span>{/if}
      {#if repo.conflicts}<span class="badge conflict">⚠{repo.conflicts}</span>{/if}
      {#if repo.stash}<span class="badge stash">⚑{repo.stash}</span>{/if}
      {#if repo.worktrees > 1}<span class="badge wt">+{repo.worktrees - 1} worktree</span>{/if}
      {#if clean}<span class="badge ok">✓ clean</span>{/if}
    </div>
    <div class="meta">{formatFetched(repo.last_fetch, now)}</div>
  {:else}
    <div class="meta err">{repo.error}</div>
  {/if}

  <div class="actions">
    <button title="Finder" onclick={() => act("open_finder")}>F</button>
    <button title="Terminal" onclick={() => act("open_terminal")}>T</button>
    <button title="SourceTree" onclick={() => act("open_source_tree")}>S</button>
    <button title="경로 복사" onclick={copyPath}>⧉</button>
  </div>
</div>

<style>
  .card { border: 1px solid #ddd; border-radius: 8px; padding: 8px 10px; min-width: 180px; display: flex; flex-direction: column; gap: 4px; }
  .card.clean { opacity: 0.8; }
  .card.error { border-color: #e55; }
  .head { display: flex; justify-content: space-between; align-items: baseline; gap: 8px; }
  .name { font-weight: 600; }
  .branch { color: #555; font-size: 0.85em; }
  .detached { font-style: italic; }
  .signals { display: flex; flex-wrap: wrap; gap: 4px; }
  .badge { font-size: 0.75em; padding: 1px 5px; border-radius: 4px; background: #eee; }
  .badge.conflict, .badge.err { background: #fdd; }
  .badge.behind.stale { opacity: 0.5; }
  .badge.ok { background: #dfd; }
  .meta { font-size: 0.72em; color: #888; }
  .meta.err { color: #c33; }
  .actions { display: flex; gap: 4px; margin-top: 2px; }
  .actions button { font-size: 0.72em; padding: 1px 6px; cursor: pointer; }
</style>
```

- [ ] **Step 2: 타입체크 + Commit**

Run: `pnpm check` → 0 errors.
```bash
git add src/components/RepoCard.svelte
git commit -m "feat(m3): RepoCard — §7 카드 신호 렌더 + Finder/터미널/SourceTree/경로복사 액션

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Grid.svelte + Header.svelte

**Files:** Create `src/components/Grid.svelte`, `src/components/Header.svelte`

- [ ] **Step 1: Grid.svelte 구현**

```svelte
<script lang="ts">
  import type { RepoStatus } from "../lib/types";
  import { groupByCategory, filterProblems } from "../lib/logic";
  import RepoCard from "./RepoCard.svelte";

  let { repos, now, staleDays, problemsOnly, search }:
    { repos: RepoStatus[]; now: number; staleDays: number; problemsOnly: boolean; search: string } = $props();

  const visible = $derived.by(() => {
    let r = filterProblems(repos, problemsOnly);
    const q = search.trim().toLowerCase();
    if (q) r = r.filter((x) => x.name.toLowerCase().includes(q));
    return groupByCategory(r);
  });
</script>

{#each visible as group (group.category)}
  <section class="group">
    <h2>{group.category} <span class="count">({group.repos.length})</span></h2>
    <div class="cards">
      {#each group.repos as repo (repo.path)}
        <RepoCard {repo} {now} {staleDays} />
      {/each}
    </div>
  </section>
{/each}

<style>
  .group { margin: 12px 0; }
  h2 { font-size: 0.9em; color: #444; border-bottom: 1px solid #eee; padding-bottom: 4px; }
  .count { color: #999; font-weight: 400; }
  .cards { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 8px; }
</style>
```

- [ ] **Step 2: Header.svelte 구현**

```svelte
<script lang="ts">
  import type { RepoStatus } from "../lib/types";
  import { summarize } from "../lib/logic";

  let { repos, search = $bindable(), problemsOnly = $bindable(), onRefresh, onSettings }:
    {
      repos: RepoStatus[];
      search: string;
      problemsOnly: boolean;
      onRefresh: () => void;
      onSettings: () => void;
    } = $props();

  const s = $derived(summarize(repos));
</script>

<header>
  <div class="row">
    <strong>GitMonitor</strong>
    <button onclick={onRefresh} title="새로고침">⟳</button>
    <button onclick={onSettings} title="설정">⚙</button>
    <input class="search" placeholder="search" bind:value={search} />
    <label><input type="checkbox" bind:checked={problemsOnly} /> 문제만</label>
  </div>
  <div class="summary">{s.total} repos · {s.dirty} dirty · {s.behind} behind · {s.ahead} ahead</div>
</header>

<style>
  header { position: sticky; top: 0; background: #fff; border-bottom: 1px solid #eee; padding: 8px; }
  .row { display: flex; align-items: center; gap: 8px; }
  .search { flex: 0 1 200px; }
  .summary { font-size: 0.78em; color: #777; margin-top: 4px; }
</style>
```

- [ ] **Step 3: 타입체크 + Commit**

Run: `pnpm check` → 0 errors.
```bash
git add src/components/Grid.svelte src/components/Header.svelte
git commit -m "feat(m3): Grid(카테고리 그룹/정렬/필터) + Header(검색/문제만/요약/새로고침)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Settings.svelte + EmptyState.svelte

**Files:** Create `src/components/Settings.svelte`, `src/components/EmptyState.svelte`

- [ ] **Step 1: EmptyState.svelte 구현**

```svelte
<script lang="ts">
  let { onAddRoot }: { onAddRoot: () => void } = $props();
</script>

<div class="empty">
  <p>스캔할 폴더가 없습니다.</p>
  <button onclick={onAddRoot}>루트 폴더 추가</button>
  <p class="hint">예: ~/Desktop/@Projects</p>
</div>

<style>
  .empty { text-align: center; padding: 60px 20px; color: #666; }
  .hint { font-size: 0.8em; color: #aaa; }
</style>
```

- [ ] **Step 2: Settings.svelte 구현**

```svelte
<script lang="ts">
  import type { Config } from "../lib/types";
  import { open } from "@tauri-apps/plugin-dialog";

  let { config, onSave, onClose }:
    { config: Config; onSave: (c: Config) => void; onClose: () => void } = $props();

  let draft = $state<Config>(structuredClone($state.snapshot(config)));

  async function addRoot() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") draft.roots = [...draft.roots, dir];
  }
  function removeRoot(i: number) { draft.roots = draft.roots.filter((_, j) => j !== i); }
  function save() { onSave($state.snapshot(draft)); }
</script>

<div class="panel">
  <div class="hd"><strong>설정</strong><button onclick={onClose}>✕</button></div>

  <section>
    <h3>스캔 루트</h3>
    {#each draft.roots as root, i (root)}
      <div class="item"><code>{root}</code><button onclick={() => removeRoot(i)}>제거</button></div>
    {/each}
    <button onclick={addRoot}>+ 폴더 추가</button>
  </section>

  <section>
    <h3>제외 글롭(줄바꿈 구분)</h3>
    <textarea
      rows="3"
      value={draft.exclude_globs.join("\n")}
      oninput={(e) => (draft.exclude_globs = e.currentTarget.value.split("\n").map((s) => s.trim()).filter(Boolean))}
    ></textarea>
  </section>

  <section class="grid2">
    <label>폴링 주기(초)<input type="number" min="10" max="300" bind:value={draft.poll_interval_secs} /></label>
    <label>스캔 깊이<input type="number" min="1" max="10" bind:value={draft.scan_depth} /></label>
    <label>stale 기준(일)<input type="number" min="1" bind:value={draft.stale_fetch_days} /></label>
    <label>터미널 앱
      <select value={typeof draft.terminal_app === "string" ? draft.terminal_app : "terminal"}
        onchange={(e) => (draft.terminal_app = e.currentTarget.value as "terminal" | "iterm")}>
        <option value="terminal">Terminal</option>
        <option value="iterm">iTerm</option>
      </select>
    </label>
  </section>

  <div class="ft"><button class="primary" onclick={save}>저장</button></div>
</div>

<style>
  .panel { position: fixed; right: 0; top: 0; bottom: 0; width: 360px; background: #fff; border-left: 1px solid #ddd; padding: 12px; overflow-y: auto; box-shadow: -2px 0 8px rgba(0,0,0,0.08); }
  .hd { display: flex; justify-content: space-between; }
  section { margin: 14px 0; }
  h3 { font-size: 0.85em; color: #555; }
  .item { display: flex; justify-content: space-between; gap: 8px; align-items: center; font-size: 0.8em; }
  .item code { word-break: break-all; }
  textarea { width: 100%; }
  .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .grid2 label { display: flex; flex-direction: column; font-size: 0.8em; gap: 2px; }
  .ft { margin-top: 16px; }
  .primary { background: #06c; color: #fff; border: none; padding: 6px 16px; border-radius: 6px; cursor: pointer; }
</style>
```

- [ ] **Step 3: 타입체크 + Commit**

Run: `pnpm check` → 0 errors.
```bash
git add src/components/Settings.svelte src/components/EmptyState.svelte
git commit -m "feat(m3): Settings(루트/제외/주기/깊이/stale/터미널 + dialog 폴더선택) + EmptyState

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: App.svelte 통합 + 최종 검증

**Files:** Modify `src/App.svelte` (T0 스텁 교체)

- [ ] **Step 1: App.svelte 구현**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { store } from "./lib/store.svelte";
  import Header from "./components/Header.svelte";
  import Grid from "./components/Grid.svelte";
  import Settings from "./components/Settings.svelte";
  import EmptyState from "./components/EmptyState.svelte";
  import type { Config } from "./lib/types";

  let search = $state("");
  let problemsOnly = $state(false);
  let showSettings = $state(false);
  let now = $state(Math.floor(Date.now() / 1000));

  onMount(() => {
    store.init();
    const t = setInterval(() => (now = Math.floor(Date.now() / 1000)), 30000);
    return () => { clearInterval(t); store.dispose(); };
  });

  const staleDays = $derived(store.config?.stale_fetch_days ?? 7);

  async function saveConfig(c: Config) {
    await store.saveConfig(c);
    showSettings = false;
  }
</script>

<main>
  {#if store.config && !store.hasRoots}
    <EmptyState onAddRoot={() => (showSettings = true)} />
  {:else}
    <Header
      repos={store.repos}
      bind:search
      bind:problemsOnly
      onRefresh={() => store.refresh()}
      onSettings={() => (showSettings = true)}
    />
    <Grid repos={store.repos} {now} {staleDays} {problemsOnly} {search} />
  {/if}

  {#if showSettings && store.config}
    <Settings config={store.config} onSave={saveConfig} onClose={() => (showSettings = false)} />
  {/if}
</main>

<style>
  main { font-family: -apple-system, system-ui, sans-serif; }
</style>
```

- [ ] **Step 2: 최종 검증**

Run: `pnpm check`
Expected: 0 errors.

Run: `pnpm test`
Expected: logic 테스트 전부 통과.

Run: `pnpm build`
Expected: vite 빌드 성공, `dist/` 생성.

Run: `cd src-tauri && cargo build`
Expected: Tauri 앱이 실제 dist로 빌드 성공.

Run: `cd src-tauri && cargo test --quiet 2>&1 | tail -2`
Expected: 백엔드 테스트 55 그대로 통과(프론트 변경은 Rust 무관).

- [ ] **Step 3: Commit**

```bash
git add src/App.svelte
git commit -m "feat(m3): App — EmptyState/Header+Grid/Settings 통합, store init·dispose, now 틱

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## M3 완료 기준 (Definition of Done)
- `pnpm test`(vitest logic) 통과, `pnpm check`(svelte-check) 0 errors, `pnpm build`(vite → dist) 성공, `cd src-tauri && cargo build` 성공.
- 백엔드 `cargo test` 55 유지.
- spec §7 카드 신호/정렬/필터/그룹·§5 empty-state·§6 설정이 코드로 구현됨.
- ⚠️ **시각/상호작용 검증은 사용자 몫**: `pnpm install`(완료됨) 후 `cd src-tauri && cargo tauri dev`(또는 `pnpm tauri dev`)로 실제 창을 띄워 그리드/액션/설정을 확인. 이 환경에선 GUI 자동검증 불가.

---

## Self-Review (작성자 점검 결과)
- **Spec 커버리지**: §7 카드 신호(RepoCard) ✅, 정렬 rank+tie-break/필터/그룹/요약(logic+Grid+Header) ✅, clean 단일 술어(logic.isClean, 정렬·필터 공통 사용) ✅, fetched 시간/stale(logic+RepoCard) ✅, first-run empty-state(EmptyState+App) ✅, 설정 재적용(store.saveConfig가 rescan 트리거) ✅, 액션 Finder/터미널/SourceTree/경로복사(navigator.clipboard) ✅, dialog 폴더선택 ✅. seq 기반 오래된 스냅샷 폐기(store) ✅.
- **Placeholder 스캔**: 모든 컴포넌트/로직에 실제 코드. CSS는 최소 기능형(시각 디테일은 사용자 조정 — DoD에 명시). 렌더 단위테스트는 생략(svelte-check+build로 대체, DoD 명시).
- **타입 일관성**: types.ts가 Rust serde snake_case와 1:1(RepoStatus/Config/RepoSnapshot/ActionKind/TerminalApp/RepoState). tauri.ts invoke 인자 camelCase(repoPath) 주의 반영. logic 함수 시그니처(isClean/rank/compareRepos/filterProblems/formatFetched/isStale/groupByCategory/summarize)가 컴포넌트 호출부와 일치. store 메서드(init/dispose/refresh/rescan/saveConfig/hasRoots)가 App 호출부와 일치.
