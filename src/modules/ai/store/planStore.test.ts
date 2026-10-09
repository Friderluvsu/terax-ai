import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@/modules/settings/store", () => ({ setLastWslDistro: vi.fn() }));

import { useWorkspaceEnvStore } from "@/modules/workspace";
import { usePlanStore } from "./planStore";

describe("plan queue env", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({});
    usePlanStore.setState({ active: true, queue: [] });
  });

  it("applies queued edits under the env they were queued in", async () => {
    const wsl = { kind: "wsl", distro: "Ubuntu" } as const;
    usePlanStore.getState().enqueue({
      id: "q1",
      kind: "write_file",
      path: "/home/u/a.txt",
      env: wsl,
      originalContent: "",
      proposedContent: "x",
      isNewFile: true,
    });
    useWorkspaceEnvStore.setState({ env: { kind: "local" } });
    const results = await usePlanStore.getState().applyAll();
    expect(results[0].ok).toBe(true);
    expect(invoke).toHaveBeenCalled();
    for (const call of invoke.mock.calls) {
      expect(call[1].workspace).toEqual(wsl);
    }
  });
});
