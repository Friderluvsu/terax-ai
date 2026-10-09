import { fsFor, type WorkspaceEnv } from "@/modules/workspace";

/** Disk access for one document, pinned to the env of the tab that owns it. */
export function documentIo(env: WorkspaceEnv, path: string) {
  const fs = fsFor(env);
  return {
    write: (content: string) => fs.writeFile(path, content, "editor"),
    stat: () => fs.stat(path),
    read: (force: boolean) => fs.readFile(path, { force }),
  };
}
