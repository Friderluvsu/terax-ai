import { loadAll } from "@/modules/spaces/lib/store";
import { droppedScratchIds, scratchIdsOf, type Tab } from "@/modules/tabs";
import { useEffect, useRef } from "react";
import { orphanScratchIds } from "./scratch";
import { discardScratch, listScratch } from "./scratchStore";

// Off the startup path: cleanup is housekeeping, never urgent.
const ORPHAN_SWEEP_DELAY_MS = 15_000;

async function sweepOrphans(liveTabs: () => Tab[]): Promise<void> {
  try {
    const [{ states }, entries] = await Promise.all([loadAll(), listScratch()]);
    if (entries.length === 0) return;
    // Persisted state counts too, so a failed restore never frees live buffers.
    const referenced = scratchIdsOf(liveTabs());
    for (const state of states.values()) {
      for (const tab of state?.tabs ?? []) {
        if (tab.kind === "editor" && "scratchId" in tab) {
          referenced.add(tab.scratchId);
        }
      }
    }
    for (const id of orphanScratchIds(entries, referenced, Date.now())) {
      void discardScratch(id);
    }
  } catch (e) {
    console.warn("[scratch] orphan sweep skipped", e);
  }
}

/**
 * Deletes the scratch file of every Untitled buffer that leaves the tab list
 * (closed, discarded or saved to a file), and sweeps old orphans after boot.
 */
export function useScratchLifecycle(tabs: Tab[], booted: boolean): void {
  const prevRef = useRef(tabs);
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;

  useEffect(() => {
    const prev = prevRef.current;
    prevRef.current = tabs;
    for (const id of droppedScratchIds(prev, tabs)) void discardScratch(id);
  }, [tabs]);

  useEffect(() => {
    if (!booted) return;
    const timer = setTimeout(
      () => void sweepOrphans(() => tabsRef.current),
      ORPHAN_SWEEP_DELAY_MS,
    );
    return () => clearTimeout(timer);
  }, [booted]);
}
