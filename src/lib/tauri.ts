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
