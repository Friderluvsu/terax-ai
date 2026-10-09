import { envCaps, type WorkspaceEnv } from "@/modules/workspace";

const WRITE_TOOLS = new Set([
  "write_file",
  "create_directory",
  "edit",
  "multi_edit",
]);
const SHELL_TOOLS = new Set([
  "bash_run",
  "bash_background",
  "spawn_coding_agent",
  "send_to_agent",
]);

/** Error text when `toolName` must not run in `env`, or null when allowed. */
export function remoteToolBlock(
  toolName: string,
  env: WorkspaceEnv,
  readOnlyPref: boolean,
): string | null {
  const caps = envCaps(env);
  if (SHELL_TOOLS.has(toolName) && !caps.aiShell) {
    return "Commands are not available in this remote workspace. Use the read-only file tools.";
  }
  if (WRITE_TOOLS.has(toolName) && !caps.aiWrite && readOnlyPref) {
    return "Write tools are disabled for remote workspaces (AI read-only mode in Settings > Fork). Use read-only tools.";
  }
  return null;
}

/** Replaces blocked tools with stubs that return the reason, without an approval prompt. */
export function guardRemoteTools<T extends Record<string, unknown>>(
  tools: T,
  env: WorkspaceEnv,
  readOnlyPref: boolean,
): T {
  const out: Record<string, unknown> = { ...tools };
  for (const name of Object.keys(out)) {
    const reason = remoteToolBlock(name, env, readOnlyPref);
    if (!reason) continue;
    out[name] = {
      ...(out[name] as object),
      needsApproval: false,
      execute: async () => ({ error: reason }),
    };
  }
  return out as T;
}
