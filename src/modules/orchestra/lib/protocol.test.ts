import { describe, expect, it } from "vitest";
import {
  parseSupervisorFiles,
  parseTaskClose,
  parseTaskOpen,
  parseTaskScreen,
  parseTaskSend,
  supervisorLaunchCommand,
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

describe("parseTaskOpen options", () => {
  it("accepts a model and settings path and rejects option-like models", () => {
    const parsed = parseTaskOpen({
      ...base,
      model: "sonnet",
      settings_path: "/data/t/claude-settings.json",
    });
    expect(parsed.model).toBe("sonnet");
    expect(parsed.settingsPath).toBe("/data/t/claude-settings.json");
    expect(() => parseTaskOpen({ ...base, model: "--x" })).toThrow();
    expect(() => parseTaskOpen({ ...base, model: "a b" })).toThrow();
  });

  it("carries the board run and rejects malformed ones", () => {
    expect(parseTaskOpen({ ...base, board: "a1b2c3d4" }).board).toBe(
      "a1b2c3d4",
    );
    expect(() => parseTaskOpen({ ...base, board: "../x" })).toThrow();
    expect(() => parseTaskOpen({ ...base, board: 3 })).toThrow();
  });
});

describe("parseTaskSend", () => {
  it("accepts one line of text for a pane", () => {
    expect(parseTaskSend({ pane_id: 3, text: "fix the test" })).toEqual({
      paneId: 3,
      text: "fix the test",
    });
    expect(() => parseTaskSend({ pane_id: 3, text: "a\nb" })).toThrow();
    expect(() => parseTaskSend({ pane_id: 3, text: " " })).toThrow();
    expect(() =>
      parseTaskSend({ pane_id: 3, text: "x".repeat(4097) }),
    ).toThrow();
    expect(() => parseTaskSend({ text: "x" })).toThrow();
  });
});

describe("supervisor launch", () => {
  it("passes the guide and settings and ends options before the prompt", () => {
    const files = parseSupervisorFiles({
      guide_path: "/a b/guide.md",
      settings_path: "/a b/settings.json",
      job_path: "/a b/job.md",
    });
    const command = supervisorLaunchCommand(
      "claude --model opus",
      files,
      false,
    );
    expect(
      command.startsWith(
        "claude --model opus --settings '/a b/settings.json' --append-system-prompt-file '/a b/guide.md' -- '",
      ),
    ).toBe(true);
    expect(command).toContain("/a b/job.md");
    expect(command).not.toContain("\n");
    expect(() => parseSupervisorFiles({ guide_path: "x" })).toThrow();
  });

  it("validates screen requests", () => {
    expect(parseTaskScreen({ pane_id: 3, lines: 60 })).toEqual({
      paneId: 3,
      lines: 60,
    });
    expect(() => parseTaskScreen({ pane_id: 3, lines: 0 })).toThrow();
    expect(() => parseTaskScreen({ pane_id: 3, lines: 500 })).toThrow();
    expect(() => parseTaskScreen({ lines: 5 })).toThrow();
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
    expect(
      command.startsWith(
        "claude --permission-mode acceptEdits --add-dir '/tmp/it'\\''s here' -- '",
      ),
    ).toBe(true);
    expect(command).toContain("/tmp/it'\\''s here/result.md");
    expect(command).toContain("/tmp/it'\\''s here/brief.md");
    expect(command).toContain("branch task/a1b2c3d4-1");
    expect(command).not.toContain("\n");
  });

  it("ends Claude's variadic --add-dir before the prompt", () => {
    const claude = workerLaunchCommand(request, "claude", false);
    expect(claude.indexOf(" -- '")).toBeGreaterThan(
      claude.indexOf("--add-dir"),
    );
    expect(
      workerLaunchCommand({ ...request, agent: "codex" }, "codex", false),
    ).not.toContain(" -- ");
  });

  it("grants directory access only to Claude", () => {
    expect(
      workerLaunchCommand({ ...request, agent: "codex" }, "codex", false),
    ).not.toContain("--add-dir");
    expect(
      workerLaunchCommand(
        { ...request, briefPath: "C:\\data\\t-1\\brief.md" },
        "claude",
        true,
      ),
    ).toContain("--add-dir 'C:\\data\\t-1'");
  });

  it("runs workers unattended with the sandbox settings and model", () => {
    const claude = workerLaunchCommand(
      { ...request, settingsPath: "/d/s.json", model: "opus" },
      "claude",
      false,
    );
    expect(claude).toMatch(
      /^claude --settings '\/d\/s\.json' --permission-mode acceptEdits --add-dir .* --model 'opus' -- '/,
    );
    expect(
      workerLaunchCommand(
        { ...request, agent: "codex", model: "o3" },
        "codex",
        false,
      ),
    ).toMatch(
      /^codex --sandbox workspace-write --ask-for-approval on-request --model 'o3' '/,
    );
    expect(
      workerLaunchCommand({ ...request, agent: "pi", model: "x" }, "pi", false),
    ).not.toContain("x'");
  });

  it("uses the interactive prompt flag where the agent needs one", () => {
    expect(
      workerLaunchCommand({ ...request, agent: "antigravity" }, "agy", false),
    ).toMatch(/^agy --mode accept-edits --add-dir '\/tmp\/it'\\''s here' -i '/);
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
