import { invoke } from "@tauri-apps/api/core";
import { type RefObject, useCallback } from "react";
import {
  type AgentInstanceCount,
  findAgentLauncher,
  validateAgentLaunchCommand,
} from "@/modules/agents";
import { usePreferencesStore } from "@/modules/settings/preferences";
import { useSpaces } from "@/modules/spaces";
import type { Tab } from "@/modules/tabs/lib/useTabs";
import { hasLeaf, whenSessionReady, writeToSession } from "@/modules/terminal";
import {
  ORCHESTRA_SPACE_ID,
  ORCHESTRA_SPACE_NAME,
  type TaskOpenRequest,
  TaskRequestError,
  workerLaunchCommand,
} from "./lib/protocol";

type Options = {
  tabsRef: RefObject<Tab[]>;
  newAgentGroupTab: (
    cwd: string | undefined,
    title: string,
    instances: AgentInstanceCount,
    options?: { spaceId?: string; activate?: boolean },
  ) => { tabId: number; leafIds: number[] };
  newTabInSpace: (spaceId: string, cwd?: string) => number;
  closeTab: (id: number) => void;
};

export type TaskOpened = { tab_id: number; pane_id: number; space_id: string };

function ensureOrchestraSpace(): string {
  const spaces = useSpaces.getState();
  if (!spaces.spaces.some((space) => space.id === ORCHESTRA_SPACE_ID)) {
    spaces.create({
      id: ORCHESTRA_SPACE_ID,
      name: ORCHESTRA_SPACE_NAME,
      root: null,
      env: { kind: "local" },
    });
  }
  return ORCHESTRA_SPACE_ID;
}

export function useOrchestraTabs({
  tabsRef,
  newAgentGroupTab,
  newTabInSpace,
  closeTab,
}: Options) {
  const openTask = useCallback(
    (request: TaskOpenRequest): TaskOpened => {
      const launcher = findAgentLauncher(request.agent);
      const stored =
        usePreferencesStore.getState().agentLaunchCommands[request.agent];
      const command = validateAgentLaunchCommand(stored);
      if (!command.ok) {
        throw new TaskRequestError("invalid_command", command.error);
      }
      const spaceId = ensureOrchestraSpace();
      const title = request.title ?? `${launcher.label} ${request.task}`;
      const { tabId, leafIds } = newAgentGroupTab(request.cwd, title, 1, {
        spaceId,
        activate: false,
      });
      const leafId = leafIds[0];
      const hooksReady = launcher.supportsHooks
        ? invoke("agent_enable_hooks", { agent: request.agent }).catch(
            (error) => {
              console.warn(
                `[terax] could not enable ${request.agent} notifications:`,
                error,
              );
            },
          )
        : Promise.resolve();
      void (async () => {
        await Promise.all([whenSessionReady(leafId), hooksReady]);
        const line = workerLaunchCommand(request, command.command);
        if (!writeToSession(leafId, `${line}\r`)) {
          console.error(
            `[terax] worker terminal ${leafId} closed before launch`,
          );
        }
      })();
      return { tab_id: tabId, pane_id: leafId, space_id: spaceId };
    },
    [newAgentGroupTab],
  );

  // A space always keeps one tab, so a cold placeholder replaces the last
  // worker tab instead of leaving a dead shell behind.
  const closeTask = useCallback(
    (paneId: number): boolean => {
      const tabs = tabsRef.current ?? [];
      const tab = tabs.find(
        (entry) => entry.kind === "terminal" && hasLeaf(entry.paneTree, paneId),
      );
      if (!tab) return false;
      if (!tabs.some((t) => t.spaceId === tab.spaceId && t.id !== tab.id)) {
        newTabInSpace(tab.spaceId);
      }
      closeTab(tab.id);
      return true;
    },
    [tabsRef, newTabInSpace, closeTab],
  );

  return { openTask, closeTask };
}
