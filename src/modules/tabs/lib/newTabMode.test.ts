import { describe, expect, it } from "vitest";
import { resolveNewTabKind } from "./newTabMode";

describe("resolveNewTabKind", () => {
  it("opens Classic on Cmd+T and Blocks on Cmd+Shift+T by default", () => {
    expect(resolveNewTabKind("classic", "default")).toBe("classic");
    expect(resolveNewTabKind("classic", "alternate")).toBe("blocks");
  });

  it("opens Blocks on Cmd+T and Classic on Cmd+Shift+T when Blocks is preferred", () => {
    expect(resolveNewTabKind("blocks", "default")).toBe("blocks");
    expect(resolveNewTabKind("blocks", "alternate")).toBe("classic");
  });

  it("ignores the preference for explicit palette and menu entries", () => {
    expect(resolveNewTabKind("blocks", "explicit", "classic")).toBe("classic");
    expect(resolveNewTabKind("classic", "explicit", "blocks")).toBe("blocks");
  });
});
