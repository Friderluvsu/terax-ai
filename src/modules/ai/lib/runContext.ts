import type { WorkspaceEnv } from "@/modules/workspace";
import type { ToolContext } from "../tools/context";
import { createNative } from "./native";

export type BaseToolContext = Omit<ToolContext, "env" | "native">;

/** Pins the environment for one agent run; later space switches do not leak in. */
export function bindRunContext(
  base: BaseToolContext,
  env: WorkspaceEnv,
): ToolContext {
  return { ...base, env, native: createNative(() => env) };
}
