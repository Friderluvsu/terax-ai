import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@/modules/settings/store", () => ({ setLastWslDistro: vi.fn() }));

import { useWorkspaceEnvStore } from "@/modules/workspace";
import { documentIo } from "./documentIo";

const wsl = { kind: "wsl", distro: "Ubuntu" } as const;

describe("documentIo", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    invoke.mockReset();
    invoke.mockResolvedValue(0);
    useWorkspaceEnvStore.setState({ env: wsl });
  });
  afterEach(() => vi.useRealTimers());

  it("deferred autosave writes with the env of its own tab after a space switch", async () => {
    const io = documentIo(wsl, "/home/u/a.txt");
    setTimeout(() => void io.write("text"), 500);
    useWorkspaceEnvStore.setState({ env: { kind: "local" } });
    await vi.advanceTimersByTimeAsync(500);
    expect(invoke).toHaveBeenCalledWith(
      "fs_write_file",
      expect.objectContaining({ path: "/home/u/a.txt", workspace: wsl }),
    );
  });

  it("reload and stat use the tab env too", async () => {
    const io = documentIo({ kind: "local" }, "/a.txt");
    useWorkspaceEnvStore.setState({ env: wsl });
    await io.read(false);
    await io.stat();
    for (const call of invoke.mock.calls) {
      expect(call[1].workspace).toEqual({ kind: "local" });
    }
  });
});
