import { afterEach, describe, expect, it, vi } from "vitest";
import type { GhosttyTerminalModelApi } from "@/modules/terminal/ghostty/GhosttyTerminalModel";
import type { WindowPresentation } from "@/modules/terminal/ghostty/WindowPresentationPolicy";
import type { TerminalFontMetrics } from "@/modules/terminal/ghostty/gpu/terminalVisuals";
import { WebGlCellRenderer } from "@/modules/terminal/ghostty/webgl/WebGlCellRenderer";
import { WebGlTerminalRuntime } from "@/modules/terminal/ghostty/webgl/WebGlTerminalRuntime";
import { WebGlTerminalSurface } from "@/modules/terminal/ghostty/webgl/WebGlTerminalSurface";

const bridge = vi.hoisted(() => ({
  runtime: null as unknown,
  visibility: new Set<(state: WindowPresentation) => void>(),
}));
vi.mock(
  "@/modules/terminal/ghostty/webgl/WebGlTerminalRuntime",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    getWebGlTerminalRuntime: () => bridge.runtime,
  }),
);
vi.mock("@/modules/terminal/ghostty/windowPresentation", () => ({
  terminalWindowPresentation: () => ({ visible: true, reclaim: false }),
  subscribeWindowPresentation: (
    listener: (state: WindowPresentation) => void,
  ) => {
    bridge.visibility.add(listener);
    return () => bridge.visibility.delete(listener);
  },
}));

const GLYPH_FLOATS = 15;

type GlyphDraw = { instances: number; widths: number[] };

