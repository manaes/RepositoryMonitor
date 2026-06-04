<script lang="ts">
  import type { RepoStatus } from "../lib/types";
  import { summarize } from "../lib/logic";

  let { repos, search = $bindable(), problemsOnly = $bindable(), onRefresh, onSettings }: {
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
  header {
    position: sticky;
    top: 0;
    background: #fff;
    border-bottom: 1px solid #eee;
    padding: 8px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .search {
    flex: 0 1 200px;
  }
  .summary {
    font-size: 0.78em;
    color: #777;
    margin-top: 4px;
  }
</style>
