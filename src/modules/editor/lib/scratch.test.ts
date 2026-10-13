import { describe, expect, it, vi } from "vitest";
import {
  extensionForLanguage,
  isBlankText,
  JSON_DETECT_MAX_CHARS,
  looksLikeJsonDocument,
  orphanScratchIds,
  resolveSaveAsInput,
  SCRATCH_ORPHAN_MIN_AGE_MS,
  suggestedFileName,
  untitledCloseAction,
} from "./scratch";

const A = "a".repeat(32);
const B = "b".repeat(32);

describe("isBlankText", () => {
  it("treats whitespace-only text as blank", () => {
    expect(isBlankText("")).toBe(true);
    expect(isBlankText(" \n\t\r\n ")).toBe(true);
    expect(isBlankText("  x ")).toBe(false);
  });
});

describe("looksLikeJsonDocument", () => {
  it("accepts whole objects and arrays", () => {
    expect(looksLikeJsonDocument('{"a": [1, 2]}')).toBe(true);
    expect(looksLikeJsonDocument('\n  [1, {"b": null}]\n')).toBe(true);
    expect(looksLikeJsonDocument("{}")).toBe(true);
  });

  it("rejects primitives, partial and invalid JSON", () => {
    expect(looksLikeJsonDocument("42")).toBe(false);
    expect(looksLikeJsonDocument('"text"')).toBe(false);
    expect(looksLikeJsonDocument("null")).toBe(false);
    expect(looksLikeJsonDocument('{"a": 1')).toBe(false);
    expect(looksLikeJsonDocument("{a: 1}")).toBe(false);
    expect(looksLikeJsonDocument('{"a":1} trailing')).toBe(false);
    expect(looksLikeJsonDocument("[1, 2] [3]")).toBe(false);
    expect(looksLikeJsonDocument("")).toBe(false);
  });

  it("skips the parse entirely when the shape cannot match or text is large", () => {
    const parse = vi.spyOn(JSON, "parse");
    expect(looksLikeJsonDocument("hello world")).toBe(false);
    const big = `[${"1,".repeat(JSON_DETECT_MAX_CHARS / 2)}1]`;
    expect(looksLikeJsonDocument(big)).toBe(false);
    expect(parse).not.toHaveBeenCalled();
    parse.mockRestore();
  });
});

describe("file names", () => {
  it("maps the language id to the extension, plain text to .txt", () => {
    expect(extensionForLanguage("json")).toBe("json");
    expect(extensionForLanguage("ts")).toBe("ts");
    expect(extensionForLanguage(null)).toBe("txt");
    expect(extensionForLanguage(undefined)).toBe("txt");
    expect(extensionForLanguage("../x")).toBe("txt");
    expect(suggestedFileName("Untitled-3", "md")).toBe("Untitled-3.md");
    expect(suggestedFileName("Untitled-1", null)).toBe("Untitled-1.txt");
  });
});

describe("resolveSaveAsInput", () => {
  it("joins relative paths onto the space root", () => {
    expect(resolveSaveAsInput("/home/u/p/", "src/./a.ts")).toEqual({
      path: "/home/u/p/src/a.ts",
    });
    expect(resolveSaveAsInput("/r", "dir\\b.txt")).toEqual({
      path: "/r/dir/b.txt",
    });
  });

  it("keeps absolute paths and rejects traversal or empty names", () => {
    expect(resolveSaveAsInput(null, "/etc/x.txt")).toEqual({
      path: "/etc/x.txt",
    });
    expect(resolveSaveAsInput("/r", "../x")).toHaveProperty("error");
    expect(resolveSaveAsInput("/r", "a/../../x")).toHaveProperty("error");
    expect(resolveSaveAsInput("/r", "  ")).toHaveProperty("error");
    expect(resolveSaveAsInput("/r", "dir/")).toHaveProperty("error");
    expect(resolveSaveAsInput(null, "a.txt")).toHaveProperty("error");
  });
});

describe("orphanScratchIds", () => {
  const now = 1_000_000_000_000;
  const old = now - SCRATCH_ORPHAN_MIN_AGE_MS;

  it("deletes only unreferenced buffers at least seven days old", () => {
    const entries = [
      { id: A, mtime: old },
      { id: B, mtime: old + 1 },
      { id: "c".repeat(32), mtime: old - 1 },
    ];
    expect(orphanScratchIds(entries, new Set(), now)).toEqual([
      A,
      "c".repeat(32),
    ]);
    expect(orphanScratchIds(entries, new Set([A]), now)).toEqual([
      "c".repeat(32),
    ]);
  });

  it("never yields malformed ids", () => {
    expect(
      orphanScratchIds([{ id: "../x", mtime: 0 }], new Set(), now),
    ).toEqual([]);
  });
});

describe("untitledCloseAction", () => {
  const never = () => Promise.reject(new Error("must not read"));

  it("closes an empty buffer silently and prompts for content", async () => {
    expect(await untitledCloseAction(undefined, true, never)).toBe("discard");
    expect(await untitledCloseAction(undefined, false, never)).toBe("prompt");
  });

  it("prefers the live unsaved text over a stale tab flag", async () => {
    expect(await untitledCloseAction("typed", true, never)).toBe("prompt");
    expect(await untitledCloseAction("  \n", false, never)).toBe("discard");
  });

  it("reads the stored buffer when emptiness is unknown", async () => {
    expect(
      await untitledCloseAction(undefined, undefined, async () => " "),
    ).toBe("discard");
    expect(
      await untitledCloseAction(undefined, undefined, async () => "x"),
    ).toBe("prompt");
  });
});
