import { describe, expect, it } from "vitest";
import {
  clampZoom,
  fitView,
  MIN_GRID,
  placeTile,
  resizeGrid,
  TILE_BORDER,
  TILE_HEADER,
  TILE_SIZE,
  tileSize,
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
});

describe("resizeGrid", () => {
  const cell = { width: 8, height: 17 };

  it("resizes by whole cells and keeps the rest of the grid", () => {
    expect(resizeGrid({ cols: 80, rows: 20 }, 80, 34, cell)).toEqual({
      cols: 90,
      rows: 22,
    });
    expect(resizeGrid({ cols: 80, rows: 20 }, 3, 5, cell)).toEqual({
      cols: 80,
      rows: 20,
    });
  });

  it("never shrinks below the minimum", () => {
    expect(resizeGrid({ cols: 40, rows: 10 }, -9999, -9999, cell)).toEqual(
      MIN_GRID,
    );
  });
});

describe("tileSize", () => {
  it("fits exactly cols x rows cells at any font size, so zoom cannot reflow", () => {
    const grid = { cols: 76, rows: 18 };
    for (const cell of [
      { width: 5, height: 11 },
      { width: 8, height: 17 },
      { width: 12, height: 26 },
    ]) {
      const size = tileSize(grid, cell);
      const bodyW = size.w - TILE_BORDER;
      const bodyH = size.h - TILE_BORDER - TILE_HEADER;
      expect(Math.floor(bodyW / cell.width)).toBe(grid.cols);
      expect(Math.floor(bodyH / cell.height)).toBe(grid.rows);
    }
  });
});
