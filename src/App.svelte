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
  main {
    font-family: -apple-system, system-ui, sans-serif;
  }
</style>
