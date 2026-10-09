import { readFile } from "node:fs/promises";
import { beforeAll, describe, expect, it } from "vitest";
import { TeraxGhostty } from "@terax/ghostty-core/adapted";
import { AdaptedGhosttyTerminalModel } from "@/modules/terminal/ghostty/AdaptedGhosttyTerminalModel";

describe.each(["ghostty-vt", "ghostty-vt-scalar"])(
  "viewportLine (%s)",
  (artifact) => {
    let core: TeraxGhostty;
    beforeAll(async () => {
      const bytes = await readFile(
        new URL(
          `../../../../packages/ghostty-core/adapted/${artifact}.wasm`,
          import.meta.url,
        ),
      );
      core = await TeraxGhostty.loadBytes(Uint8Array.from(bytes).buffer);
    });

    function create() {
      const model = new AdaptedGhosttyTerminalModel(core, {
        backend: "ghostty-webgpu",
        cols: 10,
        rows: 6,
      });
      return {
        model,
        write: (text: string) => model.write(new TextEncoder().encode(text)),
      };
    }

    it("marks the continuation of a long line as wrapped", () => {
      const { model, write } = create();
      try {
        write("abcdefghijklmnop");
        const first = model.viewportLine(0);
        const second = model.viewportLine(1);
        expect(first?.wrapped).toBe(false);
        expect(first?.cells.join("")).toBe("abcdefghij");
        expect(second?.wrapped).toBe(true);
        expect(second?.cells.slice(0, 6).join("")).toBe("klmnop");
      } finally {
        model.dispose();
      }
    });

    it("uses an empty string for the spacer cell of wide characters", () => {
      const { model, write } = create();
      try {
        write("a世b");
        const line = model.viewportLine(0);
        expect(line?.cells.slice(0, 4)).toEqual(["a", "世", "", "b"]);
      } finally {
        model.dispose();
      }
    });

    it("returns null outside the viewport", () => {
      const { model } = create();
      try {
        expect(model.viewportLine(-1)).toBeNull();
        expect(model.viewportLine(6)).toBeNull();
      } finally {
        model.dispose();
      }
    });
  },
);
