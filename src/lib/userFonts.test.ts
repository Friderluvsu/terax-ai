import { afterEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const face = (id: number, weight: number, italic = false) => ({
  id,
  weight,
  italic,
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
  invoke.mockReset();
});

function stubFonts() {
  const added: unknown[] = [];
  class FakeFace {
    constructor(
      public family: string,
      public data: unknown,
      public desc: unknown,
    ) {}
    async load() {}
  }
  vi.stubGlobal("FontFace", FakeFace);
  vi.stubGlobal("document", { fonts: { add: (f: unknown) => added.push(f) } });
  return added;
}

describe("pickFaces", () => {
  it("picks the nearest face per style and caps at four", async () => {
    const { pickFaces } = await import("./userFonts");
    const faces = [
      face(1, 300),
      face(2, 400),
      face(3, 700),
      face(4, 400, true),
      face(5, 700, true),
      face(6, 800),
    ];
    const ids = pickFaces(faces, [400, 700, 600]).map((f) => f.id);
    expect(ids.length).toBeLessThanOrEqual(4);
    expect(ids).toContain(2);
    expect(ids).toContain(3);
  });
});

describe("ensureUserFonts", () => {
  it("loads the family and a nerd fallback, then feeds resolveFontFamily", async () => {
    const added = stubFonts();
    invoke.mockImplementation(
      async (cmd: string, args: { family?: string }) => {
        if (cmd === "fonts_resolve") {
          return args.family === "JetBrainsMono"
            ? { family: "JetBrains Mono", faces: [face(1, 400), face(2, 700)] }
            : { family: "X Nerd Font", faces: [face(3, 400)] };
        }
        if (cmd === "fonts_nerd_fallback") return "X Nerd Font";
        return new ArrayBuffer(4);
      },
    );
    const { ensureUserFonts, getUserFontsVersion } = await import(
      "./userFonts"
    );
    const { resolveFontFamily } = await import("./fonts");
    await ensureUserFonts("JetBrainsMono");
    expect(added).toHaveLength(3);
    expect(getUserFontsVersion()).toBe(1);
    expect(resolveFontFamily("JetBrainsMono")).toBe(
      '"JetBrainsMono", "JetBrains Mono", "X Nerd Font", "JetBrains Mono", SFMono-Regular, Menlo, monospace',
    );
  });

  it("caches per family and survives IPC failure", async () => {
    stubFonts();
    invoke.mockRejectedValue(new Error("boom"));
    const { ensureUserFonts } = await import("./userFonts");
    const a = ensureUserFonts("Foo");
    expect(ensureUserFonts("foo")).toBe(a);
    await expect(a).resolves.toBeUndefined();
    const { resolveFontFamily } = await import("./fonts");
    expect(resolveFontFamily("Foo")).toBe(
      '"Foo", "JetBrains Mono", SFMono-Regular, Menlo, monospace',
    );
  });

  it("uses the installed nerd font for an empty setting", async () => {
    stubFonts();
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "fonts_nerd_fallback") return "X Nerd Font";
      if (cmd === "fonts_resolve")
        return { family: "X Nerd Font", faces: [face(3, 400)] };
      return new ArrayBuffer(4);
    });
    const { ensureUserFonts } = await import("./userFonts");
    const { resolveFontFamily } = await import("./fonts");
    await ensureUserFonts("");
    expect(resolveFontFamily("")).toBe(
      '"X Nerd Font", "JetBrains Mono", SFMono-Regular, Menlo, monospace',
    );
  });
});
