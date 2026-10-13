import {
  type CloseManyHazards,
  type CloseManyKind,
  type CloseManyPending,
  deletedPathTabs,
  evaluateCloseHazards,
  hasCloseManyHazards,
  hasNewCloseManyHazards,
  unsavedTabsInSpace,
} from "@/app/hooks/tabCloseGuards";
import {
  saveScratchAs,
  untitledCloseActionFor,
} from "@/modules/editor/lib/scratchStore";
import { envForSpace, useSpaces } from "@/modules/spaces";
import { usePreferencesStore } from "@/modules/settings/preferences";
import {
  type CloseTabsPlan,
  hasUnsavedEditorContent,
  isUntitledTab,
  nextActiveInSpace,
  planCloseOtherTabs,
  planCloseTabsToRight,
  type Tab,
} from "@/modules/tabs";
import { leafHasForegroundProcess, leafIds } from "@/modules/terminal";
import { useCallback, useLayoutEffect, useRef, useState } from "react";

function confirmRunningTerminal(): boolean {
  return usePreferencesStore.getState().confirmCloseRunningTerminal;
}

type Params = {
  tabs: Tab[];
  activeId: number;
  disposeTab: (id: number) => void;
  disposeDeletedTabs: (ids: number[]) => void;
  disposeTabs: (anchorId: number, plan: CloseTabsPlan) => void;
};

/**
 * Guards tab closing: dirty editors and terminals with a live foreground
 * process route through a confirmation dialog instead of closing immediately.
 * Owns the pending-close states the dialogs render from.
 */
