import { invoke } from "@tauri-apps/api/core";
import { type RefObject, useCallback } from "react";
import { toast } from "sonner";
import {
  type AgentInstanceCount,
  findAgentLauncher,
  validateAgentLaunchCommand,
} from "@/modules/agents";
import { usePreferencesStore } from "@/modules/settings/preferences";
import { useSpaces } from "@/modules/spaces";
import {
  MAX_BOARD_LEAVES,
  type Tab,
  type TerminalTab,
} from "@/modules/tabs/lib/useTabs";
import {
  hasLeaf,
  leafIds,
  leafScreen,
  whenSessionReady,
  writeToSession,
} from "@/modules/terminal";
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
  newBoardTab: (board: { run: string; repo: string }) => number;
  addBoardLeaf: (tabId: number, cwd: string) => number;
  closePaneByLeaf: (leafId: number) => void;
  setActiveId: (id: number) => void;
  activeSpaceIdRef: RefObject<string | null>;
};

const SUBMIT_DELAY_MS = 150;

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

function boardTab(tabs: Tab[], run: string): TerminalTab | undefined {
  return tabs.find(
    (tab): tab is TerminalTab =>
      tab.kind === "terminal" && tab.board?.run === run,
  );
}

export function useOrchestraTabs({
  tabsRef,
  newAgentGroupTab,
  newTabInSpace,
  closeTab,
  newBoardTab,
  addBoardLeaf,
  closePaneByLeaf,
  setActiveId,
  activeSpaceIdRef,
}: Options) {
  // A closed or full board falls back to the Orchestra space.
  const placeOnBoard = useCallback(
    (request: TaskOpenRequest) => {
      if (!request.board) return null;
      const tab = boardTab(tabsRef.current ?? [], request.board);
      if (!tab || tab.cold) return null;
      if (leafIds(tab.paneTree).length >= MAX_BOARD_LEAVES) return null;
      const leafId = addBoardLeaf(tab.id, request.cwd);
      return { tabId: tab.id, leafId, spaceId: tab.spaceId };
    },
    [tabsRef, addBoardLeaf],
  );

  const openInOrchestraSpace = useCallback(
    (request: TaskOpenRequest, label: string) => {
      const spaceId = ensureOrchestraSpace();
      const title = request.title ?? `${label} ${request.task}`;
      const { tabId, leafIds } = newAgentGroupTab(request.cwd, title, 1, {
        spaceId,
        activate: false,
      });
      return { tabId, leafId: leafIds[0], spaceId };
    },
    [newAgentGroupTab],
  );

  const openTask = useCallback(
    (request: TaskOpenRequest): TaskOpened => {
      const launcher = findAgentLauncher(request.agent);
      const stored =
        usePreferencesStore.getState().agentLaunchCommands[request.agent];
      const command = validateAgentLaunchCommand(stored);
      if (!command.ok) {
        throw new TaskRequestError("invalid_command", command.error);
      }
      const placed = placeOnBoard(request);
      const { tabId, leafId, spaceId } =
        placed ?? openInOrchestraSpace(request, launcher.label);
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
    [placeOnBoard, openInOrchestraSpace],
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
      if (tab.kind === "terminal" && tab.board) {
        closePaneByLeaf(paneId);
        return true;
      }
      if (!tabs.some((t) => t.spaceId === tab.spaceId && t.id !== tab.id)) {
        newTabInSpace(tab.spaceId);
      }
      closeTab(tab.id);
      return true;
    },
    [tabsRef, newTabInSpace, closeTab, closePaneByLeaf],
  );

  // TUIs treat a single burst ending in CR as a paste, so Enter goes separately.
  const sendTask = useCallback((paneId: number, text: string): boolean => {
    if (!writeToSession(paneId, text)) return false;
    setTimeout(() => writeToSession(paneId, "\r"), SUBMIT_DELAY_MS);
    return true;
  }, []);

  const screenTask = useCallback((paneId: number, lines: number) => {
    return leafScreen(paneId, lines)?.text ?? null;
  }, []);

  const openBoard = useCallback(
    async (cwd: string | undefined) => {
      if (!cwd) {
        toast.error("Open a folder inside a git repository first.");
        return;
      }
      let board: { run: string; repo: string };
      try {
        board = await invoke("orchestra_board_open", { cwd });
      } catch (error) {
        toast.error(`Could not open the task board: ${String(error)}`);
        return;
      }
      const existing = boardTab(tabsRef.current ?? [], board.run);
      if (existing && existing.spaceId === activeSpaceIdRef.current) {
        setActiveId(existing.id);
        return;
      }
      newBoardTab({ run: board.run, repo: board.repo });
    },
    [tabsRef, activeSpaceIdRef, setActiveId, newBoardTab],
  );

  return { openTask, closeTask, sendTask, screenTask, openBoard };
}
