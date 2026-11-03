import { describe, expect, it, vi } from "vitest";
import {
  clearLeafGeometry,
  leafFontScale,
  publishLeafCell,
  setLeafFontScale,
  subscribeLeafGeometry,
} from "./leafGeometry";

describe("leaf geometry", () => {
  it("defaults to no scale and forgets a leaf on clear", () => {
    expect(leafFontScale(901)).toBe(1);
    setLeafFontScale(901, 1.5);
    expect(leafFontScale(901)).toBe(1.5);
    setLeafFontScale(901, Number.NaN);
    setLeafFontScale(901, -2);
    expect(leafFontScale(901)).toBe(1.5);
    setLeafFontScale(901, 1);
    expect(leafFontScale(901)).toBe(1);
    setLeafFontScale(902, 0.7);
    clearLeafGeometry(902);
    expect(leafFontScale(902)).toBe(1);
  });

  it("ignores a repeated cell so subscribers are not woken needlessly", () => {
    const cell = { width: 8, height: 17, fontSize: 13 };
    publishLeafCell(903, cell);
    const listener = vi.fn();
    const stop = subscribeLeafGeometry(listener);
    publishLeafCell(903, { ...cell });
    expect(listener).not.toHaveBeenCalled();
    publishLeafCell(903, { ...cell, fontSize: 14 });
    expect(listener).toHaveBeenCalledTimes(1);
    stop();
    clearLeafGeometry(903);
  });
});
