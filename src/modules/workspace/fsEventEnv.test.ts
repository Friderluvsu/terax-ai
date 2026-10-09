import { describe, expect, it } from "vitest";
import { fsEventInEnv } from "./env";

describe("fsEventInEnv", () => {
  it("matches only events tagged with the same scope key", () => {
    expect(fsEventInEnv("local", { kind: "local" })).toBe(true);
    expect(fsEventInEnv("wsl:Ubuntu", { kind: "wsl", distro: "Ubuntu" })).toBe(
      true,
    );
    expect(fsEventInEnv("wsl:Ubuntu", { kind: "local" })).toBe(false);
    expect(fsEventInEnv("ssh:box", { kind: "wsl", distro: "Ubuntu" })).toBe(
      false,
    );
  });

  it("treats untagged events as matching", () => {
    expect(fsEventInEnv(undefined, { kind: "ssh", host: "box" })).toBe(true);
  });
});
