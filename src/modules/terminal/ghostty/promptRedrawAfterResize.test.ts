import { readFile } from "node:fs/promises";
import { TeraxGhostty } from "@terax/ghostty-core/adapted";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { AdaptedGhosttyTerminalModel } from "@/modules/terminal/ghostty/AdaptedGhosttyTerminalModel";
import {
  PTY_RESIZE_INTERVAL_MS,
  PtyResizeScheduler,
} from "@/modules/terminal/lib/ptyResizeScheduler";

const PROMPT = "\uE0B0 ~/project \uE0A0 main $ ";
const encoder = new TextEncoder();

let ghostty: TeraxGhostty;
const models: AdaptedGhosttyTerminalModel[] = [];

beforeAll(async () => {
  const file = await readFile(
    new URL(
      "../../../../packages/ghostty-core/adapted/ghostty-vt.wasm",
      import.meta.url,
    ),
  );
  ghostty = await TeraxGhostty.loadBytes(Uint8Array.from(file).buffer);
});

afterEach(() => {
  for (const model of models.splice(0)) model.dispose();
  vi.useRealTimers();
});

describe("prompt after a terminal resize", () => {
  it("is cleared by the model, so the shell must see a size change", () => {
    const h = harness();
    h.fit(81, 24);
    expect(h.screen()).not.toContain("main $");
  });

  it("comes back after a resize that settles on a new size", () => {
    const h = harness();
    h.fit(80, 20);
    h.fit(90, 22);
    vi.advanceTimersByTime(PTY_RESIZE_INTERVAL_MS * 4);
    expect(h.screen()).toContain("main $");
    expect(h.pty()).toEqual({ cols: 90, rows: 22 });
  });

  it("comes back after a layout transient returns to the delivered size", () => {
    const h = harness();
    h.fit(80, 20);
    h.fit(80, 22);
    h.fit(80, 24);
    vi.advanceTimersByTime(PTY_RESIZE_INTERVAL_MS * 4);
    expect(h.screen()).toContain("main $");
    expect(h.pty()).toEqual({ cols: 80, rows: 24 });
  });
});

function harness() {
  vi.useFakeTimers();
  const model = new AdaptedGhosttyTerminalModel(ghostty, {
    backend: "ghostty-webgl",
    cols: 80,
    rows: 24,
  });
  models.push(model);
  let pty = { cols: 80, rows: 24 };
  const scheduler = new PtyResizeScheduler((cols, rows) => {
    if (pty.cols === cols && pty.rows === rows) return;
    pty = { cols, rows };
    model.write(encoder.encode(`\r\x1b[J${PROMPT}`));
  });
  scheduler.schedule(80, 24);
  vi.advanceTimersByTime(PTY_RESIZE_INTERVAL_MS);
  model.write(
    encoder.encode(`last login\r\n\x1b]133;A\x1b\\\x1b]133;B\x1b\\${PROMPT}`),
  );
  return {
    fit(cols: number, rows: number) {
      model.resize(cols, rows);
      scheduler.schedule(cols, rows);
    },
    screen: () => model.readText(model.rows),
    pty: () => pty,
  };
}
