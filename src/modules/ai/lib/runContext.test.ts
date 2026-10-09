import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@/modules/settings/store", () => ({ setLastWslDistro: vi.fn() }));

import { useWorkspaceEnvStore } from "@/modules/workspace";
import { bindRunContext, type BaseToolContext } from "./runContext";

const base = { readCache: new Map() } as unknown as BaseToolContext;

describe("AI run environment binding", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({});
    useWorkspaceEnvStore.setState({ env: { kind: "local" } });
  });

  it("keeps the run env when the active space changes mid-run", async () => {
    const wsl = { kind: "wsl", distro: "Ubuntu" } as const;
    const ctx = bindRunContext(base, wsl);
    useWorkspaceEnvStore.setState({ env: { kind: "ssh", host: "box" } });
    await ctx.native.runCommand("ls");
    await ctx.native.gitStatus("/repo");
    expect(invoke).toHaveBeenCalledTimes(2);
    for (const call of invoke.mock.calls) {
      expect(call[1].workspace).toEqual(wsl);
    }
    expect(ctx.env).toEqual(wsl);
  });

  it("two runs keep their own environments", async () => {
    const a = bindRunContext(base, { kind: "local" });
    const b = bindRunContext(base, { kind: "wsl", distro: "Debian" });
    await a.native.runCommand("a");
    await b.native.runCommand("b");
    expect(invoke.mock.calls[0][1].workspace).toEqual({ kind: "local" });
    expect(invoke.mock.calls[1][1].workspace).toEqual({
      kind: "wsl",
      distro: "Debian",
    });
  });
});