const cleanup: (() => void)[] = [];
afterEach(() => {
  for (const dispose of cleanup.splice(0).reverse()) dispose();
  bridge.visibility.clear();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("WebGL surface after hide and show", () => {
  it.each([
    ["without a font change", false],
    ["with a fontsVersion change while hidden", true],
  ])("redraws every row on the first frame after show %s", (_, bump) => {
    const h = harness();
    const pane = h.pane();
    h.flush();
    expect(rowsDrawn(h.takeGlyphDraw(), pane.model)).toEqual(
      allRows(pane.model),
    );

    pane.hide();
    if (bump) pane.surface.setFontMetrics(withFontsVersion(METRICS, 1));
    pane.show();
    h.flush();

    expect(rowsDrawn(h.takeGlyphDraw(), pane.model)).toEqual(
      allRows(pane.model),
    );
  });

  it.each([
    ["without a font change", false],
    ["with a fontsVersion change while hidden", true],
  ])(
    "redraws every row after another tab borrowed the renderer %s",
    (_, bump) => {
      const h = harness();
      const first = h.pane();
      h.flush();
      first.hide();
      const second = h.pane();
      h.flush();
      second.hide();
      h.takeGlyphDraw();
      if (bump) {
        first.surface.setFontMetrics(withFontsVersion(METRICS, 1));
        second.surface.setFontMetrics(withFontsVersion(METRICS, 1));
      }
      first.show();
      h.flush();

      expect(rowsDrawn(h.takeGlyphDraw(), first.model)).toEqual(
        allRows(first.model),
      );
    },
  );

  it("redraws every row when a visible pane receives a new fontsVersion", () => {
    const h = harness();
    const pane = h.pane();
    h.flush();
    h.takeGlyphDraw();
    pane.surface.setFontMetrics(withFontsVersion(METRICS, 1));
    h.flush();

    expect(rowsDrawn(h.takeGlyphDraw(), pane.model)).toEqual(
      allRows(pane.model),
    );
  });

  it("redraws every row after the window returns from another space", () => {
    const h = harness();
    const pane = h.pane();
    h.flush();
    h.takeGlyphDraw();
    h.presentation({ visible: false, reclaim: false });
    h.presentation({ visible: false, reclaim: true });
    h.presentation({ visible: true, reclaim: false });
    h.flush();

    expect(rowsDrawn(h.takeGlyphDraw(), pane.model)).toEqual(
      allRows(pane.model),
    );
  });
});

const METRICS: TerminalFontMetrics = {
  font: {
    family: '"JetBrainsMono Nerd Font", monospace',
    size: 14,
    lineHeight: 1.2,
    letterSpacing: 0,
    weight: "400",
  },
  cellWidth: 8,
  cellHeight: 16,
  baseline: 12,
};
const THEME = {
  background: [0, 0, 0],
  foreground: [255, 255, 255],
  cursor: [255, 255, 255],
  selection: { color: [50, 50, 50], alpha: 0.5 },
  palette: [],
} as const;

function withFontsVersion(
  metrics: TerminalFontMetrics,
  fontsVersion: number,
): TerminalFontMetrics {
  return { ...metrics, font: { ...metrics.font, fontsVersion } };
}

function allRows(model: FakeModel): number[] {
  return Array.from({ length: model.rows }, (_, row) => row);
}

function rowsDrawn(draw: GlyphDraw | null, model: FakeModel): number[] {
  if (!draw || draw.instances !== model.cols * model.rows) return [];
  const rows: number[] = [];
  for (let row = 0; row < model.rows; row++) {
    let complete = true;
    for (let column = 0; column < model.cols; column++) {
      if (!(draw.widths[row * model.cols + column] > 0)) complete = false;
    }
    if (complete) rows.push(row);
  }
  return rows;
}

type FakeModel = ReturnType<typeof fakeModel>;

function fakeModel() {
  let damage: { kind: "none" } | { kind: "full" } = { kind: "full" };
  const listeners = new Set<() => void>();
  const model = {
    cols: 80,
    rows: 24,
    setCursorOptions: () => {},
    revision: () => 0,
    subscribeDamage: (listener: () => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    trackedSelection: () => null,
    selectionText: () => null,
    setSelection: () => {},
    scrollPosition: () => ({ history: 0, offset: 0 }),
    viewportOriginLine: () => 0,
    modes: () => ({ alternateScreen: false, mouseTracking: false }),
    setPixelSize: () => {},
    resize: (cols: number, rows: number) => {
      model.cols = cols;
      model.rows = rows;
      damage = { kind: "full" };
      for (const listener of listeners) listener();
    },
    cursor: () => ({
      x: 0,
      y: 0,
      visible: true,
      blinking: false,
      style: "block",
    }),
    deferPresentation: () => false,
    consumeDamage: () => {
      const current = damage;
      damage = { kind: "none" };
      return current;
    },
    releasePresentationResources: () => {
      damage = { kind: "full" };
    },
    compactPresentationResources: () => {
      damage = { kind: "full" };
    },
    searchViewportMatches: () => [],
    blockSearchActive: () => false,
    clearSearch: () => {},
    grapheme: () => "",
    renderCells: () => ({
      length: model.cols * model.rows,
      width: () => 1,
      flags: () => 0,
      codepoint: () => 0xe0b0,
      graphemeLength: () => 0,
      backgroundPacked: () => 0,
      foregroundPacked: () => 0xffffff,
      underlineColorPacked: () => 0,
      underlineStyle: () => 0,
      overline: () => false,
    }),
  };
  return model;
}

function fakeGl(onGlyphDraw: (draw: GlyphDraw) => void) {
  const constants = new Map<string, number>();
  const buffers: Float32Array[] = [];
  const programs: object[] = [];
  const handles: object[] = [];
  let bound: number | null = null;
  let program: object | null = null;
  const target: Record<string, unknown> = {
    createBuffer: () => {
      buffers.push(new Float32Array(0));
      const handle = { buffer: buffers.length - 1 };
      handles.push(handle);
      return handle;
    },
    bindBuffer: (_: number, handle: { buffer: number } | null) => {
      bound = handle ? handle.buffer : null;
    },
    bufferData: (_: number, data: number | ArrayBufferView) => {
      if (bound === null) return;
      buffers[bound] =
        typeof data === "number"
          ? new Float32Array(data / 4)
          : new Float32Array(
              data.buffer.slice(
                data.byteOffset,
                data.byteOffset + data.byteLength,
              ),
            );
    },
    bufferSubData: (_: number, offset: number, data: Float32Array) => {
      if (bound === null) return;
      const shadow = buffers[bound];
      if (shadow.length < offset / 4 + data.length) {
        const grown = new Float32Array(offset / 4 + data.length);
        grown.set(shadow);
        buffers[bound] = grown;
      }
      buffers[bound].set(data, offset / 4);
    },
    createProgram: () => {
      const value = {};
      programs.push(value);
      return value;
    },
    useProgram: (value: object) => {
      program = value;
    },
    drawElementsInstanced: (
      _mode: number,
      _count: number,
      _type: number,
      _offset: number,
      instances: number,
    ) => {
      if (program !== programs[1]) return;
      const glyphs = buffers[2];
      const widths: number[] = [];
      for (let index = 0; index < instances; index++)
        widths.push(glyphs[index * GLYPH_FLOATS + 2] ?? 0);
      onGlyphDraw({ instances, widths });
    },
    getExtension: () => null,
    getError: () => 0,
    getProgramParameter: () => true,
    getShaderParameter: () => true,
  };
  return new Proxy(target, {
    get: (object, key: string) => {
      if (key in object) return object[key];
      if (/^[A-Z_0-9]+$/.test(key)) {
        if (!constants.has(key)) constants.set(key, constants.size + 1);
        return constants.get(key);
      }
      const method = () => ({});
      object[key] = method;
      return method;
    },
  });
}

function fakeContext2d() {
  return {
    font: "",
    textAlign: "left",
    textBaseline: "alphabetic",
    fillStyle: "white",
    clearRect: () => {},
    fillText: () => {},
    measureText: () => ({
      width: 8,
      actualBoundingBoxLeft: 0,
      actualBoundingBoxRight: 7,
      actualBoundingBoxAscent: 10,
      actualBoundingBoxDescent: 2,
    }),
    getImageData: (_x: number, _y: number, width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4).fill(255),
    }),
  };
}

function harness() {
  vi.useFakeTimers();
  let lastGlyphDraw: GlyphDraw | null = null;
  type FakeNode = Record<string, unknown>;
  const element = (): FakeNode => {
    const node: FakeNode = Object.assign(
      new EventTarget() as unknown as FakeNode,
      {
        width: 1,
        height: 1,
        className: "",
        parentElement: null as unknown,
        style: { setProperty: () => {}, width: "", height: "" },
        scrollTop: 0,
        setAttribute: () => {},
        getAttribute: () => null,
        contains: () => false,
        focus: () => {},
        append: (...children: FakeNode[]) => {
          for (const child of children) child.parentElement = node;
        },
        appendChild: (child: FakeNode) => {
          child.parentElement = node;
        },
        prepend: (child: FakeNode) => {
          child.parentElement = node;
        },
        remove: () => {
          node.parentElement = null;
        },
        getBoundingClientRect: () => ({ width: 640, height: 384 }),
        getContext: (kind: string) =>
          kind === "webgl2"
            ? fakeGl((draw) => {
                lastGlyphDraw = draw;
              })
            : fakeContext2d(),
      },
    );
    return node;
  };
  const media = new EventTarget();
  vi.stubGlobal("document", { createElement: element });
  vi.stubGlobal("window", {
    devicePixelRatio: 1,
    setTimeout,
    clearTimeout,
    matchMedia: () => media,
  });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      disconnect() {}
    },
  );
  const frames = new Map<number, (frameAt?: number) => void>();
  let nextFrame = 1;
  let now = 0;
  const runtime = new WebGlTerminalRuntime({
    createRenderer: () => new WebGlCellRenderer(),
    now: () => now,
    isVisible: () => true,
    isWindowFocused: () => true,
    requestFrame: (callback) => {
      frames.set(nextFrame, callback);
      return nextFrame++;
    },
    cancelFrame: (handle) => frames.delete(handle),
    setTimer: (callback, delay) => window.setTimeout(callback, delay),
    clearTimer: (handle) => window.clearTimeout(handle),
    bindVisibility: () => () => {},
    bindWindowFocus: () => () => {},
  });
  bridge.runtime = runtime;
  cleanup.push(() => runtime.dispose());
  return {
    pane() {
      const model = fakeModel();
      const host = element() as unknown as HTMLElement;
      const surface = new WebGlTerminalSurface({
        model: model as unknown as GhosttyTerminalModelApi,
        metrics: METRICS,
        theme: THEME,
        cursorBlink: false,
        cursorStyle: "block",
        onResize: () => {},
        onError: (error) => {
          throw error;
        },
      });
      cleanup.push(() => surface.dispose());
      surface.attach(host);
      return {
        model,
        surface,
        hide() {
          surface.setVisible(false);
          surface.detach();
        },
        show() {
          surface.attach(host);
          surface.setVisible(true);
        },
      };
    },
    presentation(state: WindowPresentation) {
      for (const listener of bridge.visibility) listener(state);
    },
    flush() {
      for (let round = 0; round < 8; round++) {
        now += 100;
        vi.advanceTimersByTime(100);
        const pending = [...frames.values()];
        frames.clear();
        for (const callback of pending) callback(now);
      }
    },
    takeGlyphDraw() {
      const draw = lastGlyphDraw;
      lastGlyphDraw = null;
      return draw;
    },
  };
}
