import type { EditorTab, Tab } from "./useTabs";

export type UntitledState = {
  /** Opaque 32-hex id of the app-data scratch file backing the buffer. */
  scratchId: string;
  /** Whitespace-only buffer. Undefined until the buffer has been loaded. */
  empty?: boolean;
};

export type UntitledActions = {
  setEmpty: (id: number, empty: boolean) => void;
  setDetectedLanguage: (id: number, language: string) => void;
  convertToFile: (id: number, path: string) => void;
};

const UNTITLED_TITLE = /^Untitled-(\d+)$/;

export function isUntitledTab(
  tab: Tab | undefined,
): tab is EditorTab & { untitled: UntitledState } {
  return tab?.kind === "editor" && tab.untitled !== undefined;
}

/** Unknown emptiness counts as content, so nothing is discarded unasked. */
export function untitledHasContent(tab: Tab): boolean {
  return isUntitledTab(tab) && tab.untitled.empty !== true;
}

export function hasUnsavedEditorContent(tab: Tab): boolean {
  return tab.kind === "editor" && (tab.dirty || untitledHasContent(tab));
}

export function nextUntitledNumber(tabs: readonly Tab[]): number {
  const taken = new Set<number>();
  for (const tab of tabs) {
    if (!isUntitledTab(tab)) continue;
    const match = UNTITLED_TITLE.exec(tab.title);
    if (match) taken.add(Number(match[1]));
  }
  let n = 1;
  while (taken.has(n)) n += 1;
  return n;
}

export function newScratchId(): string {
  return crypto.randomUUID().replace(/-/g, "");
}

export function planUntitledTabOpen(
  tabs: Tab[],
  spaceId: string,
  allocId: () => number,
  scratchId: string,
): { tabs: Tab[]; tabId: number } {
  const tabId = allocId();
  const tab: EditorTab = {
    id: tabId,
    kind: "editor",
    spaceId,
    title: `Untitled-${nextUntitledNumber(tabs)}`,
    path: "",
    dirty: false,
    preview: false,
    overrideLanguage: null,
    untitled: { scratchId, empty: true },
  };
  return { tabs: [...tabs, tab], tabId };
}

function mapUntitled(
  tabs: Tab[],
  id: number,
  fn: (tab: EditorTab & { untitled: UntitledState }) => Tab,
): Tab[] {
  const index = tabs.findIndex((tab) => tab.id === id);
  const tab = tabs[index];
  if (!isUntitledTab(tab)) return tabs;
  const next = fn(tab);
  if (next === tab) return tabs;
  const out = [...tabs];
  out[index] = next;
  return out;
}

export function setUntitledEmpty(
  tabs: Tab[],
  id: number,
  empty: boolean,
): Tab[] {
  return mapUntitled(tabs, id, (tab) =>
    tab.untitled.empty === empty
      ? tab
      : { ...tab, untitled: { ...tab.untitled, empty } },
  );
}

/** Never overrides a language the user already picked. */
export function applyDetectedLanguage(
  tabs: Tab[],
  id: number,
  language: string,
): Tab[] {
  return mapUntitled(tabs, id, (tab) =>
    tab.overrideLanguage ? tab : { ...tab, overrideLanguage: language },
  );
}

/** Same tab id, so the mounted editor keeps its undo history and viewport. */
export function convertUntitledToFile(
  tabs: Tab[],
  id: number,
  path: string,
): Tab[] {
  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  return mapUntitled(tabs, id, (tab) => {
    const { untitled: _untitled, ...rest } = tab;
    return {
      ...rest,
      title: name,
      path,
      dirty: false,
      preview: false,
      overrideLanguage: null,
    };
  });
}

export function scratchIdsOf(tabs: readonly Tab[]): Set<string> {
  const ids = new Set<string>();
  for (const tab of tabs) {
    if (isUntitledTab(tab)) ids.add(tab.untitled.scratchId);
  }
  return ids;
}

/** Scratch ids that left the tab list: closed, discarded or saved to a file. */
export function droppedScratchIds(
  prev: readonly Tab[],
  next: readonly Tab[],
): string[] {
  const live = scratchIdsOf(next);
  return [...scratchIdsOf(prev)].filter((id) => !live.has(id));
}
