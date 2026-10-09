import type { TerminalDefaultMode } from "@/modules/settings/store";

export type NewTabVariant = "default" | "alternate" | "explicit";

/**
 * Cmd+T uses "default"; Cmd+Shift+T uses "alternate" and flips the preference.
 * Palette and menu entries are "explicit": they open the kind they name.
 */
export function resolveNewTabKind(
  pref: TerminalDefaultMode,
  variant: NewTabVariant,
  requested?: TerminalDefaultMode,
): TerminalDefaultMode {
  if (variant === "explicit") return requested ?? pref;
  if (pref === "blocks") {
    return variant === "default" ? "blocks" : "classic";
  }
  return variant === "default" ? "classic" : "blocks";
}
