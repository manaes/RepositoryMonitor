import type { UnlistenFn } from "@tauri-apps/api/event";
import type { Config, RepoStatus } from "./types";
import { getConfig, listenReposUpdated, refreshStatus, scanRepos, setConfig } from "./tauri";

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
    await setConfig(config);
    this.config = config;
    await this.rescan(); // 루트/제외 변경 반영
  }

  /** 해당 repo 절대경로를 exclude_globs에 추가하고 재스캔(그리드에서 사라짐). */
  async excludeRepo(path: string): Promise<void> {
    if (!this.config) return;
    if (this.config.exclude_globs.includes(path)) return;
    await this.saveConfig({
      ...this.config,
      exclude_globs: [...this.config.exclude_globs, path],
    });
  }

  get hasRoots(): boolean {
    return (this.config?.roots.length ?? 0) > 0;
  }
}

export const store = new GitMonitorStore();
