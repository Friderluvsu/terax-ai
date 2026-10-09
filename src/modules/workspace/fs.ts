import { invoke } from "@tauri-apps/api/core";
import type { WorkspaceEnv } from "./env";

export type FsReadResult =
  | { kind: "text"; content: string; size: number; mtime: number }
  | { kind: "binary"; size: number }
  | { kind: "toolarge"; size: number; limit: number };

export type FsDirEntry = {
  name: string;
  kind: "file" | "dir" | "symlink";
  size: number;
  mtime: number;
  gitignored: boolean;
};

export type FsFileStat = { size: number; mtime: number; kind: string };

export type FsSearchHit = {
  path: string;
  rel: string;
  name: string;
  is_dir: boolean;
};
export type FsSearchResult = { hits: FsSearchHit[]; truncated: boolean };

export type FsListFilesResult = { files: string[]; truncated: boolean };

export type FsGrepHit = {
  path: string;
  rel: string;
  line: number;
  text: string;
};
export type FsGrepResult = {
  hits: FsGrepHit[];
  truncated: boolean;
  files_scanned: number;
};

export type FsGlobResult = {
  hits: { path: string; rel: string }[];
  truncated: boolean;
};

export type FsMoveResult =
  | { status: "conflict"; replaceable: boolean; token: string }
  | { status: "moved" };

export type FsDeleteBatchResult = { deleted: string[]; failed: number };

export type WorkspaceFs = {
  readDir(
    path: string,
    opts: { showHidden: boolean; gitDecorations?: boolean },
  ): Promise<FsDirEntry[]>;
  listSubdirs(path: string, showHidden: boolean): Promise<string[]>;
  readFile(path: string, opts?: { force?: boolean }): Promise<FsReadResult>;
  writeFile(path: string, content: string, source?: string): Promise<number>;
  stat(path: string): Promise<FsFileStat>;
  canonicalize(path: string): Promise<string>;
  createFile(path: string): Promise<void>;
  createDir(path: string): Promise<void>;
  rename(from: string, to: string): Promise<void>;
  move(args: {
    from: string;
    to: string;
    root: string;
    expectedConflict: string | null | undefined;
  }): Promise<FsMoveResult>;
  delete(path: string): Promise<void>;
  deleteBatch(paths: string[], root: string): Promise<FsDeleteBatchResult>;
  copy(sources: string[], destDir: string): Promise<void>;
  search(args: {
    root: string;
    query: string;
    limit: number;
    showHidden: boolean;
  }): Promise<FsSearchResult>;
  listFiles(root: string): Promise<FsListFilesResult>;
  grep(args: {
    pattern: string;
    root: string;
    glob: string[] | null;
    caseInsensitive: boolean | null;
    maxResults: number | null;
  }): Promise<FsGrepResult>;
  grepInteractive(args: {
    pattern: string;
    root: string;
    maxResults: number;
  }): Promise<FsGrepResult>;
  glob(args: {
    pattern: string;
    root: string;
    maxResults: number | null;
  }): Promise<FsGlobResult>;
  watchAdd(paths: string[]): Promise<void>;
  watchRemove(paths: string[]): Promise<void>;
};

export function fsFor(workspace: WorkspaceEnv): WorkspaceFs {
  return {
    readDir: (path, { showHidden, gitDecorations }) =>
      invoke("fs_read_dir", {
        path,
        showHidden,
        ...(gitDecorations === undefined ? {} : { gitDecorations }),
        workspace,
      }),
    listSubdirs: (path, showHidden) =>
      invoke("list_subdirs", { path, showHidden, workspace }),
    readFile: (path, opts) =>
      invoke("fs_read_file", {
        path,
        workspace,
        ...(opts?.force === undefined ? {} : { force: opts.force }),
      }),
    writeFile: (path, content, source) =>
      invoke("fs_write_file", {
        path,
        content,
        workspace,
        ...(source === undefined ? {} : { source }),
      }),
    stat: (path) => invoke("fs_stat", { path, workspace }),
    canonicalize: (path) => invoke("fs_canonicalize", { path, workspace }),
    createFile: (path) => invoke("fs_create_file", { path, workspace }),
    createDir: (path) => invoke("fs_create_dir", { path, workspace }),
    rename: (from, to) => invoke("fs_rename", { from, to, workspace }),
    move: ({ from, to, root, expectedConflict }) =>
      invoke("fs_move", { from, to, root, expectedConflict, workspace }),
    delete: (path) => invoke("fs_delete", { path, workspace }),
    deleteBatch: (paths, root) =>
      invoke("fs_delete_batch", { paths, root, workspace }),
    copy: (sources, destDir) =>
      invoke("fs_copy", { sources, destDir, workspace }),
    search: ({ root, query, limit, showHidden }) =>
      invoke("fs_search", { root, query, limit, showHidden, workspace }),
    listFiles: (root) => invoke("fs_list_files", { root, workspace }),
    grep: (a) => invoke("fs_grep", { ...a, workspace }),
    grepInteractive: (a) => invoke("fs_grep_interactive", { ...a, workspace }),
    glob: (a) => invoke("fs_glob", { ...a, workspace }),
    watchAdd: (paths) => invoke("fs_watch_add", { paths, workspace }),
    watchRemove: (paths) => invoke("fs_watch_remove", { paths, workspace }),
  };
}
