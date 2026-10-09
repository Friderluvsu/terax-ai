import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  currentWorkspaceEnv,
  fsFor,
  type WorkspaceEnv,
} from "@/modules/workspace";

const FS_CHANGED_EVENT = "fs:changed";

type FsChangedPayload = { paths: string[]; env?: string };

export function watchAdd(
  paths: string[],
  env: WorkspaceEnv = currentWorkspaceEnv(),
): void {
  if (paths.length === 0) return;
  void fsFor(env)
    .watchAdd(paths)
    .catch(() => {});
}

export function watchRemove(
  paths: string[],
  env: WorkspaceEnv = currentWorkspaceEnv(),
): void {
  if (paths.length === 0) return;
  void fsFor(env)
    .watchRemove(paths)
    .catch(() => {});
}

export function listenFsChanged(
  handler: (paths: string[], env: string | undefined) => void,
): Promise<() => void> {
  return getCurrentWebviewWindow().listen<FsChangedPayload>(
    FS_CHANGED_EVENT,
    (e) => handler(e.payload.paths, e.payload.env),
  );
}

export function parentDir(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (i <= 0) return path.slice(0, i + 1) || path;
  return path.slice(0, i);
}
