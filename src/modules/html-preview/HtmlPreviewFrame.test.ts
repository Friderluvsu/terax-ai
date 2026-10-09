import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { relativeToRoot } from "./lib/htmlPreviewUrl";

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
  it("takes the iframe src from the backend's canonical url", () => {
    expect(src).toMatch(/served\?\.url/);
    expect(src).not.toMatch(/relativeToRoot|htmlPreviewUrl/);
  });
});

describe("HtmlPreviewFrame reload wiring", () => {
  it("re-runs preview_serve from the error state", () => {
    expect(src).toMatch(/serveNonce\]\)/);
    expect(src).toMatch(/reloadAction\(error\)/);
  });
  it("discards the pending reload when a suspended frame is shown", () => {
    expect(src).toMatch(/!loadedRef\.current\) discardPendingReload\(\)/);
  });
});

describe("non-canonical path alias", () => {
  it("is not derivable client-side, so the frame must not rely on it", () => {
    expect(
      relativeToRoot("/var/proj/a.html", "/private/var/proj", "macos"),
    ).toBeNull();
  });
});
