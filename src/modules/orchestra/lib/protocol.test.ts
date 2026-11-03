import { describe, expect, it } from "vitest";
import {
  parseTaskClose,
  parseTaskOpen,
  type TaskOpenRequest,
  workerLaunchCommand,
} from "./protocol";

const base = {
  task: "a1b2c3d4-1",
  agent: "claude",
  cwd: "/data/orchestra/a1b2c3d4/a1b2c3d4-1/worktree",
  brief_path: "/data/orchestra/a1b2c3d4/a1b2c3d4-1/brief.md",
};

describe("parseTaskOpen", () => {
  it("accepts a well-formed request", () => {
    expect(parseTaskOpen({ ...base, title: "Export" })).toEqual({
      task: "a1b2c3d4-1",
      agent: "claude",
      title: "Export",
      cwd: base.cwd,
      briefPath: base.brief_path,
    });
  });

  it.each([
    [{ ...base, task: "../x" }],
    [{ ...base, task: "A-1" }],
    [{ ...base, agent: "bash" }],
    [{ ...base, cwd: "" }],
    [{ ...base, title: "two\nlines" }],
    [{ ...base, brief_path: 7 }],
    [null],
  ])("rejects %o", (params) => {
    expect(() => parseTaskOpen(params)).toThrow();
  });
});

describe("parseTaskClose", () => {
  it("requires an integer pane id", () => {
    expect(parseTaskClose({ pane_id: 12 })).toBe(12);
    expect(() => parseTaskClose({ pane_id: "12" })).toThrow();
    expect(() => parseTaskClose({ pane_id: -1 })).toThrow();
    expect(() => parseTaskClose({})).toThrow();
  });
});

describe("workerLaunchCommand", () => {
  const request: TaskOpenRequest = {
    task: "a1b2c3d4-1",
    agent: "claude",
    cwd: base.cwd,
    briefPath: "/tmp/it's here/brief.md",
  };

  it("passes the brief path as one quoted prompt argument", () => {
    const command = workerLaunchCommand(request, "claude", false);
    expect(command.startsWith("claude '")).toBe(true);
    expect(command).toContain("/tmp/it'\\''s here/brief.md");
    expect(command).toContain("branch task/a1b2c3d4-1");
    expect(command).not.toContain("\n");
  });

  it("uses the interactive prompt flag where the agent needs one", () => {
    expect(
      workerLaunchCommand({ ...request, agent: "gemini" }, "gemini", false),
    ).toMatch(/^gemini -i '/);
    expect(
      workerLaunchCommand({ ...request, agent: "opencode" }, "opencode", false),
    ).toMatch(/^opencode --prompt '/);
  });

  it("quotes for PowerShell on Windows", () => {
    expect(workerLaunchCommand(request, "claude", true)).toContain(
      "/tmp/it''s here/brief.md",
    );
  });
});
