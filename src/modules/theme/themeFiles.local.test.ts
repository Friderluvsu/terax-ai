import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({
  emit: vi.fn(),
  listen: vi.fn(),
}));
vi.mock("@tauri-apps/api/path", () => ({
  appConfigDir: async () => "/cfg",
  join: async (...parts: string[]) => parts.join("/"),
}));
vi.mock("@/modules/workspace", () => ({
  LOCAL_WORKSPACE: { kind: "local" },
  currentWorkspaceEnv: () => ({ kind: "ssh", host: "dev-box" }),
}));

import { deleteThemeFile, starterTheme, writeThemeFile } from "./themeFiles";

describe("theme files with an active ssh workspace", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("writes theme files against the local workspace", async () => {
    await writeThemeFile(starterTheme());
    expect(invokeMock).toHaveBeenCalled();
    for (const call of invokeMock.mock.calls) {
      expect(call[1].workspace).toEqual({ kind: "local" });
    }
  });

  it("deletes theme files against the local workspace", async () => {
    await deleteThemeFile("x");
    expect(invokeMock).toHaveBeenCalledWith(
      "fs_delete",
      expect.objectContaining({ workspace: { kind: "local" } }),
    );
  });
});
