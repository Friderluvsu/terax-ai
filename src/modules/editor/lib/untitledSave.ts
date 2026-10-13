import type { WorkspaceEnv } from "@/modules/workspace";
import { fsFor } from "@/modules/workspace/fs";
import { toast } from "sonner";
import { requestSaveAsPath } from "./saveAsPrompt";

type SaveAsArgs = {
  env: WorkspaceEnv;
  root: string | null;
  name: string;
  text: string;
};

// The native panel asks before replacing an existing file on every platform.
async function pickPath(
  env: WorkspaceEnv,
  root: string | null,
  name: string,
): Promise<string | null> {
  if (env.kind !== "local") return requestSaveAsPath({ env, root, name });
  const { save } = await import("@tauri-apps/plugin-dialog");
  const base = root?.replace(/[\\/]+$/, "");
  const drive = base !== undefined && /^[A-Za-z]:/.test(base);
  const dir = drive ? base.replace(/\//g, "\\") : base;
  const sep = drive ? "\\" : "/";
  const picked = await save({
    defaultPath: dir ? `${dir}${sep}${name}` : name,
  });
  return picked ? picked.replace(/\\/g, "/") : null;
}

/** Null when the user cancelled or the write failed (already reported). */
export async function saveUntitledAs({
  env,
  root,
  name,
  text,
}: SaveAsArgs): Promise<{ path: string; mtime: number } | null> {
  let path: string | null;
  try {
    path = await pickPath(env, root, name);
  } catch (e) {
    toast.error("Save As failed", { description: String(e) });
    return null;
  }
  if (!path) return null;
  try {
    const mtime = await fsFor(env).writeFile(path, text, "editor");
    return { path, mtime };
  } catch (e) {
    toast.error("Could not save file", { description: String(e) });
    return null;
  }
}
