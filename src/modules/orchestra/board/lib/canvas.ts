export type Rect = { x: number; y: number; w: number; h: number };
export type View = { x: number; y: number; zoom: number };
export type Size = { w: number; h: number };

export const TILE_SIZE: Size = { w: 560, h: 340 };
export const MIN_TILE: Size = { w: 280, h: 160 };
const MIN_ZOOM = 0.5;
const MAX_ZOOM = 1.5;
const GAP = 24;
const COLUMNS = 3;

export function clampZoom(zoom: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(zoom * 100) / 100));
}

function overlaps(a: Rect, b: Rect): boolean {
  return (
    a.x < b.x + b.w + GAP &&
    b.x < a.x + a.w + GAP &&
    a.y < b.y + b.h + GAP &&
    b.y < a.y + a.h + GAP
  );
}

/** First free cell of a three-column grid, scanning row by row. */
export function placeTile(taken: Rect[], size: Size = TILE_SIZE): Rect {
  for (let index = 0; ; index++) {
    const rect = {
      x: (index % COLUMNS) * (size.w + GAP),
      y: Math.floor(index / COLUMNS) * (size.h + GAP),
      ...size,
    };
    if (!taken.some((other) => overlaps(rect, other))) return rect;
  }
}

/** Zoom changes tile geometry rather than scaling pixels, so terminal text
 *  stays sharp and pointer math needs no correction. */
export function toScreen(rect: Rect, view: View): Rect {
  return {
    x: Math.round(rect.x * view.zoom + view.x),
    y: Math.round(rect.y * view.zoom + view.y),
    w: Math.round(rect.w * view.zoom),
    h: Math.round(rect.h * view.zoom),
  };
}

/** Keeps the world point under `at` (viewport pixels) fixed while zooming. */
export function zoomAt(view: View, zoom: number, at: { x: number; y: number }) {
  const next = clampZoom(zoom);
  const scale = next / view.zoom;
  return {
    x: at.x - (at.x - view.x) * scale,
    y: at.y - (at.y - view.y) * scale,
    zoom: next,
  };
}

export function fitView(rects: Rect[], viewport: Size, padding = 32): View {
  if (rects.length === 0) return { x: padding, y: padding, zoom: 1 };
  const left = Math.min(...rects.map((r) => r.x));
  const top = Math.min(...rects.map((r) => r.y));
  const right = Math.max(...rects.map((r) => r.x + r.w));
  const bottom = Math.max(...rects.map((r) => r.y + r.h));
  const zoom = clampZoom(
    Math.min(
      1,
      (viewport.w - padding * 2) / (right - left),
      (viewport.h - padding * 2) / (bottom - top),
    ),
  );
  return {
    x: Math.round((viewport.w - (right - left) * zoom) / 2 - left * zoom),
    y: Math.round(padding - top * zoom),
    zoom,
  };
}

export function moveRect(rect: Rect, dx: number, dy: number, zoom: number) {
  return { ...rect, x: rect.x + dx / zoom, y: rect.y + dy / zoom };
}

export function resizeRect(rect: Rect, dx: number, dy: number, zoom: number) {
  return {
    ...rect,
    w: Math.max(MIN_TILE.w, rect.w + dx / zoom),
    h: Math.max(MIN_TILE.h, rect.h + dy / zoom),
  };
}
