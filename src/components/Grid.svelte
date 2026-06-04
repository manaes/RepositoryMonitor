<script lang="ts">
  import type { RepoStatus } from "../lib/types";
  import { groupByCategory, filterProblems } from "../lib/logic";
  import RepoCard from "./RepoCard.svelte";

  let { repos, now, staleDays, problemsOnly, search }: {
    repos: RepoStatus[];
    now: number;
    staleDays: number;
    problemsOnly: boolean;
    search: string;
  } = $props();

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
  .group {
    margin: 12px 0;
  }
  h2 {
    font-size: 0.9em;
    color: #444;
    border-bottom: 1px solid #eee;
    padding-bottom: 4px;
  }
  .count {
    color: #999;
    font-weight: 400;
  }
  .cards {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
  }
</style>
