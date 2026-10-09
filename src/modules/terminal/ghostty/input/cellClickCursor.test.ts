import { describe, expect, it, vi } from "vitest";
import {
  cellClickDelta,
  handleCellClick,
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
