import { describe, expect, it } from "vitest";
import { PromptInputTracker } from "./PromptInputTracker";

function run(types: string[]) {
  const tracker = new PromptInputTracker();
  for (const type of types) tracker.handle({ type } as never);
  return tracker.phase();
}

describe("PromptInputTracker", () => {
  it("walks idle, prompt, input, running, idle", () => {
    const tracker = new PromptInputTracker();
    expect(tracker.phase()).toBe("idle");
    tracker.handle({ type: "prompt-start" });
    expect(tracker.phase()).toBe("prompt");
    tracker.handle({ type: "prompt-end" });
    expect(tracker.phase()).toBe("input");
    tracker.handle({ type: "end-of-input" });
    expect(tracker.phase()).toBe("running");
    tracker.handle({ type: "end-of-command" });
    expect(tracker.phase()).toBe("idle");
  });

  it("handles a repeated A/B without C (prompt redraw)", () => {
    expect(
      run(["prompt-start", "prompt-end", "prompt-start", "prompt-end"]),
    ).toBe("input");
    expect(run(["prompt-start", "prompt-end", "prompt-start"])).toBe("prompt");
  });

  it("resets on screen-cleared and overflow", () => {
    expect(run(["prompt-start", "prompt-end", "screen-cleared"])).toBe("idle");
    expect(run(["prompt-start", "overflow"])).toBe("idle");
  });

  it("ignores unrelated events and supports reset", () => {
    const tracker = new PromptInputTracker();
    tracker.handle({ type: "prompt-start" });
    tracker.handle({ type: "bell" });
    expect(tracker.phase()).toBe("prompt");
    tracker.reset();
    expect(tracker.phase()).toBe("idle");
  });
});
