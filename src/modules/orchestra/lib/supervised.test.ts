import { describe, expect, it } from "vitest";
import { isSupervised, markSupervised } from "./supervised";

describe("supervised runs", () => {
  it("tracks runs that have a supervisor", () => {
    expect(isSupervised("r1")).toBe(false);
    expect(isSupervised(undefined)).toBe(false);
    markSupervised("r1");
    expect(isSupervised("r1")).toBe(true);
  });
});
