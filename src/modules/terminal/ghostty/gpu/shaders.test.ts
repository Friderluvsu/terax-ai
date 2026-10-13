import { describe, expect, it } from "vitest";
import { GLYPH_SHADER } from "./shaders";

describe("glyph atlas sampling", () => {
  it("needs no implicit derivatives after per-glyph branches or early returns", () => {
    expect(GLYPH_SHADER).not.toMatch(/\btextureSample\s*\(/);
    for (const texture of ["color_texture", "coverage_texture"])
      expect(GLYPH_SHADER).toMatch(
        new RegExp(
          `textureSampleLevel\\(${texture},\\s*glyph_sampler,\\s*input\\.uv,\\s*0\\.0\\)`,
        ),
      );
  });
});
