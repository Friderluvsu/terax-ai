import { describe, expect, it } from "vitest";
import {
  applyDetectedLanguage,
  convertUntitledToFile,
  droppedScratchIds,
  hasUnsavedEditorContent,
  isUntitledTab,
  newScratchId,
  nextUntitledNumber,
  planUntitledTabOpen,
  setUntitledEmpty,
  untitledHasContent,
} from "./untitledTabs";
import { planFileTabOpen, type EditorTab, type Tab } from "./useTabs";

const A = "a".repeat(32);
const B = "b".repeat(32);

function untitled(id: number, n: number, over: Partial<EditorTab> = {}): Tab {
  return {
    id,
    kind: "editor",
    spaceId: "s1",
    title: `Untitled-${n}`,
    path: "",
    dirty: false,
    preview: false,
    overrideLanguage: null,
    untitled: { scratchId: `${String(id).padStart(32, "0")}` },
    ...over,
  };
}

function file(id: number, path: string, over: Partial<EditorTab> = {}): Tab {
  return {
    id,
    kind: "editor",
    spaceId: "s1",
    title: path.split("/").pop() ?? path,
    path,
    dirty: false,
    preview: false,
    ...over,
  };
}

describe("nextUntitledNumber", () => {
  it("starts at 1 and takes the smallest free number", () => {
    expect(nextUntitledNumber([])).toBe(1);
    expect(nextUntitledNumber([untitled(1, 1), untitled(2, 3)])).toBe(2);
    expect(nextUntitledNumber([untitled(1, 2), untitled(2, 1)])).toBe(3);
  });

  it("ignores files that merely look untitled and spans every space", () => {
    const tabs = [file(1, "/p/Untitled-1"), untitled(2, 2, { spaceId: "s2" })];
    expect(nextUntitledNumber(tabs)).toBe(1);
    expect(nextUntitledNumber([...tabs, untitled(3, 1)])).toBe(3);
  });
});

describe("newScratchId", () => {
  it("is 32 lowercase hex chars and unique", () => {
    const a = newScratchId();
    expect(a).toMatch(/^[a-f0-9]{32}$/);
    expect(newScratchId()).not.toBe(a);
  });
});

describe("planUntitledTabOpen", () => {
  it("appends an empty, pinned Untitled tab to the space", () => {
    let next = 10;
    const plan = planUntitledTabOpen([untitled(1, 1)], "s2", () => next++, A);
    expect(plan.tabId).toBe(10);
    const tab = plan.tabs[1];
    expect(tab).toMatchObject({
      id: 10,
      kind: "editor",
      spaceId: "s2",
      title: "Untitled-2",
      path: "",
      preview: false,
      dirty: false,
      untitled: { scratchId: A, empty: true },
    });
  });

  it("is never replaced by a preview-slot open", () => {
    const opened = planUntitledTabOpen([], "s1", () => 1, A);
    const after = planFileTabOpen(opened.tabs, "/p/a.ts", false, "s1", () => 2);
    expect(after.tabs.map((t) => t.id)).toEqual([1, 2]);
  });
});

describe("untitled flags", () => {
  it("treats unknown emptiness as content", () => {
    expect(untitledHasContent(untitled(1, 1))).toBe(true);
    expect(
      untitledHasContent(
        untitled(1, 1, { untitled: { scratchId: A, empty: true } }),
      ),
    ).toBe(false);
    expect(untitledHasContent(file(2, "/p/a.ts", { dirty: true }))).toBe(false);
  });

  it("counts dirty files and non-empty Untitled buffers as unsaved", () => {
    expect(hasUnsavedEditorContent(file(1, "/p/a", { dirty: true }))).toBe(
      true,
    );
    expect(hasUnsavedEditorContent(file(1, "/p/a"))).toBe(false);
    expect(
      hasUnsavedEditorContent(
        untitled(1, 1, { untitled: { scratchId: A, empty: false } }),
      ),
    ).toBe(true);
  });

  it("setUntitledEmpty only changes state on a real transition", () => {
    const tabs = [untitled(1, 1, { untitled: { scratchId: A, empty: true } })];
    expect(setUntitledEmpty(tabs, 1, true)).toBe(tabs);
    const next = setUntitledEmpty(tabs, 1, false);
    expect(next).not.toBe(tabs);
    expect(isUntitledTab(next[0]) && next[0].untitled.empty).toBe(false);
    expect(setUntitledEmpty(tabs, 99, false)).toBe(tabs);
  });
});

describe("applyDetectedLanguage", () => {
  it("sets JSON only while no language was chosen", () => {
    const tabs = [untitled(1, 1)];
    const next = applyDetectedLanguage(tabs, 1, "json");
    expect((next[0] as EditorTab).overrideLanguage).toBe("json");
    const manual = [untitled(1, 1, { overrideLanguage: "md" })];
    expect(applyDetectedLanguage(manual, 1, "json")).toBe(manual);
  });

  it("leaves file tabs alone", () => {
    const tabs = [file(1, "/p/a.txt")];
    expect(applyDetectedLanguage(tabs, 1, "json")).toBe(tabs);
  });
});

describe("convertUntitledToFile", () => {
  it("keeps the tab id and turns it into a plain file tab", () => {
    const tabs = [
      untitled(1, 1, { overrideLanguage: "json", untitled: { scratchId: A } }),
      file(2, "/p/b.ts"),
    ];
    const next = convertUntitledToFile(tabs, 1, "C:/p/data.json");
    const tab = next[0] as EditorTab;
    expect(tab.id).toBe(1);
    expect(tab.path).toBe("C:/p/data.json");
    expect(tab.title).toBe("data.json");
    expect(tab.overrideLanguage).toBeNull();
    expect(tab.dirty).toBe(false);
    expect(tab.preview).toBe(false);
    expect("untitled" in tab).toBe(false);
    expect(next[1]).toBe(tabs[1]);
  });

  it("is a no-op for a tab that is no longer Untitled", () => {
    const tabs = [file(1, "/p/a.ts")];
    expect(convertUntitledToFile(tabs, 1, "/p/b.ts")).toBe(tabs);
  });
});

describe("droppedScratchIds", () => {
  it("reports ids that left the list, including saved-as tabs", () => {
    const a = untitled(1, 1, { untitled: { scratchId: A } });
    const b = untitled(2, 2, { untitled: { scratchId: B } });
    expect(droppedScratchIds([a, b], [a, b])).toEqual([]);
    expect(droppedScratchIds([a, b], [b])).toEqual([A]);
    expect(
      droppedScratchIds([a, b], convertUntitledToFile([a, b], 2, "/x")),
    ).toEqual([B]);
    expect(droppedScratchIds([a], [{ ...a, spaceId: "s9" } as Tab])).toEqual(
      [],
    );
  });
});
