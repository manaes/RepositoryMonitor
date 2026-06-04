<script lang="ts">
  import { untrack } from "svelte";
  import type { Config } from "../lib/types";
  import { open } from "@tauri-apps/plugin-dialog";

  let { config, onSave, onClose }: {
    config: Config;
    onSave: (c: Config) => void;
    onClose: () => void;
  } = $props();

  // config의 일회성 편집 복사본(의도적으로 비반응 — untrack으로 초기값 캡처 경고 억제)
  let draft = $state<Config>(untrack(() => structuredClone($state.snapshot(config))));

  async function addRoot() {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") draft.roots = [...draft.roots, dir];
  }
  function removeRoot(i: number) {
    draft.roots = draft.roots.filter((_, j) => j !== i);
  }
  function save() {
    onSave($state.snapshot(draft));
  }
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
      oninput={(e) =>
        (draft.exclude_globs = e.currentTarget.value
          .split("\n")
          .map((s) => s.trim())
          .filter(Boolean))}
    ></textarea>
  </section>

  <section class="grid2">
    <label>폴링 주기(초)<input type="number" min="10" max="300" bind:value={draft.poll_interval_secs} /></label>
    <label>스캔 깊이<input type="number" min="1" max="10" bind:value={draft.scan_depth} /></label>
    <label>stale 기준(일)<input type="number" min="1" bind:value={draft.stale_fetch_days} /></label>
    <label>터미널 앱
      <select
        value={typeof draft.terminal_app === "string" ? draft.terminal_app : "terminal"}
        onchange={(e) => (draft.terminal_app = e.currentTarget.value as "terminal" | "iterm")}
      >
        <option value="terminal">Terminal</option>
        <option value="iterm">iTerm</option>
      </select>
    </label>
  </section>

  <div class="ft"><button class="primary" onclick={save}>저장</button></div>
</div>

<style>
  .panel {
    position: fixed;
    right: 0;
    top: 0;
    bottom: 0;
    width: 360px;
    background: var(--panel-bg);
    border-left: 1px solid var(--border);
    padding: 12px;
    overflow-y: auto;
    box-shadow: -2px 0 8px var(--shadow);
  }
  .hd {
    display: flex;
    justify-content: space-between;
  }
  section {
    margin: 14px 0;
  }
  h3 {
    font-size: 0.85em;
    color: var(--fg-muted);
  }
  .item {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    align-items: center;
    font-size: 0.8em;
  }
  .item code {
    word-break: break-all;
  }
  textarea {
    width: 100%;
  }
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .grid2 label {
    display: flex;
    flex-direction: column;
    font-size: 0.8em;
    gap: 2px;
  }
  .ft {
    margin-top: 16px;
  }
  .primary {
    background: var(--accent);
    color: var(--accent-fg);
    border: none;
    padding: 6px 16px;
    border-radius: 6px;
    cursor: pointer;
  }
</style>
