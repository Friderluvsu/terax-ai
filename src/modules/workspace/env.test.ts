import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/modules/settings/store", () => ({ setLastWslDistro: vi.fn() }));

import {
  envCaps,
  LOCAL_WORKSPACE,
  parseWorkspaceScopeKey,
  sameWorkspaceEnv,
  workspaceScopeKey,
  type WorkspaceEnv,
} from "./env";

describe("workspace scope keys", () => {
  const envs: WorkspaceEnv[] = [
    LOCAL_WORKSPACE,
    { kind: "wsl", distro: "Ubuntu" },
    { kind: "ssh", host: "dev-box" },
  ];

  it("serializes each kind", () => {
    expect(envs.map(workspaceScopeKey)).toEqual([
      "local",
      "wsl:Ubuntu",
      "ssh:dev-box",
    ]);
  });

  it("round-trips through parse", () => {
    for (const env of envs) {
      expect(parseWorkspaceScopeKey(workspaceScopeKey(env))).toEqual(env);
    }
  });

  it("falls back to local for unknown keys", () => {
    expect(parseWorkspaceScopeKey("bogus")).toEqual(LOCAL_WORKSPACE);
  });

  it("compares envs by scope key", () => {
    expect(
      sameWorkspaceEnv({ kind: "ssh", host: "a" }, { kind: "ssh", host: "a" }),
    ).toBe(true);
    expect(
      sameWorkspaceEnv({ kind: "ssh", host: "a" }, { kind: "ssh", host: "b" }),
    ).toBe(false);
    expect(sameWorkspaceEnv(LOCAL_WORKSPACE, { kind: "ssh", host: "a" })).toBe(
      false,
    );
  });
});

describe("envCaps", () => {
  it("enables everything locally", () => {
    expect(Object.values(envCaps(LOCAL_WORKSPACE)).every(Boolean)).toBe(true);
  });

  it("keeps html preview off for ssh and wsl", () => {
    expect(envCaps({ kind: "ssh", host: "h" }).htmlPreview).toBe(false);
    expect(envCaps({ kind: "wsl", distro: "d" }).htmlPreview).toBe(false);
  });

  it("disables remote-unsafe features for ssh", () => {
    const caps = envCaps({ kind: "ssh", host: "h" });
    expect(caps.watch).toBe(false);
    expect(caps.lsp).toBe(false);
    expect(caps.reveal).toBe(false);
  });
});
