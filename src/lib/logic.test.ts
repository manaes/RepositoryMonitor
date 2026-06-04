import { describe, it, expect } from "vitest";
import { isClean, rank, compareRepos, filterProblems, formatFetched, isStale, groupByCategory, summarize } from "./logic";
import type { RepoStatus } from "./types";

function mk(over: Partial<RepoStatus>): RepoStatus {
  return {
    path: "/r", name: "r", category: "c", branch: "main", detached_sha: null,
    upstream: "origin/main", has_upstream: true, ahead: 0, behind: 0,
    staged: 0, modified: 0, untracked: 0, conflicts: 0, stash: 0,
    is_clean: true, state: "clean", worktrees: 1, last_fetch: null, last_checked: 0, error: null,
    vcs: "git",
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
