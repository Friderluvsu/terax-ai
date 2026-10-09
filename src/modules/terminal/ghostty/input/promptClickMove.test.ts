import { readFile } from "node:fs/promises";
import { TeraxGhostty } from "@terax/ghostty-core/adapted";
import { beforeAll, describe, expect, it } from "vitest";
import {
  encodeArrowRun,
  promptClickDelta,
  type PromptClickLine,
} from "@/modules/terminal/ghostty/input/promptClickMove";

function row(text: string, cols: number, wrapped = false): PromptClickLine {
  const cells: string[] = [];
  for (const ch of text) {
    cells.push(ch);
    if (/[　-鿿\u{1f300}-\u{1faff}]/u.test(ch)) cells.push("");
  }
  while (cells.length < cols) cells.push(" ");
  return { cells, wrapped };
}

function lines(rows: PromptClickLine[]) {
  return (r: number) => rows[r] ?? null;
}

describe("promptClickDelta", () => {
  const cols = 10;

  it("moves within an ASCII line", () => {
    const line = lines([row("$ hello", cols)]);
    const base = { cols, line, cursor: { row: 0, col: 7 } };
    expect(promptClickDelta({ ...base, click: { row: 0, col: 3 } })).toBe(-4);
    expect(promptClickDelta({ ...base, click: { row: 0, col: 7 } })).toBe(0);
    const mid = { ...base, cursor: { row: 0, col: 3 } };
    expect(promptClickDelta({ ...mid, click: { row: 0, col: 6 } })).toBe(3);
  });

  it("spans soft-wrapped rows", () => {
    const line = lines([
      row("$ abcdefgh", cols),
      row("ijklmnopqr", cols, true),
      row("st", cols, true),
    ]);
    const base = { cols, line, cursor: { row: 2, col: 2 } };
    expect(promptClickDelta({ ...base, click: { row: 0, col: 2 } })).toBe(-20);
    expect(promptClickDelta({ ...base, click: { row: 1, col: 5 } })).toBe(-7);
    const up = { ...base, cursor: { row: 0, col: 2 } };
    expect(promptClickDelta({ ...up, click: { row: 2, col: 1 } })).toBe(19);
  });

  it("returns null outside the wrapped group", () => {
    const line = lines([
      row("old", cols),
      row("$ ab", cols),
      row("next", cols),
    ]);
    const base = { cols, line, cursor: { row: 1, col: 4 } };
    expect(promptClickDelta({ ...base, click: { row: 0, col: 1 } })).toBeNull();
    expect(promptClickDelta({ ...base, click: { row: 2, col: 1 } })).toBeNull();
  });

  it("clamps a click right of the input to the end", () => {
    const line = lines([row("$ ab", cols)]);
    const base = { cols, line, cursor: { row: 0, col: 2 } };
    expect(promptClickDelta({ ...base, click: { row: 0, col: 9 } })).toBe(2);
    const atEnd = { ...base, cursor: { row: 0, col: 4 } };
    expect(promptClickDelta({ ...atEnd, click: { row: 0, col: 9 } })).toBe(0);
  });

  it("moves left into the prompt", () => {
    const line = lines([row("$ ab", cols)]);
    const base = { cols, line, cursor: { row: 0, col: 4 } };
    expect(promptClickDelta({ ...base, click: { row: 0, col: 0 } })).toBe(-4);
  });

  it("counts characters for CJK and emoji and snaps spacer clicks", () => {
    // cells: $ ' ' a 你 spacer b emoji spacer
    const line = lines([row("$ a你b\u{1f600}", 12)]);
    const base = { cols: 12, line, cursor: { row: 0, col: 8 } };
    expect(promptClickDelta({ ...base, click: { row: 0, col: 4 } })).toBe(-3);
    expect(promptClickDelta({ ...base, click: { row: 0, col: 5 } })).toBe(-2);
    expect(promptClickDelta({ ...base, click: { row: 0, col: 2 } })).toBe(-4);
    const mid = { ...base, cursor: { row: 0, col: 3 } };
    expect(promptClickDelta({ ...mid, click: { row: 0, col: 7 } })).toBe(2);
  });

  it("handles a cursor in pending-wrap state", () => {
    const line = lines([row("$ abcdefgh", cols)]);
    const base = { cols, line, cursor: { row: 0, col: 9 }, pendingWrap: true };
    expect(promptClickDelta({ ...base, click: { row: 0, col: 5 } })).toBe(-5);
    expect(promptClickDelta({ ...base, click: { row: 0, col: 9 } })).toBe(-1);
  });
});

describe("encodeArrowRun on the real WASM core", () => {
  let ghostty: TeraxGhostty;

  beforeAll(async () => {
    const file = await readFile(
      new URL(
        "../../../../../packages/ghostty-core/adapted/ghostty-vt.wasm",
        import.meta.url,
      ),
    );
    ghostty = await TeraxGhostty.loadBytes(Uint8Array.from(file).buffer);
  });

  it("encodes ESC [ D normally and ESC O D under DECCKM", () => {
    const terminal = ghostty.createTerminal(8, 3);
    try {
      const text = (b: Uint8Array) => new TextDecoder().decode(b);
      expect(text(encodeArrowRun(terminal, -2))).toBe("\x1b[D\x1b[D");
      expect(text(encodeArrowRun(terminal, 1))).toBe("\x1b[C");
      terminal.write(new TextEncoder().encode("\x1b[?1h"));
      expect(text(encodeArrowRun(terminal, -2))).toBe("\x1bOD\x1bOD");
      expect(encodeArrowRun(terminal, 0)).toHaveLength(0);
    } finally {
      terminal.dispose();
    }
  });

  it("caps the run length", () => {
    const terminal = ghostty.createTerminal(8, 3);
    try {
      expect(encodeArrowRun(terminal, 10000, 5)).toHaveLength(15);
      expect(encodeArrowRun(terminal, 10000)).toHaveLength(3 * 4096);
    } finally {
      terminal.dispose();
    }
  });
});
