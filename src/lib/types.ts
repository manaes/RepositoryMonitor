export type RepoState =
  | "clean" | "merging" | "rebasing" | "cherry_picking" | "reverting" | "bisecting";

export type VcsKind = "git" | "svn";

export interface RepoRef {
  path: string;
  name: string;
  category: string;
  vcs: VcsKind;
}

export interface RepoStatus {
  path: string;
  name: string;
  category: string;
  branch: string | null;
  detached_sha: string | null;
  upstream: string | null;
  has_upstream: boolean;
  ahead: number | null;
  behind: number | null;
  staged: number;
  modified: number;
  untracked: number;
  conflicts: number;
  stash: number;
  is_clean: boolean;
  state: RepoState;
  worktrees: number;
  last_fetch: number | null;
  last_checked: number;
  error: string | null;
  vcs: VcsKind;
}

export type TerminalApp =
  | "terminal"
  | "iterm"
  | "ghostty"
  | "windows_terminal"
  | "powershell"
  | "cmd"
  | { custom: string };

export interface Config {
  version: number;
  roots: string[];
  manual_paths: string[];
  exclude_globs: string[];
  poll_interval_secs: number;
  scan_depth: number;
  stale_fetch_days: number;
  terminal_app: TerminalApp;
}

export interface RepoSnapshot {
  seq: number;
  repos: RepoStatus[];
}

export type ActionKind = "open_finder" | "open_terminal" | "open_source_tree";
