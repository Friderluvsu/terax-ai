import type { WorkspaceEnv } from "@/modules/workspace";
import { create } from "zustand";

export type SaveAsRequest = {
  env: WorkspaceEnv;
  root: string | null;
  name: string;
  resolve: (path: string | null) => void;
};

export const useSaveAsPrompt = create<{ request: SaveAsRequest | null }>(
  () => ({ request: null }),
);

/** In-app Save As for workspaces without a native dialog (WSL, SSH). */
export function requestSaveAsPath(
  args: Omit<SaveAsRequest, "resolve">,
): Promise<string | null> {
  return new Promise((resolve) => {
    useSaveAsPrompt.getState().request?.resolve(null);
    useSaveAsPrompt.setState({ request: { ...args, resolve } });
  });
}

export function settleSaveAsPath(path: string | null): void {
  const request = useSaveAsPrompt.getState().request;
  if (!request) return;
  useSaveAsPrompt.setState({ request: null });
  request.resolve(path);
}
