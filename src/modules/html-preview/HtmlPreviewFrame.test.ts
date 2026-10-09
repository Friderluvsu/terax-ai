import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const src = readFileSync(path.join(here, "HtmlPreviewFrame.tsx"), "utf8");
const iframeJsx = (src.match(/<iframe[\s\S]*?\/>/)?.[0] ?? "")
  .replace(/\/\*[\s\S]*?\*\//g, "")
  .replace(/\/\/[^\n]*/g, "");

describe("HtmlPreviewFrame iframe sandbox", () => {
  it("declares a sandboxed iframe", () => {
    expect(iframeJsx).toMatch(/sandbox="[^"]*allow-scripts/);
  });
  it("omits allow-same-origin and top navigation", () => {
    expect(iframeJsx).not.toMatch(/allow-same-origin/);
    expect(iframeJsx).not.toMatch(/allow-top-navigation/);
  });
  it("has no srcDoc and sends no referrer", () => {
    expect(iframeJsx).not.toMatch(/srcDoc/i);
    expect(iframeJsx).toMatch(/referrerPolicy="no-referrer"/);
  });
  it("lets popups escape the sandbox", () => {
    expect(iframeJsx).toMatch(/allow-popups-to-escape-sandbox/);
  });
  it("does not touch the fs watch commands", () => {
    expect(src).not.toMatch(/watchAdd|watchRemove|fs_watch/);
  });
});
