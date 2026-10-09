import { describe, expect, it } from "vitest";
import { LongCommandTracker } from "./LongCommandTracker";

const ev = (e: object) => e as never;

describe("LongCommandTracker", () => {
  it("reports duration, exit code and command", () => {
    const t = new LongCommandTracker();
    t.handle(ev({ type: "end-of-input", command: "sleep 9" }), 1000);
    expect(
      t.handle(ev({ type: "end-of-command", exitCode: 2 }), 11000),
    ).toEqual({ command: "sleep 9", exitCode: 2, durationMs: 10000 });
  });

  it("ignores D without C", () => {
    const t = new LongCommandTracker();
    expect(t.handle(ev({ type: "end-of-command", exitCode: 0 }), 5)).toBeNull();
  });

  it("drops a started command on prompt-start", () => {
    const t = new LongCommandTracker();
    t.handle(ev({ type: "end-of-input" }), 0);
    t.handle(ev({ type: "prompt-start" }), 1);
    expect(t.handle(ev({ type: "end-of-command", exitCode: 0 }), 9)).toBeNull();
  });
});
