import type { PromptPhase } from "../core/PromptInputTracker";
import type { GhosttyTerminalModelApi } from "../GhosttyTerminalModel";
import { encodeArrowRun, promptClickDelta } from "./promptClickMove";

export type CellClickState = {
  readonly enabled: boolean;
  readonly blocksPrompt: boolean;
  readonly alternateScreen: boolean;
  readonly mouseTracking: boolean;
  readonly scrollOffset: number;
  readonly composing: boolean;
  readonly phase: PromptPhase;
  readonly altKey: boolean;
};

export type CellClickEvent = {
  readonly altKey: boolean;
  readonly metaKey: boolean;
  readonly ctrlKey: boolean;
};

export type CellClickPoint = { readonly row: number; readonly column: number };

export type CellClickDeps = {
  readonly model: Pick<
    GhosttyTerminalModelApi,
    "cols" | "cursor" | "viewportLine" | "encodeKey"
  >;
  readonly state: (altKey: boolean) => CellClickState;
  readonly write: (bytes: Uint8Array) => void;
};

export function shouldMoveCursor(state: CellClickState): boolean {
  if (!state.enabled || state.blocksPrompt) return false;
  if (state.alternateScreen || state.mouseTracking) return false;
  if (state.scrollOffset !== 0 || state.composing) return false;
  return state.altKey || state.phase === "input";
}

export function cellClickDelta(
  deps: CellClickDeps,
  cell: CellClickPoint,
  event: CellClickEvent,
): number | null {
  if (event.metaKey || event.ctrlKey) return null;
  const state = deps.state(event.altKey);
  if (!shouldMoveCursor(state)) return null;
  const { model } = deps;
  const viewportLine = model.viewportLine?.bind(model);
  if (!viewportLine) return null;
  const cursor = model.cursor();
  const under = viewportLine(cursor.y)?.cells[cursor.x] ?? " ";
  const pendingWrap =
    state.phase === "input" &&
    cursor.x === model.cols - 1 &&
    under !== " " &&
    under !== "";
  return promptClickDelta({
    cols: model.cols,
    cursor: { row: cursor.y, col: cursor.x },
    click: { row: cell.row, col: cell.column },
    line: viewportLine,
    pendingWrap,
  });
}

export function shouldOpenLinkOnClick(
  deps: CellClickDeps,
  cell: CellClickPoint,
  event: CellClickEvent,
): boolean {
  return cellClickDelta(deps, cell, event) === null;
}

export function handleCellClick(
  deps: CellClickDeps,
  cell: CellClickPoint,
  event: CellClickEvent,
): boolean {
  const delta = cellClickDelta(deps, cell, event);
  if (delta === null) return false;
  const bytes = encodeArrowRun(deps.model, delta);
  if (bytes.length > 0) deps.write(bytes);
  return true;
}
