<script lang="ts">
  import { onMount } from "svelte";
  import { store } from "./lib/store.svelte";
  import { theme } from "./lib/theme.svelte";
  import Header from "./components/Header.svelte";
  import Grid from "./components/Grid.svelte";
  import Settings from "./components/Settings.svelte";
  import EmptyState from "./components/EmptyState.svelte";
  import type { Config, RepoStatus } from "./lib/types";

  let search = $state("");
  let problemsOnly = $state(false);
  let showSettings = $state(false);
  let now = $state(Math.floor(Date.now() / 1000));
  let ctxMenu = $state<{ repo: RepoStatus; x: number; y: number } | null>(null);

  onMount(() => {
    theme.init();
    store.init();
    const t = setInterval(() => (now = Math.floor(Date.now() / 1000)), 30000);
    return () => {
      clearInterval(t);
      store.dispose();
    };
  });

  const staleDays = $derived(store.config?.stale_fetch_days ?? 7);

  async function saveConfig(c: Config) {
    await store.saveConfig(c);
    showSettings = false;
  }

  function openContext(repo: RepoStatus, e: MouseEvent) {
    e.preventDefault();
    ctxMenu = { repo, x: e.clientX, y: e.clientY };
  }

  async function excludeCurrent() {
    if (!ctxMenu) return;
    const path = ctxMenu.repo.path;
    ctxMenu = null;
    await store.excludeRepo(path);
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
    <Grid repos={store.repos} {now} {staleDays} {problemsOnly} {search} onContext={openContext} />
  {/if}

  {#if showSettings && store.config}
    <Settings config={store.config} onSave={saveConfig} onClose={() => (showSettings = false)} />
  {/if}

  {#if ctxMenu}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="ctx-backdrop"
      onclick={() => (ctxMenu = null)}
      oncontextmenu={(e) => {
        e.preventDefault();
        ctxMenu = null;
      }}
    ></div>
    <div class="ctx-menu" style="left: {ctxMenu.x}px; top: {ctxMenu.y}px">
      <button onclick={excludeCurrent}>이 프로젝트 제외하기</button>
    </div>
  {/if}
</main>

<style>
  main {
    font-family: -apple-system, system-ui, sans-serif;
  }
  .ctx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
  }
  .ctx-menu {
    position: fixed;
    z-index: 101;
    background: var(--card-bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 2px 12px var(--shadow);
    padding: 4px;
    min-width: 160px;
  }
  .ctx-menu button {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    color: var(--fg);
    padding: 6px 10px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.85em;
  }
  .ctx-menu button:hover {
    background: var(--badge-bg);
  }
</style>
