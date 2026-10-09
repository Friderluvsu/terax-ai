import { Key, KeyAction, Mods } from "@terax/ghostty-core";
import type { KeyEvent } from "@terax/ghostty-core/protocol";

export type PromptClickPoint = { readonly row: number; readonly col: number };

export type PromptClickLine = {
  readonly cells: readonly string[];
  readonly wrapped: boolean;
};

export type PromptClickInput = {
  readonly cols: number;
  readonly cursor: PromptClickPoint;
  readonly click: PromptClickPoint;
  readonly line: (row: number) => PromptClickLine | null | undefined;
  /** Cursor sits on the last column with the next glyph about to wrap. */
  readonly pendingWrap?: boolean;
};

export const PROMPT_CLICK_ARROW_CAP = 4096;

export function promptClickDelta(input: PromptClickInput): number | null {
  const { cols, cursor, click, line } = input;
  if (cols <= 0) return null;

  let top = cursor.row;
  while (top > 0 && line(top)?.wrapped) top -= 1;
  let bottom = cursor.row;
  while (line(bottom + 1)?.wrapped) bottom += 1;
  if (click.row < top || click.row > bottom) return null;

  const cells: string[] = [];
  for (let row = top; row <= bottom; row += 1) {
    const rowCells = line(row)?.cells ?? [];
    for (let col = 0; col < cols; col += 1) cells.push(rowCells[col] ?? " ");
  }

  const cursorPos =
    (cursor.row - top) * cols + cursor.col + (input.pendingWrap ? 1 : 0);
  let end = 0;
  for (let i = cells.length - 1; i >= 0; i -= 1) {
    if (cells[i] !== " ") {
      end = i + 1;
      break;
    }
  }

  const clickCol = Math.min(Math.max(click.col, 0), cols - 1);
  let target = Math.min(
    (click.row - top) * cols + clickCol,
    Math.max(cursorPos, end),
  );
  while (target > 0 && cells[target] === "") target -= 1;

  const from = Math.min(cursorPos, target);
  const to = Math.max(cursorPos, target);
  let count = 0;
  for (let i = from; i < to; i += 1) {
    if (cells[i] !== "") count += 1;
  }
  return target >= cursorPos ? count : -count;
}

export function encodeArrowRun(
  model: { encodeKey(event: KeyEvent): Uint8Array },
  delta: number,
  cap: number = PROMPT_CLICK_ARROW_CAP,
): Uint8Array {
  const steps = Math.min(Math.abs(Math.trunc(delta)), cap);
  if (steps === 0) return new Uint8Array(0);
  const one = model.encodeKey({
    action: KeyAction.PRESS,
    key: delta < 0 ? Key.LEFT : Key.RIGHT,
    mods: Mods.NONE,
  });
  const out = new Uint8Array(one.length * steps);
  for (let i = 0; i < steps; i += 1) out.set(one, i * one.length);
  return out;
}
