import type { Tab } from "@/modules/tabs/lib/useTabs";
import { leafIds } from "@/modules/terminal/lib/panes";

export const MAX_SCREEN_LINES = 1000;
export const MAX_SEND_CHARS = 16_000;

export type MobileAgent = { agent: string; status: "working" | "waiting" };

export type MobileSpace = { id: string; name: string };

export type MobileTerminal = {
  leaf: number;
  tab: number;
  spaceId: string;
  space: string;
  cold: boolean;
  title: string;
  active: boolean;
  agent: string | null;
  agentStatus: MobileAgent["status"] | null;
};

export class MobileRequestError extends Error {}

const KEYS: Record<string, string> = {
  enter: "\r",
  esc: "\x1b",
  tab: "\t",
  "shift-tab": "\x1b[Z",
  up: "\x1b[A",
  down: "\x1b[B",
  right: "\x1b[C",
  left: "\x1b[D",
  backspace: "\x7f",
  "ctrl-c": "\x03",
  "ctrl-d": "\x04",
};

/** Private tabs never leave the machine, matching the AI context rule. */
export function describeTerminals(
  tabs: Tab[],
  activeTabId: number,
  agents: Record<number, MobileAgent>,
  labelAgent: (agent: string) => string,
  spaces: readonly MobileSpace[],
): MobileTerminal[] {
  const order = new Map(spaces.map((space, index) => [space.id, index]));
  const names = new Map(spaces.map((space) => [space.id, space.name]));
  return tabs
    .filter((tab) => tab.kind === "terminal" && !tab.private)
    .map((tab, index) => ({ tab, index }))
    .sort(
      (a, b) =>
        (order.get(a.tab.spaceId) ?? spaces.length) -
          (order.get(b.tab.spaceId) ?? spaces.length) || a.index - b.index,
    )
    .flatMap(({ tab }) => {
      if (tab.kind !== "terminal") return [];
      const leaves = leafIds(tab.paneTree);
      return leaves.map((leaf, index) => {
        const agent = agents[leaf];
        return {
          leaf,
          tab: tab.id,
          spaceId: tab.spaceId,
          space: names.get(tab.spaceId) ?? "Default",
          cold: tab.cold === true,
          title: leaves.length > 1 ? `${tab.title} ${index + 1}` : tab.title,
          active: tab.id === activeTabId && tab.activeLeafId === leaf,
          agent: agent ? labelAgent(agent.agent) : null,
          agentStatus: agent?.status ?? null,
        };
      });
    });
}

function record(params: unknown): Record<string, unknown> {
  if (typeof params !== "object" || params === null || Array.isArray(params)) {
    throw new MobileRequestError("parameters must be an object");
  }
  return params as Record<string, unknown>;
}

export function requireTerminal(
  params: unknown,
  allowed: readonly MobileTerminal[],
): MobileTerminal {
  const leaf = record(params).leaf;
  const terminal = Number.isSafeInteger(leaf)
    ? allowed.find((t) => t.leaf === leaf)
    : undefined;
  if (!terminal) throw new MobileRequestError("terminal is not available");
  return terminal;
}

export function parseScreen(params: unknown): {
  lines: number;
  since: string | null;
} {
  const value = record(params);
  const lines = Number.isSafeInteger(value.lines)
    ? Math.min(Math.max(value.lines as number, 1), MAX_SCREEN_LINES)
    : 200;
  const since = typeof value.since === "string" ? value.since : null;
  return { lines, since };
}

export function parseSend(params: unknown): { text: string; enter: boolean } {
  const value = record(params);
  if (typeof value.text !== "string") {
    throw new MobileRequestError("text is required");
  }
  if (value.text.length > MAX_SEND_CHARS) {
    throw new MobileRequestError("text is too long");
  }
  return { text: value.text, enter: value.enter === true };
}

export function keySequence(params: unknown): string {
  const key = record(params).key;
  const sequence =
    typeof key === "string" && Object.prototype.hasOwnProperty.call(KEYS, key)
      ? KEYS[key]
      : undefined;
  if (sequence === undefined) {
    throw new MobileRequestError("unsupported key");
  }
  return sequence;
}

/** FNV-1a over UTF-16 code units; only detects change, not tampering. */
export function screenHash(text: string, cols: number): string {
  let hash = 0x811c9dc5 ^ cols;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return `${(hash >>> 0).toString(36)}-${text.length}`;
}
