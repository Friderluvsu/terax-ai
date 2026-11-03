import { describe, expect, it } from "vitest";
import {
  type BoardTask,
  boardTasks,
  statusLabel,
  statusTone,
  summarize,
} from "./board";

const task = (patch: Partial<BoardTask>): BoardTask => ({
  id: "ab-1",
  run: "ab",
  agent: "claude",
  status: "running",
  branch: "task/ab-1",
  ...patch,
});

describe("orchestra board", () => {
  it("hides settled tasks whose tab is gone", () => {
    const tasks = [
      task({ id: "ab-1", status: "done" }),
      task({ id: "ab-2", status: "stopped" }),
      task({ id: "ab-3", status: "done", pane_id: 4 }),
      task({ id: "ab-4", status: "queued" }),
    ];
    expect(boardTasks(tasks).map((t) => t.id)).toEqual(["ab-3", "ab-4"]);
    expect(summarize([task({ status: "interrupted" })]).visible).toBe(false);
  });

  it("counts work, queue, input and failures", () => {
    const summary = summarize([
      task({ status: "running", pane_id: 1 }),
      task({ status: "done", checks: "running", pane_id: 2 }),
      task({ status: "needs_input", pane_id: 3 }),
      task({ status: "queued" }),
      task({ status: "done", checks: "failed", pane_id: 5 }),
    ]);
    expect(summary).toEqual({
      visible: true,
      working: 2,
      queued: 1,
      needsInput: 1,
      failed: 1,
      total: 5,
    });
  });

  it("labels checks before the raw status", () => {
    expect(statusLabel(task({ status: "done", checks: "running" }))).toBe(
      "checking",
    );
    expect(statusLabel(task({ status: "needs_input" }))).toBe("needs input");
    expect(statusTone(task({ status: "needs_input" }))).toBe("attention");
    expect(statusTone(task({ status: "done", checks: "failed" }))).toBe("bad");
    expect(statusTone(task({ status: "done", checks: "passed" }))).toBe("ok");
  });
});
