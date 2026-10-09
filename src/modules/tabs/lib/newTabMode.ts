import type { TerminalDefaultMode } from "@/modules/settings/store";

export type NewTabVariant = "default" | "alternate";

/** Cmd+T uses "default"; Cmd+Shift+T uses "alternate" and flips the preference. */
export function resolveNewTabKind(
  pref: TerminalDefaultMode,
  variant: NewTabVariant,
): TerminalDefaultMode {
  if (pref === "blocks") {
    return variant === "default" ? "blocks" : "classic";
  }
  return variant === "default" ? "classic" : "blocks";
}
