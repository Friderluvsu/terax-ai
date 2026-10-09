import { quoteShellPath } from "./quoteShellPath";

const RUNNABLE_EXTS = new Set(["command", "tool", "sh", "bash", "zsh", "fish"]);
const LAUNCH_EXTS = new Set(["command", "tool"]);

function extOf(path: string): string {
  const name = path.slice(
    Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) + 1,
  );
  const dot = name.lastIndexOf(".");
  return dot <= 0 ? "" : name.slice(dot + 1).toLowerCase();
}

export function isRunnableScriptPath(path: string): boolean {
  return RUNNABLE_EXTS.has(extOf(path));
}

export function isLaunchRunnable(path: string): boolean {
  return LAUNCH_EXTS.has(extOf(path));
}

export function hasShebang(text: string): boolean {
  return text.startsWith("#!");
}

export function buildRunInput(path: string): string {
  return `${quoteShellPath(path)}\r`;
}

export function scriptDirAndName(path: string): { dir: string; name: string } {
  const i = path.lastIndexOf("/");
  if (i === -1) return { dir: ".", name: path };
  return { dir: i === 0 ? "/" : path.slice(0, i), name: path.slice(i + 1) };
}
