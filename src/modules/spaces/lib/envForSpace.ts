import { LOCAL_WORKSPACE, type WorkspaceEnv } from "@/modules/workspace";
import type { SpaceMeta } from "./store";
import { useSpaces } from "./useSpaces";

export function envForSpace(
  spaceId: string | null | undefined,
  spaces: SpaceMeta[] = useSpaces.getState().spaces,
): WorkspaceEnv {
  if (!spaceId) return LOCAL_WORKSPACE;
  return spaces.find((s) => s.id === spaceId)?.env ?? LOCAL_WORKSPACE;
}
