import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { currentWorkspaceEnv, fsFor } from "@/modules/workspace";

const FS_CHANGED_EVENT = "fs:changed";

type FsChangedPayload = { paths: string[] };

export function watchAdd(paths: string[]): void {
  if (paths.length === 0) return;
  void fsFor(currentWorkspaceEnv())
    .watchAdd(paths)
    .catch(() => {});
}

export function watchRemove(paths: string[]): void {
  if (paths.length === 0) return;
  void fsFor(currentWorkspaceEnv())
    .watchRemove(paths)
    .catch(() => {});
}

export function listenFsChanged(
  handler: (paths: string[]) => void,
): Promise<() => void> {
  return getCurrentWebviewWindow().listen<FsChangedPayload>(
    FS_CHANGED_EVENT,
    (e) => handler(e.payload.paths),
  );
}

export function parentDir(path: string): string {
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (i <= 0) return path.slice(0, i + 1) || path;
  return path.slice(0, i);
}
