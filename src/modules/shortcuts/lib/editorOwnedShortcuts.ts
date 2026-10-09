import type { ShortcutId } from "@/modules/shortcuts/shortcuts";

const EDITOR_OWNED: ReadonlySet<ShortcutId> = new Set<ShortcutId>([
  "pane.splitRight",
  "pane.splitDown",
  "pane.focusNext",
  "pane.focusPrev",
  "pane.source",
]);

export function isEditorOwnedShortcut(
  id: ShortcutId,
  target: EventTarget | null,
): boolean {
  if (!EDITOR_OWNED.has(id)) return false;
  const el = target as {
    closest?: (selector: string) => Element | null;
  } | null;
  return typeof el?.closest === "function" && el.closest(".cm-editor") !== null;
}
