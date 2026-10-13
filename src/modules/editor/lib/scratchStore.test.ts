import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const calls: Array<[string, Record<string, unknown> | undefined]> = [];
const disk = new Map<string, string>();
let gate: Promise<void> = Promise.resolve();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (cmd: string, args?: Record<string, unknown>) => {
    calls.push([cmd, args]);
    await gate;
    const id = args?.id as string;
    if (cmd === "scratch_write") disk.set(id, args?.content as string);
    if (cmd === "scratch_delete") disk.delete(id);
    if (cmd === "scratch_read") return disk.get(id) ?? null;
    return undefined;
  },
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn() } }));

const {
  discardScratch,
  flushAllScratch,
  flushScratch,
  readScratch,
  scheduleScratchWrite,
  untitledCloseActionFor,
} = await import("./scratchStore");

const A = "a".repeat(32);

beforeEach(() => {
  vi.useFakeTimers();
  calls.length = 0;
  disk.clear();
  gate = Promise.resolve();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("scratchStore", () => {
  it("coalesces keystrokes into one debounced write of the latest text", async () => {
    scheduleScratchWrite(A, "a");
    scheduleScratchWrite(A, "ab");
    scheduleScratchWrite(A, "abc");
    expect(calls).toEqual([]);
    await vi.advanceTimersByTimeAsync(500);
    expect(calls).toEqual([["scratch_write", { id: A, content: "abc" }]]);
    expect(disk.get(A)).toBe("abc");
  });

  it("flushes pending writes immediately for quit", async () => {
    scheduleScratchWrite(A, "draft");
    await flushAllScratch();
    expect(disk.get(A)).toBe("draft");
    await vi.advanceTimersByTimeAsync(1000);
    expect(calls).toHaveLength(1);
  });

  it("drops a pending write on discard and deletes after in-flight writes", async () => {
    let release = () => {};
    gate = new Promise((resolve) => {
      release = resolve;
    });
    scheduleScratchWrite(A, "first");
    const inFlight = flushScratch(A);
    scheduleScratchWrite(A, "never written");
    const discarded = discardScratch(A);
    release();
    await Promise.all([inFlight, discarded]);
    await vi.advanceTimersByTimeAsync(1000);
    expect(calls.map(([cmd]) => cmd)).toEqual([
      "scratch_write",
      "scratch_delete",
    ]);
    expect(disk.has(A)).toBe(false);
  });

  it("reads the pending text before touching disk", async () => {
    disk.set(A, "old");
    scheduleScratchWrite(A, "new");
    expect(await readScratch(A)).toBe("new");
    expect(calls).toEqual([]);
    await flushScratch(A);
    expect(await readScratch(A)).toBe("new");
    expect(await readScratch("b".repeat(32))).toBe("");
  });

  it("decides closes from the live buffer, not a stale flag", async () => {
    scheduleScratchWrite(A, "typed just now");
    expect(await untitledCloseActionFor(A, true)).toBe("prompt");
    await flushScratch(A);
    expect(await untitledCloseActionFor(A)).toBe("prompt");
    disk.set(A, "  ");
    expect(await untitledCloseActionFor(A)).toBe("discard");
  });
});
