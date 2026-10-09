import { describe, expect, it } from "vitest";
import { splitBootAuthorizations } from "@/modules/spaces/lib/useSpacesBoot";

describe("splitBootAuthorizations", () => {
  it("awaits local and wsl cwds and backgrounds ssh roots", () => {
    const local = { cwd: "/Users/me/p", env: { kind: "local" as const } };
    const wsl = {
      cwd: "/home/me",
      env: { kind: "wsl" as const, distro: "Ubuntu" },
    };
    const ssh = {
      cwd: "/srv/app",
      env: { kind: "ssh" as const, host: "oracle" },
    };
    const { awaited, background } = splitBootAuthorizations([local, ssh, wsl]);
    expect(awaited).toEqual([local, wsl]);
    expect(background).toEqual([ssh]);
  });
});
