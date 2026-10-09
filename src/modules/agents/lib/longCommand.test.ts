import { describe, expect, it } from "vitest";
import { buildLongCommandMessage, formatCommandDuration } from "./longCommand";

const base = {
  command: "pnpm build",
  exitCode: 0,
  durationMs: 45_000,
  thresholdSeconds: 30,
  isPrivate: false,
};

describe("buildLongCommandMessage", () => {
  it("is null when off or below threshold", () => {
    expect(
      buildLongCommandMessage({ ...base, thresholdSeconds: 0 }),
    ).toBeNull();
    expect(buildLongCommandMessage({ ...base, durationMs: 29_999 })).toBeNull();
  });

  it("includes command, duration and exit code", () => {
    expect(buildLongCommandMessage(base)).toEqual({
      kind: "finished",
      title: "pnpm build finished",
      body: "Took 45s",
    });
    expect(buildLongCommandMessage({ ...base, exitCode: 2 })).toMatchObject({
      kind: "error",
      title: "pnpm build failed (exit 2)",
    });
  });

  it("hides the command in private tabs", () => {
    const m = buildLongCommandMessage({ ...base, isPrivate: true });
    expect(m?.title).toBe("Command finished");
  });

  it("formats durations", () => {
    expect(formatCommandDuration(125_000)).toBe("2m 5s");
    expect(formatCommandDuration(3_600_000)).toBe("1h");
  });
});
