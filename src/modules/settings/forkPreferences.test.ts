import { describe, expect, it } from "vitest";
import {
  clampNotifySeconds,
  coerceTerminalDefaultMode,
  coerceTerminalOptionAsMeta,
  NOTIFY_LONG_COMMANDS_MAX_SECONDS,
} from "./store";

describe("fork preference coercion", () => {
  it("keeps valid terminal modes and falls back to classic", () => {
    expect(coerceTerminalDefaultMode("blocks")).toBe("blocks");
    expect(coerceTerminalDefaultMode("classic")).toBe("classic");
    expect(coerceTerminalDefaultMode("tabs")).toBe("classic");
    expect(coerceTerminalDefaultMode(null)).toBe("classic");
  });

  it("keeps valid option-as-meta values and falls back to off", () => {
    for (const v of ["off", "left", "both"]) {
      expect(coerceTerminalOptionAsMeta(v)).toBe(v);
    }
    expect(coerceTerminalOptionAsMeta("right")).toBe("off");
    expect(coerceTerminalOptionAsMeta(true)).toBe("off");
  });

  it("clamps long-command notification seconds to 0..max", () => {
    expect(clampNotifySeconds(0)).toBe(0);
    expect(clampNotifySeconds(30.4)).toBe(30);
    expect(clampNotifySeconds(-5)).toBe(0);
    expect(clampNotifySeconds(Number.NaN)).toBe(0);
    expect(clampNotifySeconds("10")).toBe(0);
    expect(clampNotifySeconds(Number.POSITIVE_INFINITY)).toBe(0);
    expect(clampNotifySeconds(1e12)).toBe(NOTIFY_LONG_COMMANDS_MAX_SECONDS);
  });
});
