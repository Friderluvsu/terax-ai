import type { WorkspaceEnv } from "@/modules/workspace";

/** Remote servers report whole seconds, so only compare to the second there. */
export function mtimeChanged(
  env: WorkspaceEnv,
  known: number,
  current: number,
): boolean {
  if (env.kind === "ssh") {
    return Math.floor(known / 1000) !== Math.floor(current / 1000);
  }
  return known !== current;
}
