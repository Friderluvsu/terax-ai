import { describe, expect, it } from "vitest";
import {
  clampZoom,
  fitView,
  MIN_TILE,
  placeTile,
  resizeRect,
  TILE_SIZE,
  toScreen,
  zoomAt,
} from "./canvas";

describe("placeTile", () => {
  it("fills a three-column grid and skips occupied cells", () => {
    const first = placeTile([]);
    expect(first).toMatchObject({ x: 0, y: 0, ...TILE_SIZE });
    const second = placeTile([first]);
    expect(second.y).toBe(0);
    expect(second.x).toBeGreaterThan(first.w);
    const moved = { ...first, x: 2000 };
    expect(placeTile([moved])).toMatchObject({ x: 0, y: 0 });
    const row = [first, second, placeTile([first, second])];
    expect(placeTile(row).y).toBeGreaterThan(first.h);
  });
});

describe("view math", () => {
  it("scales geometry and keeps the zoom anchor fixed", () => {
    const view = { x: 10, y: 20, zoom: 1 };
    const rect = { x: 100, y: 100, w: 200, h: 100 };
    expect(toScreen(rect, view)).toEqual({ x: 110, y: 120, w: 200, h: 100 });
    const at = { x: 110, y: 120 };
    const zoomed = zoomAt(view, 1.5, at);
    expect(toScreen(rect, zoomed)).toMatchObject({ x: 110, y: 120, w: 300 });
    expect(clampZoom(9)).toBe(1.5);
    expect(clampZoom(0.01)).toBe(0.5);
  });

  it("fits every tile without enlarging past 100 percent", () => {
    const rects = [
      { x: 0, y: 0, w: 500, h: 300 },
      { x: 600, y: 0, w: 500, h: 300 },
    ];
    const view = fitView(rects, { w: 700, h: 600 });
    const right = toScreen(rects[1], view);
    expect(right.x + right.w).toBeLessThanOrEqual(700);
    expect(fitView(rects, { w: 5000, h: 5000 }).zoom).toBe(1);
  });

  it("never shrinks a tile below its minimum", () => {
    const rect = { x: 0, y: 0, w: 300, h: 200 };
    expect(resizeRect(rect, -1000, -1000, 1)).toMatchObject(MIN_TILE);
    expect(resizeRect(rect, 100, 0, 0.5).w).toBe(500);
  });
});
