import type { GhosttyTerminalModelApi } from "../GhosttyTerminalModel";

type Cell = { readonly row: number; readonly column: number };
type ClickOptions = {
  readonly model: Pick<GhosttyTerminalModelApi, "hyperlinkAtViewportCell">;
  readonly shouldOpenLink?: (event: MouseEvent, cell: Cell) => boolean;
  readonly onOpenLink?: (uri: string) => void;
  readonly onCellClick?: (cell: Cell, event: PointerEvent) => void;
};

export function handleTerminalCellClick(
  options: ClickOptions,
  cell: Cell,
  event: PointerEvent,
): void {
  const uri = options.model.hyperlinkAtViewportCell(cell.row, cell.column);
  if (
    uri &&
    options.onOpenLink &&
    options.shouldOpenLink?.(event, cell) !== false
  ) {
    options.onOpenLink(uri);
    return;
  }
  options.onCellClick?.(cell, event);
}
