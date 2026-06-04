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
