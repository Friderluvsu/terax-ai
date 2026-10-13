import { traceBuiltStartup } from "./scripts/built-startup.mjs";

export default [
  {
    name: "main window startup JS (all static dependencies)",
    path: traceBuiltStartup("dist/index.html"),
    limit: "540 KB",
    gzip: true,
  },
  {
    name: "settings window startup JS (all static dependencies)",
    path: traceBuiltStartup("dist/settings.html"),
    limit: "540 KB",
    gzip: true,
  },
  {
    name: "total client JS",
    path: ["dist/assets/*.js"],
    limit: "1505 KB",
    gzip: true,
  },
  {
    name: "Ghostty primary terminal JS (lazy entry and shared presentation)",
    path: [
      "dist/assets/useGhosttyTerminalSession-*.js",
      "dist/assets/TerminalFitQueue-*.js",
      "dist/assets/ghostty-vt-*.js",
    ],
    limit: "56 KB",
    gzip: true,
  },
  {
    name: "Ghostty WebGL fallback JS (loaded on demand)",
    path: [
      "dist/assets/WebGlTerminalSurface-*.js",
      "dist/assets/WebGlTerminalRuntime-*.js",
    ],
    limit: "15 KB",
    gzip: true,
  },
  {
    name: "Ghostty terminal WASM variants (one loaded per webview)",
    path: ["dist/assets/ghostty-vt-*.wasm"],
    limit: "450 KB",
    gzip: true,
  },
];
