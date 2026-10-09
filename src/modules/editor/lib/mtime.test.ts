import { describe, expect, it } from "vitest";
import { mtimeChanged } from "./mtime";

const ssh = { kind: "ssh", host: "h" } as const;
const local = { kind: "local" } as const;

describe("mtimeChanged", () => {
  it("ignores sub-second differences for ssh", () => {
    expect(mtimeChanged(ssh, 1_700_000_000_120, 1_700_000_000_900)).toBe(false);
  });

  it("detects a second-level change for ssh", () => {
    expect(mtimeChanged(ssh, 1_700_000_000_900, 1_700_000_001_000)).toBe(true);
  });

  it("stays exact for local files", () => {
    expect(mtimeChanged(local, 1_700_000_000_120, 1_700_000_000_900)).toBe(
      true,
    );
    expect(mtimeChanged(local, 5, 5)).toBe(false);
  });
});
