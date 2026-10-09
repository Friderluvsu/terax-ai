import { describe, expect, it, vi } from "vitest";

vi.mock("@/lib/platform", () => ({ IS_WINDOWS: false }));

import {
  buildRunInput,
  hasShebang,
  isLaunchRunnable,
  launchAction,
  isRunnableScriptPath,
  scriptDirAndName,
} from "./runScript";

describe("script path classification", () => {
  it("recognizes runnable extensions case-insensitively", () => {
    for (const p of [
      "/a/x.command",
      "/a/x.TOOL",
      "/a/x.sh",
      "/a/x.bash",
      "/a/x.zsh",
      "/a/x.fish",
    ]) {
      expect(isRunnableScriptPath(p)).toBe(true);
    }
    expect(isRunnableScriptPath("/a/x.ts")).toBe(false);
    expect(isRunnableScriptPath("/a/.sh")).toBe(false);
    expect(isRunnableScriptPath("/a/sh")).toBe(false);
  });

  it("limits Finder launch to command and tool", () => {
    expect(isLaunchRunnable("/a/x.command")).toBe(true);
    expect(isLaunchRunnable("/a/x.tool")).toBe(true);
    expect(isLaunchRunnable("/a/x.sh")).toBe(false);
  });
});

describe("hasShebang", () => {
  it("detects only a leading shebang", () => {
    expect(hasShebang("#!/bin/sh\n")).toBe(true);
    expect(hasShebang(" #!/bin/sh")).toBe(false);
    expect(hasShebang("echo\n#!/bin/sh")).toBe(false);
  });
});

describe("buildRunInput", () => {
  it("leaves clean paths verbatim and ends with a carriage return", () => {
    expect(buildRunInput("/Users/me/run.command")).toBe(
      "/Users/me/run.command\r",
    );
  });

  it("quotes spaces, quotes, cyrillic, dollar and newlines", () => {
    expect(buildRunInput("/a b/run.command")).toBe("'/a b/run.command'\r");
    expect(buildRunInput("/a/it's.command")).toBe("'/a/it'\\''s.command'\r");
    expect(buildRunInput("/a/скрипт.command")).toBe("'/a/скрипт.command'\r");
    expect(buildRunInput("/a/$HOME.command")).toBe("'/a/$HOME.command'\r");
    expect(buildRunInput("/a/x\ny.command")).toBe("'/a/x\ny.command'\r");
  });
});

describe("scriptDirAndName", () => {
  it("splits dir and name", () => {
    expect(scriptDirAndName("/a/b/c.command")).toEqual({
      dir: "/a/b",
      name: "c.command",
    });
    expect(scriptDirAndName("/c.command")).toEqual({
      dir: "/",
      name: "c.command",
    });
  });
});

describe("launchAction", () => {
  it("runs only .command/.tool when the setting is run in a local env", () => {
    expect(launchAction("/a/x.command", "run", "local")).toBe("run");
    expect(launchAction("/a/x.tool", "run", "local")).toBe("run");
    expect(launchAction("/a/x.sh", "run", "local")).toBe("edit");
    expect(launchAction("/a/x.command", "edit", "local")).toBe("edit");
    expect(launchAction("/a/x.command", null, "local")).toBe("edit");
    expect(launchAction("/a/x.command", "run", "wsl")).toBe("edit");
  });
});
