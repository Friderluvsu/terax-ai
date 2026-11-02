import type { Tab } from "@/modules/tabs/lib/useTabs";
import { describe, expect, it } from "vitest";
import {
  describeTerminals,
  keySequence,
  MAX_SCREEN_LINES,
  MAX_SEND_CHARS,
  parseScreen,
  parseSend,
  requireTerminal,
  screenHash,
} from "./protocol";

function terminal(id: number, extra: Record<string, unknown> = {}): Tab {
  return {
    id,
    kind: "terminal",
    spaceId: "a",
    title: `t${id}`,
    paneTree: { kind: "leaf", id: id * 10 },
    activeLeafId: id * 10,
    ...extra,
  } as unknown as Tab;
}

const split = terminal(3, {
  spaceId: "a",
  paneTree: {
    kind: "split",
    id: 99,
    dir: "row",
    children: [
      { kind: "leaf", id: 31 },
      { kind: "leaf", id: 32 },
    ],
    sizes: [50, 50],
  },
  activeLeafId: 32,
});

const SPACES = [
  { id: "b", name: "Work" },
  { id: "a", name: "Home" },
];

describe("describeTerminals", () => {
  it("lists every non-private leaf grouped by space order", () => {
    const tabs = [
      terminal(1),
      terminal(2, { private: true }),
      terminal(4, { cold: true, spaceId: "b" }),
      split,
      terminal(6, { spaceId: "gone" }),
      { id: 5, kind: "editor", title: "e", path: "/x" } as unknown as Tab,
    ];
    const list = describeTerminals(
      tabs,
      3,
      { 32: { agent: "claude", status: "waiting" } },
      (a) => a.toUpperCase(),
      SPACES,
    );
    expect(list.map((t) => t.leaf)).toEqual([40, 10, 31, 32, 60]);
    expect(list[0]).toMatchObject({ space: "Work", cold: true, tab: 4 });
    expect(list[1]).toMatchObject({ space: "Home", cold: false });
    expect(list[3]).toMatchObject({
      title: "t3 2",
      active: true,
      agent: "CLAUDE",
      agentStatus: "waiting",
    });
    expect(list[2].active).toBe(false);
    expect(list[4].space).toBe("Default");
  });
});

describe("request parsing", () => {
  const allowed = describeTerminals([terminal(1)], 1, {}, (a) => a, SPACES);

  it("only accepts listed leaves", () => {
    expect(requireTerminal({ leaf: 10 }, allowed).tab).toBe(1);
    expect(() => requireTerminal({ leaf: 20 }, allowed)).toThrow();
    expect(() => requireTerminal({ leaf: "10" }, allowed)).toThrow();
    expect(() => requireTerminal(null, allowed)).toThrow();
  });

  it("clamps screen lines", () => {
    expect(parseScreen({ lines: 5000 }).lines).toBe(MAX_SCREEN_LINES);
    expect(parseScreen({ lines: 0 }).lines).toBe(1);
    expect(parseScreen({}).lines).toBe(200);
    expect(parseScreen({ since: 3 }).since).toBeNull();
  });

  it("validates send payloads", () => {
    expect(parseSend({ text: "ls", enter: true })).toEqual({
      text: "ls",
      enter: true,
    });
    expect(parseSend({ text: "y", enter: "yes" }).enter).toBe(false);
    expect(() => parseSend({ text: 1 })).toThrow();
    expect(() => parseSend({ text: "a".repeat(MAX_SEND_CHARS + 1) })).toThrow();
  });

  it("maps only allowlisted keys", () => {
    expect(keySequence({ key: "enter" })).toBe("\r");
    expect(keySequence({ key: "ctrl-c" })).toBe("\x03");
    expect(() => keySequence({ key: "toString" })).toThrow();
    expect(() => keySequence({ key: "__proto__" })).toThrow();
    expect(() => keySequence({ key: "f13" })).toThrow();
  });
});

describe("screenHash", () => {
  it("changes with text or width", () => {
    expect(screenHash("abc", 80)).toBe(screenHash("abc", 80));
    expect(screenHash("abc", 80)).not.toBe(screenHash("abd", 80));
    expect(screenHash("abc", 80)).not.toBe(screenHash("abc", 100));
  });
});
