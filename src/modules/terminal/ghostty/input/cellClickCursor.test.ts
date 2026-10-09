import { describe, expect, it, vi } from "vitest";
import {
  cellClickDelta,
  handleCellClick,
  shouldOpenLinkOnClick,
  shouldMoveCursor,
  type CellClickDeps,
  type CellClickState,
} from "@/modules/terminal/ghostty/input/cellClickCursor";
import { encodeArrowRun } from "@/modules/terminal/ghostty/input/promptClickMove";

const base: CellClickState = {
  enabled: true,
  blocksPrompt: false,
  alternateScreen: false,
  mouseTracking: false,
  scrollOffset: 0,
  composing: false,
  phase: "input",
  altKey: false,
};

describe("shouldMoveCursor", () => {
  it("allows a plain click in the input phase", () => {
    expect(shouldMoveCursor(base)).toBe(true);
  });

  it.each([
    ["disabled", { enabled: false }],
    ["blocks prompt", { blocksPrompt: true }],
    ["alternate screen", { alternateScreen: true }],
    ["mouse tracking", { mouseTracking: true }],
    ["scrolled back", { scrollOffset: 3 }],
    ["composing", { composing: true }],
    ["idle phase", { phase: "idle" as const }],
    ["prompt phase", { phase: "prompt" as const }],
    ["running phase", { phase: "running" as const }],
  ])("blocks when %s", (_name, patch) => {
    expect(shouldMoveCursor({ ...base, ...patch })).toBe(false);
  });

  it("lets Option bypass the phase but not the other guards", () => {
    expect(shouldMoveCursor({ ...base, phase: "idle", altKey: true })).toBe(
      true,
    );
    expect(shouldMoveCursor({ ...base, phase: "running", altKey: true })).toBe(
      true,
    );
    expect(
      shouldMoveCursor({
        ...base,
        phase: "idle",
        altKey: true,
        enabled: false,
      }),
    ).toBe(false);
    expect(
      shouldMoveCursor({
        ...base,
        phase: "idle",
        altKey: true,
        alternateScreen: true,
      }),
    ).toBe(false);
  });
});

function deps(state: Partial<CellClickState> = {}, cursorX = 7) {
  const cols = 10;
  const cells = [..."$ hello"];
  while (cells.length < cols) cells.push(" ");
  const write = vi.fn();
  const model = {
    cols,
    cursor: () => ({
      x: cursorX,
      y: 0,
      visible: true,
      blinking: false,
      style: "block" as const,
    }),
    viewportLine: (row: number) =>
      row === 0 ? { cells, wrapped: false } : null,
    encodeKey: (event: { key: number }) =>
      Uint8Array.of(event.key === 0 ? 1 : 2),
  };
  const value = {
    model,
    state: (altKey: boolean) => ({ ...base, ...state, altKey }),
    write,
  } as unknown as CellClickDeps;
  return { value, write, model };
}

const plain = { altKey: false, metaKey: false, ctrlKey: false };

describe("handleCellClick", () => {
  it("writes exactly the arrow run for the delta", () => {
    const { value, write, model } = deps();
    expect(handleCellClick(value, { row: 0, column: 3 }, plain)).toBe(true);
    expect(write).toHaveBeenCalledTimes(1);
    expect(write.mock.calls[0][0]).toEqual(encodeArrowRun(model, -4));
  });

  it("writes nothing for a click on the cursor but still claims it", () => {
    const { value, write } = deps();
    expect(handleCellClick(value, { row: 0, column: 7 }, plain)).toBe(true);
    expect(write).not.toHaveBeenCalled();
  });

  it("ignores clicks outside the input line group", () => {
    const { value, write } = deps();
    expect(handleCellClick(value, { row: 3, column: 3 }, plain)).toBe(false);
    expect(write).not.toHaveBeenCalled();
  });

  it("ignores Cmd+click so links can open", () => {
    const { value, write } = deps();
    const event = { ...plain, metaKey: true };
    expect(cellClickDelta(value, { row: 0, column: 3 }, event)).toBeNull();
    expect(handleCellClick(value, { row: 0, column: 3 }, event)).toBe(false);
    expect(write).not.toHaveBeenCalled();
  });

  it("does nothing when the state forbids moving", () => {
    const { value, write } = deps({ phase: "running" });
    expect(handleCellClick(value, { row: 0, column: 3 }, plain)).toBe(false);
    expect(write).not.toHaveBeenCalled();
  });

  it("works in the idle phase with Option", () => {
    const { value, write } = deps({ phase: "idle" });
    const event = { ...plain, altKey: true };
    expect(handleCellClick(value, { row: 0, column: 3 }, event)).toBe(true);
    expect(write).toHaveBeenCalledTimes(1);
  });
});

describe("shouldOpenLinkOnClick", () => {
  it("does not open a link on a plain click inside the input group", () => {
    const { value } = deps();
    expect(shouldOpenLinkOnClick(value, { row: 0, column: 3 }, plain)).toBe(
      false,
    );
  });

  it("opens a link on Cmd+click", () => {
    const { value } = deps();
    const event = { ...plain, metaKey: true };
    expect(shouldOpenLinkOnClick(value, { row: 0, column: 3 }, event)).toBe(
      true,
    );
  });

  it("opens a link on a click outside the input group", () => {
    const { value } = deps();
    expect(shouldOpenLinkOnClick(value, { row: 3, column: 3 }, plain)).toBe(
      true,
    );
  });
});

describe("cellClickDelta wrapping", () => {
  function wrapDeps(
    rows: { text: string; wrapped: boolean }[],
    cursor: { x: number; y: number },
  ) {
    const cols = 10;
    const lines = rows.map(({ text, wrapped }) => {
      const cells = [...text];
      while (cells.length < cols) cells.push(" ");
      return { cells, wrapped };
    });
    return {
      model: {
        cols,
        cursor: () => ({
          ...cursor,
          visible: true,
          blinking: false,
          style: "block" as const,
        }),
        viewportLine: (row: number) => lines[row] ?? null,
        encodeKey: () => Uint8Array.of(1),
      },
      state: (altKey: boolean) => ({ ...base, altKey }),
      write: vi.fn(),
    } as unknown as CellClickDeps;
  }

  it("accounts for pending wrap on a full last column", () => {
    const value = wrapDeps([{ text: "$ abcdefgh", wrapped: false }], {
      x: 9,
      y: 0,
    });
    expect(cellClickDelta(value, { row: 0, column: 5 }, plain)).toBe(-5);
    expect(cellClickDelta(value, { row: 0, column: 9 }, plain)).toBe(-1);
  });

  it("does not add pending wrap on an empty last cell", () => {
    const value = wrapDeps([{ text: "$ abc", wrapped: false }], {
      x: 9,
      y: 0,
    });
    expect(cellClickDelta(value, { row: 0, column: 5 }, plain)).toBe(-4);
  });

  it("spans a group of two wrapped rows", () => {
    const value = wrapDeps(
      [
        { text: "$ abcdefgh", wrapped: false },
        { text: "ij", wrapped: true },
      ],
      { x: 2, y: 1 },
    );
    expect(cellClickDelta(value, { row: 0, column: 3 }, plain)).toBe(-9);
    expect(cellClickDelta(value, { row: 1, column: 0 }, plain)).toBe(-2);
    expect(cellClickDelta(value, { row: 4, column: 0 }, plain)).toBeNull();
  });
});