export function useTabCloseGuards({
  tabs,
  activeId,
  disposeTab,
  disposeDeletedTabs,
  disposeTabs,
}: Params) {
  const tabsRef = useRef(tabs);
  const activeIdRef = useRef(activeId);
  useLayoutEffect(() => {
    tabsRef.current = tabs;
    activeIdRef.current = activeId;
  }, [tabs, activeId]);
  const [pendingCloseTab, setPendingCloseTab] = useState<number | null>(null);
  const [pendingTerminalCloseTab, setPendingTerminalCloseTab] = useState<
    number | null
  >(null);
  const [pendingDeleteTabs, setPendingDeleteTabs] = useState<number[] | null>(
    null,
  );
  const [pendingCloseMany, setPendingCloseMany] =
    useState<CloseManyPending | null>(null);
  const [closeManyConfirming, setCloseManyConfirming] = useState(false);
  const closeManyRequestRef = useRef(0);
  const [pendingSpaceDelete, setPendingSpaceDelete] = useState<{
    spaceId: string;
    unsaved: number;
    run: () => void;
  } | null>(null);

  const handleClose = useCallback(
    async (id: number) => {
      // Last tab in its space can't be closed (closeTab refuses). Skip the
      // dialog entirely so confirming it doesn't appear to silently fail.
      if (nextActiveInSpace(tabs, id) === null) return;
      const t = tabs.find((x) => x.id === id);
      if (isUntitledTab(t)) {
        const action = await untitledCloseActionFor(
          t.untitled.scratchId,
          t.untitled.empty,
        );
        if (action === "discard") disposeTab(id);
        else setPendingCloseTab(id);
        return;
      }
      if (t?.kind === "editor" && t.dirty) {
        setPendingCloseTab(id);
        return;
      }
      if (t?.kind === "terminal" && confirmRunningTerminal()) {
        const leaves = leafIds(t.paneTree);
        const checks = await Promise.all(leaves.map(leafHasForegroundProcess));
        if (checks.some(Boolean)) {
          setPendingTerminalCloseTab(id);
          return;
        }
      }
      disposeTab(id);
    },
    [tabs, disposeTab],
  );

  const captureCloseMany = useCallback((closeIds: number[]) => {
    const close = new Set(closeIds);
    const affected = tabsRef.current.filter((tab) => close.has(tab.id));
    return {
      dirtyIds: affected.filter(hasUnsavedEditorContent).map((tab) => tab.id),
      leafIds: affected
        .filter((tab) => tab.kind === "terminal")
        .flatMap((tab) => leafIds(tab.paneTree)),
    };
  }, []);

  const evaluateCloseMany = useCallback(
    (closeIds: number[]): Promise<CloseManyHazards> =>
      evaluateCloseHazards(
        () => captureCloseMany(closeIds),
        leafHasForegroundProcess,
        confirmRunningTerminal(),
      ),
    [captureCloseMany],
  );

  const planCloseMany = useCallback(
    (kind: CloseManyKind, anchorId: number) =>
      kind === "right"
        ? planCloseTabsToRight(tabsRef.current, anchorId, activeIdRef.current)
        : planCloseOtherTabs(tabsRef.current, anchorId, activeIdRef.current),
    [],
  );

  const withCurrentActive = useCallback(
    (plan: CloseTabsPlan): CloseTabsPlan => ({
      closeIds: plan.closeIds,
      nextActiveId: activeIdRef.current,
    }),
    [],
  );

  const handleCloseMany = useCallback(
    async (kind: CloseManyKind, anchorId: number) => {
      const plan = planCloseMany(kind, anchorId);
      if (plan.closeIds.length === 0) return;
      const requestId = ++closeManyRequestRef.current;
      const hazards = await evaluateCloseMany(plan.closeIds);
      if (requestId !== closeManyRequestRef.current) return;
      if (hasCloseManyHazards(hazards)) {
        setPendingCloseMany({ kind, anchorId, plan, ...hazards });
        return;
      }
      disposeTabs(anchorId, withCurrentActive(plan));
    },
    [disposeTabs, evaluateCloseMany, planCloseMany, withCurrentActive],
  );

  const handleCloseTabsToRight = useCallback(
    (anchorId: number) => {
      void handleCloseMany("right", anchorId);
    },
    [handleCloseMany],
  );

  const handleCloseOtherTabs = useCallback(
    (anchorId: number) => {
      void handleCloseMany("other", anchorId);
    },
    [handleCloseMany],
  );

  const confirmCloseMany = useCallback(async () => {
    if (pendingCloseMany === null) return;
    const requestId = ++closeManyRequestRef.current;
    setCloseManyConfirming(true);
    const hazards = await evaluateCloseMany(pendingCloseMany.plan.closeIds);
    if (requestId !== closeManyRequestRef.current) return;
    if (hasNewCloseManyHazards(pendingCloseMany, hazards)) {
      setPendingCloseMany({ ...pendingCloseMany, ...hazards });
      setCloseManyConfirming(false);
      return;
    }
    disposeTabs(
      pendingCloseMany.anchorId,
      withCurrentActive(pendingCloseMany.plan),
    );
    setPendingCloseMany(null);
    setCloseManyConfirming(false);
  }, [pendingCloseMany, disposeTabs, evaluateCloseMany, withCurrentActive]);

  const cancelCloseMany = useCallback(() => {
    closeManyRequestRef.current += 1;
    setPendingCloseMany(null);
    setCloseManyConfirming(false);
  }, []);

  const confirmClose = useCallback(() => {
    if (pendingCloseTab !== null) {
      disposeTab(pendingCloseTab);
      setPendingCloseTab(null);
    }
  }, [pendingCloseTab, disposeTab]);

  const cancelClose = useCallback(() => {
    setPendingCloseTab(null);
  }, []);

  // Untitled "Save": Save As from the scratch copy, close only once written.
  const saveAndClose = useCallback(async () => {
    const id = pendingCloseTab;
    setPendingCloseTab(null);
    const tab = tabsRef.current.find((t) => t.id === id);
    if (id === null || !isUntitledTab(tab)) return;
    const { spaces } = useSpaces.getState();
    const saved = await saveScratchAs(
      {
        scratchId: tab.untitled.scratchId,
        title: tab.title,
        language: tab.overrideLanguage,
      },
      envForSpace(tab.spaceId, spaces),
      spaces.find((s) => s.id === tab.spaceId)?.root ?? null,
    );
    if (saved) disposeTab(id);
  }, [pendingCloseTab, disposeTab]);

  const requestDeleteSpace = useCallback((spaceId: string, run: () => void) => {
    const unsaved = unsavedTabsInSpace(tabsRef.current, spaceId).length;
    if (unsaved === 0) run();
    else setPendingSpaceDelete({ spaceId, unsaved, run });
  }, []);

  const confirmSpaceDelete = useCallback(() => {
    pendingSpaceDelete?.run();
    setPendingSpaceDelete(null);
  }, [pendingSpaceDelete]);

  const cancelSpaceDelete = useCallback(() => {
    setPendingSpaceDelete(null);
  }, []);

  const confirmTerminalClose = useCallback(() => {
    if (pendingTerminalCloseTab !== null) disposeTab(pendingTerminalCloseTab);
    setPendingTerminalCloseTab(null);
  }, [pendingTerminalCloseTab, disposeTab]);

  const cancelTerminalClose = useCallback(() => {
    setPendingTerminalCloseTab(null);
  }, []);

  const confirmDeleteClose = useCallback(() => {
    if (pendingDeleteTabs !== null) {
      disposeDeletedTabs(pendingDeleteTabs);
      setPendingDeleteTabs(null);
    }
  }, [pendingDeleteTabs, disposeDeletedTabs]);

  const cancelDeleteClose = useCallback(() => {
    setPendingDeleteTabs(null);
  }, []);

  const handlePathsDeleted = useCallback(
    (paths: string[]) => {
      const affected = deletedPathTabs(tabsRef.current, paths);
      disposeDeletedTabs(affected.cleanIds);
      if (affected.dirtyIds.length > 0) {
        setPendingDeleteTabs((current) => [
          ...new Set([...(current ?? []), ...affected.dirtyIds]),
        ]);
      }
    },
    [disposeDeletedTabs],
  );

  return {
    pendingCloseTab,
    pendingTerminalCloseTab,
    pendingDeleteTabs,
    pendingCloseMany,
    closeManyConfirming,
    handleClose,
    handleCloseTabsToRight,
    handleCloseOtherTabs,
    confirmClose,
    cancelClose,
    saveAndClose,
    pendingSpaceDelete,
    requestDeleteSpace,
    confirmSpaceDelete,
    cancelSpaceDelete,
    confirmTerminalClose,
    cancelTerminalClose,
    confirmDeleteClose,
    cancelDeleteClose,
    confirmCloseMany,
    cancelCloseMany,
    handlePathsDeleted,
  };
}
