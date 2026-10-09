import { describe, expect, it } from "vitest";
import { isEditorOwnedShortcut } from "@/modules/shortcuts/lib/editorOwnedShortcuts";

const inEditor = {
  closest: (s: string) => (s === ".cm-editor" ? {} : null),
} as unknown as EventTarget;
const outside = { closest: () => null } as unknown as EventTarget;

describe("isEditorOwnedShortcut", () => {
  it.each([
    "pane.splitRight",
    "pane.splitDown",
    "pane.focusNext",
    "pane.focusPrev",
    "pane.source",
  ] as const)("yields %s to the editor when focus is in CodeMirror", (id) => {
    expect(isEditorOwnedShortcut(id, inEditor)).toBe(true);
    expect(isEditorOwnedShortcut(id, outside)).toBe(false);
    expect(isEditorOwnedShortcut(id, null)).toBe(false);
  });

  it("does not claim unrelated shortcuts inside the editor", () => {
    expect(isEditorOwnedShortcut("editor.save", inEditor)).toBe(false);
    expect(isEditorOwnedShortcut("tab.new", inEditor)).toBe(false);
  });
});
