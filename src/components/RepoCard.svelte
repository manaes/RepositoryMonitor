<script lang="ts">
  import type { RepoStatus, ActionKind } from "../lib/types";
  import { openAction } from "../lib/tauri";
  import { formatFetched, isStale, isClean } from "../lib/logic";

  let { repo, now, staleDays }: { repo: RepoStatus; now: number; staleDays: number } = $props();

  const clean = $derived(isClean(repo));
  const stale = $derived(isStale(repo.last_fetch, now, staleDays));

  async function act(kind: ActionKind) {
    try {
      await openAction(repo.path, kind);
    } catch (e) {
      console.error("openAction", e);
    }
  }
  async function copyPath() {
    try {
      await navigator.clipboard.writeText(repo.path);
    } catch (e) {
      console.error("copy", e);
    }
  }
  const stateLabel: Record<string, string> = {
    merging: "merging",
    rebasing: "rebasing",
    cherry_picking: "cherry-pick",
    reverting: "reverting",
    bisecting: "bisecting",
    clean: "",
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
  .card {
    border: 1px solid #ddd;
    border-radius: 8px;
    padding: 8px 10px;
    min-width: 180px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .card.clean {
    opacity: 0.8;
  }
  .card.error {
    border-color: #e55;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
  }
  .name {
    font-weight: 600;
  }
  .branch {
    color: #555;
    font-size: 0.85em;
  }
  .detached {
    font-style: italic;
  }
  .signals {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .badge {
    font-size: 0.75em;
    padding: 1px 5px;
    border-radius: 4px;
    background: #eee;
  }
  .badge.conflict,
  .badge.err {
    background: #fdd;
  }
  .badge.behind.stale {
    opacity: 0.5;
  }
  .badge.ok {
    background: #dfd;
  }
  .meta {
    font-size: 0.72em;
    color: #888;
  }
  .meta.err {
    color: #c33;
  }
  .actions {
    display: flex;
    gap: 4px;
    margin-top: 2px;
  }
  .actions button {
    font-size: 0.72em;
    padding: 1px 6px;
    cursor: pointer;
  }
</style>
