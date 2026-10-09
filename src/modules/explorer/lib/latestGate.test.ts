import { describe, expect, it } from "vitest";
import { createLatestGate } from "./latestGate";

describe("createLatestGate", () => {
  it("drops an older ticket once a newer one began for the same key", () => {
    const gate = createLatestGate();
    const first = gate.begin("/a");
    const second = gate.begin("/a");
    expect(gate.isLatest("/a", first)).toBe(false);
    expect(gate.isLatest("/a", second)).toBe(true);
  });

  it("tracks keys independently", () => {
    const gate = createLatestGate();
    const a = gate.begin("/a");
    const b = gate.begin("/b");
    expect(gate.isLatest("/a", a)).toBe(true);
    expect(gate.isLatest("/b", b)).toBe(true);
  });

  it("invalidates everything on clear", () => {
    const gate = createLatestGate();
    const t = gate.begin("/a");
    gate.clear();
    expect(gate.isLatest("/a", t)).toBe(false);
  });
});
