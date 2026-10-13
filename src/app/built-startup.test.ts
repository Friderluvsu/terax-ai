import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { traceBuiltStartup } from "../../scripts/built-startup.mjs";

const directories: string[] = [];

function fixture(files: Record<string, string>): string {
  const root = mkdtempSync(join(tmpdir(), "terax-startup-test-"));
  directories.push(root);
  mkdirSync(join(root, "assets"));
  for (const [name, content] of Object.entries(files))
    writeFileSync(join(root, name), content);
  return root;
}

afterEach(() => {
  for (const root of directories.splice(0))
    rmSync(root, { recursive: true, force: true });
});

describe("built startup graph", () => {
  it("counts transitive imports, re-exports and preloads once, excluding lazy code", () => {
    const root = fixture({
      "index.html":
        '<script type="module" src="/assets/main.js"></script><link rel="modulepreload" href="/assets/preload.js">',
      "assets/main.js":
        'import "./shared.js"; export { value } from "./bridge.js"; import("./lazy.js");',
      "assets/shared.js":
        'import "./main.js"; import "https://example.test/external.js";',
      "assets/bridge.js": 'export { value } from "./shared.js";',
      "assets/preload.js": "export const preloaded = true;",
    });
    expect(traceBuiltStartup(join(root, "index.html"))).toEqual(
      ["bridge", "main", "preload", "shared"].map((name) =>
        join(root, "assets", `${name}.js`),
      ),
    );
  });

  it("resolves relative entry URLs and ignores query strings", () => {
    const root = fixture({
      "index.html": "<script src='assets/main.js?v=1' type='module'></script>",
      "assets/main.js": 'import "./shared.js?v=2";',
      "assets/shared.js": "export const value = 1;",
    });
    expect(traceBuiltStartup(join(root, "index.html"))).toHaveLength(2);
  });

  it("fails if an eager dependency is missing", () => {
    const root = fixture({
      "index.html": '<script type="module" src="/assets/main.js"></script>',
      "assets/main.js": 'import "./missing.js";',
    });
    expect(() => traceBuiltStartup(join(root, "index.html"))).toThrow();
  });

  it("fails if no JavaScript entry is found", () => {
    const root = fixture({ "index.html": "<html></html>" });
    expect(() => traceBuiltStartup(join(root, "index.html"))).toThrow(
      "No startup JavaScript found",
    );
  });
});
