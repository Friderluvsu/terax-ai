import { describe, expect, it } from "vitest";
import { buildSshCommand, isValidSshAlias, sshHostDetail } from "./hosts";

describe("ssh alias", () => {
  it("accepts ordinary aliases", () => {
    for (const a of ["oracle", "imac-remote", "u@h.example", "[::1]", "a-"]) {
      expect(isValidSshAlias(a)).toBe(true);
    }
  });

  it("rejects option injection and shell metacharacters", () => {
    const bad = [
      "",
      "-oProxyCommand=x",
      "-",
      "a b",
      "a;b",
      "a$(id)",
      "a`id`",
      "a\nb",
      "a'b",
      'a"b',
      "a|b",
      "a".repeat(256),
    ];
    for (const a of bad) {
      expect(isValidSshAlias(a), JSON.stringify(a)).toBe(false);
      expect(buildSshCommand(a)).toBeNull();
    }
  });

  it("builds a quoted command", () => {
    expect(buildSshCommand("oracle")).toBe("ssh -- 'oracle'\r");
  });

  it("formats host detail", () => {
    expect(
      sshHostDetail({ alias: "a", hostname: "h", user: "u", port: 22 }),
    ).toBe("u@h:22");
    expect(
      sshHostDetail({ alias: "a", hostname: null, user: null, port: null }),
    ).toBe("a");
  });
});
