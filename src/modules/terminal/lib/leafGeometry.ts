import { useSyncExternalStore } from "react";

export type LeafCell = { width: number; height: number; fontSize: number };

const scales = new Map<number, number>();
const cells = new Map<number, LeafCell>();
const listeners = new Set<() => void>();

function emit(): void {
  for (const listener of listeners) listener();
}

export function subscribeLeafGeometry(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Scales one terminal's font on top of the user's size and the app zoom. */
export function setLeafFontScale(leafId: number, scale: number): void {
  if (!Number.isFinite(scale) || scale <= 0) return;
  const current = scales.get(leafId) ?? 1;
  if (current === scale) return;
  if (scale === 1) scales.delete(leafId);
  else scales.set(leafId, scale);
  emit();
}

export function leafFontScale(leafId: number): number {
  return scales.get(leafId) ?? 1;
}

/** Called by the session once a font's metrics are applied to its surface. */
export function publishLeafCell(leafId: number, cell: LeafCell): void {
  const previous = cells.get(leafId);
  if (
    previous?.width === cell.width &&
    previous.height === cell.height &&
    previous.fontSize === cell.fontSize
  ) {
    return;
  }
  cells.set(leafId, cell);
  emit();
}

export function clearLeafGeometry(leafId: number): void {
  const had = scales.delete(leafId) || cells.delete(leafId);
  if (had) emit();
}

export function useLeafFontScale(leafId: number): number {
  return useSyncExternalStore(subscribeLeafGeometry, () =>
    leafFontScale(leafId),
  );
}

export function useLeafCell(leafId: number): LeafCell | null {
  return useSyncExternalStore(
    subscribeLeafGeometry,
    () => cells.get(leafId) ?? null,
  );
}
