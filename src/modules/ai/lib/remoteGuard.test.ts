import { describe, expect, it } from "vitest";
import { guardRemoteTools, remoteToolBlock } from "./remoteGuard";
import { checkReadableCanonical } from "./security";

const ssh = { kind: "ssh", host: "h" } as const;
const local = { kind: "local" } as const;

describe("remoteToolBlock", () => {
  it("rejects write tools in ssh when read-only is on", () => {
    for (const t of ["write_file", "create_directory", "edit", "multi_edit"]) {
      expect(remoteToolBlock(t, ssh, true)).toMatch(/disabled/);
    }
  });

  it("allows write tools in ssh when read-only is off", () => {
    expect(remoteToolBlock("write_file", ssh, false)).toBeNull();
  });

  it("rejects commands in ssh regardless of the setting", () => {
    expect(remoteToolBlock("bash_run", ssh, false)).toMatch(/Commands/);
    expect(remoteToolBlock("bash_background", ssh, true)).toMatch(/Commands/);
  });

  it("does not restrict local or read tools", () => {
    expect(remoteToolBlock("write_file", local, true)).toBeNull();
    expect(remoteToolBlock("bash_run", local, true)).toBeNull();
    expect(remoteToolBlock("read_file", ssh, true)).toBeNull();
  });
});

describe("guardRemoteTools", () => {
  it("turns a write tool into a rejecting stub without approval", async () => {
    let ran = false;
    const tools = {
      write_file: {
        needsApproval: true,
        execute: async () => {
          ran = true;
          return { ok: true };
        },
      },
      read_file: { execute: async () => ({ ok: true }) },
    };
    const g = guardRemoteTools(tools, ssh, true);
    expect(g.write_file.needsApproval).toBe(false);
    expect(await g.write_file.execute()).toMatchObject({
      error: expect.stringMatching(/disabled/),
    });
    expect(ran).toBe(false);
    expect(g.read_file).toBe(tools.read_file);
  });
});

describe("remote deny-list", () => {
  it("blocks ~/.ssh/config and .env resolved by the remote realpath", async () => {
    const remote = async (p: string) =>
      p === "/srv/link" ? "/home/u/.ssh/config" : p;
    expect(
      await checkReadableCanonical("/home/u/.ssh/config", remote),
    ).toMatchObject({ ok: false });
    expect(await checkReadableCanonical("/srv/link", remote)).toMatchObject({
      ok: false,
    });
    expect(await checkReadableCanonical("/srv/app/.env", remote)).toMatchObject(
      { ok: false },
    );
  });
  it("replaces the description of a stubbed tool", () => {
    const tools = {
      bash_run: { description: "Run a command", execute: async () => 1 },
    };
    const out = guardRemoteTools(tools, ssh, true) as typeof tools;
    expect(out.bash_run.description).toMatch(
      /Unavailable in this SSH workspace/,
    );
  });
});
