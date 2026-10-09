# html-preview

## existingSupport
Verified in code:

- **No HTML rendering exists.** `.html` opens as a CodeMirror editor tab with the lazy lang-html grammar.
- **File-open dispatch** is `App.tsx:666-675` (`handleOpenFile`): markdown goes to `newMarkdownTab`, everything else to `openFileTab`, which creates an editor tab.
- **Markdown Raw/Rendered:**
  - The toggle flips the tab kind between `editor` and `markdown` through `setMarkdownView` (`useTabs.ts:1000-1037`), and refuses Rendered while the buffer is dirty.
  - The toggle UI is `MarkdownViewToggle.tsx`, shown in `EditorStack.tsx:105-113` and `MarkdownPreviewPane.tsx:65`.
  - Rendering is lazy (`MarkdownStackLazy.tsx`). It reads the file once with `fs_read_file` and has **no live reload**.
- **Web preview:**
  - `preview/PreviewPane.tsx` is an eagerly loaded sandboxed iframe with `allow-scripts allow-same-origin …` and no top-navigation.
  - It has a reload-by-remount nonce, a 30 s hidden suspend, an address bar with port presets, and "open in system browser" through plugin-opener `openUrl`.
  - There is no native child webview.
- **Media (image, video, audio, PDF)** renders inside `EditorPane.tsx:568-628` with `convertFileSrc`. The asset protocol is enabled with scope `["**"]` (`tauri.conf.json:31-34`), and the PDF iframe has no sandbox.
- **CSP** `frame-src` allows `'self' http: https: asset:`.
- **No custom URI scheme** is registered in Rust.
- **File watching:** a refcounted NonRecursive notify watcher (`fs/watch.rs`) on authorized dirs, skipping dist, build, out and similar. It emits `fs:changed`, and `fs_write_file` emits `fs:file-written`. Only editor tabs consume these (`useEditorFileSync.ts`).
- **Secret deny-list** is frontend and AI-only (`ai/lib/security.ts`). Rust `fs_read_file` has no deny-list.
- **Bundle budget:** `.size-limit.json` (540 KB startup, 1505 KB total gzip) and `src/app/eager-budget.test.ts` with `scripts/eager-graph.mjs`. The current eager graph is 295 modules with no heavy packages.

## report
## Area: rendered HTML preview for local .html files (Terax fork, macOS)

Everything below was read in code unless it is marked **[inferred]** or **[upstream]**. **[upstream]** means I checked it against the Tauri v2.12.0 source on GitHub (Cargo.lock pins tauri 2.12.0 and wry 0.57.0; @tauri-apps/api is 2.11.1). The repo was not modified. The only commands I ran were read-only, including `node scripts/eager-graph.mjs`.

---
### 1. How opening a file picks the tab kind

- **Single dispatcher: `handleOpenFile`** in `src/app/App.tsx:666-675`.
  - `isMarkdownPath(path)` goes to `newMarkdownTab(path)`.
  - Every other file goes to `openFileTab(path, pin ?? false)` and gets an `editor` tab. A single click fills the reusable preview slot.
  - Callers:
    - explorer `onOpenFile` (`App.tsx:1454`, `FileExplorer.tsx:555` Enter key, `FileExplorer.tsx:817-824` context "Open" with pin=true, `TreeRow.tsx:94` click)
    - source control (`App.tsx:1472`)
    - launch files (`App.tsx:677-714`, through the `terax:open-file` event and `get_launch_files`)
- **Paths that bypass `handleOpenFile` and always open an editor:**
  - `openContentHit` (`App.tsx:1302-1311`)
  - `openControlFile` for the CLI (`App.tsx:1313-1343`)
  - theme files (`useThemeFileEditing.ts:77`)
- **Extension predicate:** `isMarkdownPath` in `src/lib/utils.ts:8-10`, tested in `src/lib/utils.test.ts`. There is no `isHtmlPath` today.
- **Media is decided at render time, not as a tab kind.** Images, video, audio and PDF are not tab kinds. `EditorPane.tsx:568-628` renders them when `useDocument` reports `binary`/`toolarge` and the extension matches a hard-coded list (lines 570-581).
  - Note: `.svg` is UTF-8 text, so it opens in CodeMirror, not as an image.
- **HTML today** is a plain `editor` tab. Its language is HTML (`languageDefinitions.ts:168-178`, lazy `@codemirror/lang-html`).
- **Tab model** (`src/modules/tabs/lib/useTabs.ts`):
  - Union at lines 128-136; `MarkdownTab` at 75-80; `PreviewTab` (url) at 68-73; `EditorTab` (path, dirty, preview, overrideLanguage) at 53-66.
  - Plans: `planMarkdownTabOpen` 174-203; `planFileTabOpen` 205-282.
  - Openers: `openFileTab` 842-859; `newPreviewTab` 952-966; `newMarkdownTab` 972-986.
  - `updateTab` 1163-1212, with a markdown branch at 1188-1194.
- **Existing dedupe bug (verified):** `planMarkdownTabOpen` only looks for `kind === "markdown"`, and `planFileTabOpen` only looks for `kind === "editor"`.
  - If a .md tab was flipped to Raw (it becomes `kind: "editor"`), clicking the file again in the explorer creates a second (markdown) tab for the same path.
  - A mirrored HTML implementation must dedupe across both kinds.

### 2. The Markdown Raw/Rendered toggle, end to end

- **Toggle state is the tab kind itself.**
  - `setMarkdownView(id, mode)` (`useTabs.ts:1000-1037`) rewrites the tab between `kind: "markdown"` and `kind: "editor"`.
  - Rendered is refused while the editor is dirty (`useTabs.ts:1021`).
  - It leaks `overrideLanguage` onto the markdown object (lines 1013-1016 and 1029), which `MarkdownTab` does not declare.
  - It is wired through `App.tsx:163` → `WorkspaceSurface onSetMarkdownView` (`App.tsx:1512`) → both `EditorStack` and `MarkdownStack`.
- **Raw side:** `EditorStack.tsx:105-113` renders `<MarkdownViewToggle mode="raw" renderedDisabled={t.dirty} renderedHint="Save to preview">` absolutely positioned over `EditorPane`, only when `isMarkdownPath(t.path)`.
- **Rendered side:**
  - `MarkdownStack.tsx:11-39` filters `kind === "markdown" && !cold`, keeps every pane mounted and hides inactive ones.
  - `MarkdownPreviewPane.tsx:30-56` reads the file **once** with `invoke("fs_read_file", { path, workspace: currentWorkspaceEnv() })`; the effect depends only on `[path]`.
  - It renders `<MarkdownViewToggle mode="rendered">` (line 65) plus Streamdown (lines 86-94).
- **No live reload for markdown (verified).** `useEditorFileSync.ts` only handles `kind === "editor"` for reload on `fs:file-written` (43-63), for the watch set (65-75) and for `fs:changed` (77-96). The rendered view refreshes only when the pane remounts after a Raw→Rendered flip.
- **Toggle UI:** `MarkdownViewToggle.tsx:12-49`, with hard-coded labels "Rendered" and "Raw".
- **Lazy loading:**
  - `markdown/index.ts` exports `MarkdownStack` from `MarkdownStackLazy.tsx:5-17` (`React.lazy` plus `Suspense fallback={null}`) and the tiny toggle.
  - The editor uses the same pattern (`EditorStackLazy.tsx`).
- **Every place that knows the `markdown` kind** (a new kind must touch each):
  - `useTabs.ts` (77, 183, 195, 1009, 1024, 1188)
  - `WorkspaceSurface.tsx:71, 120-129`
  - `TabBar.tsx:628` (file icon)
  - `TabSwitcherHud.tsx:13`
  - `SpaceSwitcher.tsx:67`
  - `tabLabel.ts:12`
  - `tabCloseGuards.ts:30, 74` (rename/delete guards)
  - `App.tsx:719` (rename follow) and `App.tsx:760` (explorer active file)
  - `spaces/lib/serialize.ts:27, 63, 84-85, 188-196` (persistence)
  - `tabs/index.ts` (type export)
- **Tests:**
  - `planFileTabOpen.test.ts:107-190` covers `planMarkdownTabOpen`.
  - `MarkdownPreviewPane.test.ts` is a source-regex test; there is no jsdom, and vitest runs in the node environment.
  - `serialize.test.ts`
  - `utils.test.ts`

### 3. The preview module (`src/modules/preview/`)

- **It is a plain `<iframe>`, not a native child webview.** `src-tauri` has no `WebviewBuilder`, `add_child` or child-webview code. The only `WebviewWindowBuilder` is the Settings window (`lib.rs:108`).
- **`PreviewPane.tsx`:**
  - The iframe (155-169) uses `sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox allow-downloads"`, `referrerPolicy="no-referrer"`, and `key={url#nonce}`. Reload bumps the nonce, which remounts the frame (comment at 41-44).
  - Hidden frames are torn down after `SUSPEND_AFTER_MS = 30_000` (35-58).
  - The imperative handle exposes `reload`, `focusAddressBar` and `getUrl` (60-71).
  - It shows an X-Frame-Options hint for non-loopback URLs and a cookie hint for loopback (73-76, 92-145).
- **`PreviewPane.test.ts:25-58`** asserts by source regex: a sandbox is present, it includes allow-scripts and allow-same-origin, there is no `allow-top-navigation`, popups imply escape-sandbox, and `no-referrer` is set.
- **URL validation is weak.**
  - `lib/previewUrl.ts:3-22` (`loopbackPreviewOrigin`) is used only to choose a hint.
  - `PreviewAddressBar.tsx:236-244` (`normalizeUrl`) adds a scheme but otherwise passes text through.
  - The real limit is the parent CSP `frame-src`.
  - The AI tool `open_preview` (`ai/tools/terminal.ts:63-100`) allows only http/https on loopback hosts.
- **Address bar** (`PreviewAddressBar.tsx`): port presets (31-48), `fetch` no-cors probe (222-234), and "Open in system browser" through `openUrl` from plugin-opener (187-203).
- **Eager loading (verified):** `preview/index.ts` exports `PreviewStack` directly (not lazy). `traceEager("src/main.tsx")` lists `PreviewStack.tsx`, `PreviewPane.tsx`, `PreviewAddressBar.tsx` and `previewUrl.ts` as eager.
- **Wiring:** `App.tsx:810-820` (`openPreviewTab`), `1062-1073` (handles, `updateTab({url})`), `WorkspaceSurface.tsx:109-119`. Serialized tabs persist `{kind:"preview", url}` (`serialize.ts:26, 82-83, 179-187`).

### 4. How media viewers load local bytes; asset protocol, CSP and capabilities

- **Loading:**
  - `EditorPane.tsx:584` uses `convertFileSrc(path)`. `<img>`, `<video>` and `<audio>` use the asset URL directly.
  - The **PDF uses `<iframe src={assetUrl}>` with no sandbox** (619-624).
  - There are no blob URLs and no base64 over IPC.
- **Config:**
  - `Cargo.toml:35` enables tauri features `macos-private-api` and `protocol-asset`.
  - `tauri.conf.json:31-34` sets `assetProtocol { enable: true, scope: ["**"] }`, which covers the **whole filesystem**.
  - CSP at `tauri.conf.json:30`: `script-src 'self' 'wasm-unsafe-eval'` (no unsafe-inline); `style-src 'self' 'unsafe-inline'`; `img-src`/`media-src` allow `asset: https://asset.localhost blob:`; `frame-src 'self' http: https: asset: https://asset.localhost`; `connect-src` includes `https: http://localhost:* http://127.0.0.1:*`.
- **Capabilities:** `capabilities/default.json` grants `opener:default`. **[upstream]** That set is allow-open-url, allow-reveal-item-in-dir and allow-default-urls (http, https, mailto, tel). **Not open-path**, so "open this .html in the system browser" needs a Rust command (`tauri_plugin_opener` Rust API) or a new scoped capability.
- **No custom schemes:** there is no `register_uri_scheme_protocol` or `register_asynchronous_uri_scheme_protocol` anywhere (builder at `lib.rs:187-213`).
- **[upstream] `convertFileSrc`** (`crates/tauri/scripts/core.js`) is `encodeURIComponent(filePath)`, giving `asset://localhost/%2FUsers%2F...`. Because the slashes are encoded, **relative URLs inside an HTML page loaded this way resolve against the root and break**.
- **[upstream] `protocol/asset.rs`:**
  - The path is `percent_decode(uri.path()[1..])`.
  - `Access-Control-Allow-Origin` is set to the window origin.
  - `SafePathBuf` rejects `..`, then the scope is checked.
  - The MIME type is sniffed from 8 KiB plus the path.
  - Ranges are supported (MAX_LEN 1000 KiB).
  - **No CSP header is sent.**
- **[upstream] Frames and IPC** (`manager/webview.rs`, `webview/mod.rs`, `ipc/protocol.rs`):
  - Core and IPC init scripts are **main-frame-only** (`for_main_frame_only: true`), so iframes get no `__TAURI_INTERNALS__`.
  - `is_local_url` treats the app origin **and any user-registered custom protocol scheme** as LOCAL. Local origins can call app commands without an ACL; remote origins need a `remote` capability, and there is none.
  - The remaining barrier is the per-launch **invoke key**: `on_message` compares `request.invoke_key != manager.invoke_key()` and silently drops a mismatch. The key lives only in the main-frame script.
  - For `ipc://` requests the "url" is the `Origin` header, so an opaque-origin frame sends `null`, which fails URL parsing and is rejected.

### 5. Path guards

- **The secret-path deny-list is frontend and AI-tools only.** `src/modules/ai/lib/security.ts` holds the patterns (31-59), protected dirs (66-98), `checkReadable` (198-229) and `checkReadableCanonical` (265-284).
- **Rust `fs_read_file` (`fs/file.rs:57-103`) has no deny-list and no registry check.**
  - It only resolves the path for WSL (`workspace.rs:339-349`).
  - Limits are 10 MB, or 50 MB with `force`; it sniffs for NUL and checks UTF-8.
- **Patterns worth reusing in Rust:**
  - `fs/mutate.rs:39-70`: `resolve_authorized_root` (canonicalize, then `registry.is_authorized`) and `authorize_mutation_entry` (canonical parent `starts_with(root)`).
  - `WorkspaceRegistry` (`workspace.rs:20-36`) is bootstrapped with the launch dir **and $HOME** (`workspace.rs:135-140`).
  - `control.rs:558-567` `generate_token` (getrandom, hex) and `control.rs:109` (a 127.0.0.1:0 `TcpListener` precedent).

### 6. File watching (for live reload)

- **`fs/watch.rs`:**
  - One notify watcher with **NonRecursive** directory watches (186), refcounted (97-102, 182-208).
  - Watches are added only for canonical, **authorized** dirs whose basename is **not in `SKIP_DIRS`** (211-227). The skip list (19-84) includes `dist`, `build`, `out`, `.next`, `node_modules`, `target`.
  - Events are debounced 150 ms, max window 1 s (13-15).
  - It emits `fs:changed {paths}` with **canonical** paths via `to_canon` (163-178).
- **`fs_write_file` (`file.rs:127-158`)** writes atomically (tempfile then rename, 114-123) and emits `fs:file-written {path (raw arg), source}`. The editor saves with `source:"editor"` (`useDocument.ts:63-67`).
- **Frontend helpers:** `explorer/lib/watch.ts:9-32` (`watchAdd`, `watchRemove`, `listenFsChanged`) and `parentDir` (34-38). Consumers are `useEditorFileSync` (editor dirs) and `explorer/lib/useFileTree.ts:168-282` (expanded dirs).
- **Consequences for an HTML preview:**
  - (i) The preview must request its own watches.
  - (ii) Assets in subdirectories (`css/`, `js/`) are not seen by a non-recursive watch on the HTML's own directory.
  - (iii) **A preview rooted at `.../dist` or `.../build` would never get `fs:changed`.** Only Terax-internal saves would arrive, through `fs:file-written`.
  - (iv) A single save produces both `fs:file-written` and, about 150 ms later, `fs:changed`, so the preview must coalesce them.
  - (v) Compare event paths against a **canonical** root, because editor tab paths are raw and events are canonical (`useEditorFileSync.ts:81-85` has this mismatch today).

### 7. Bundle budget enforcement

- **`.size-limit.json`:** main startup JS (main, react, radix, alert-dialog) ≤ 540 KB gzip; total client JS ≤ 1505 KB gzip. CI runs `pnpm size` (`.github/workflows/ci.yml:51`).
- **`src/app/eager-budget.test.ts`:**
  - The HEAVY list (10-19: @ai-sdk, ai, streamdown, @codemirror, @uiw, motion, xterm) must not be eagerly reachable from `src/main.tsx` or `src/settings/main.tsx` (58-64).
  - Lines 27-34 show a per-file assertion pattern built on `traceEager().files`.
  - The tracer (`scripts/eager-graph.mjs`) follows static imports only; `lazy(() => import())` is a boundary.
  - Current baseline (run read-only): 295 eager local modules, no heavy hits.
- **`vite.config.ts:72-140` (`manualChunks`)** pins the preload helper and clsx/tailwind-merge into `react`, so a new lazy chunk does not drag the startup graph.
- **For new code:** an HTML preview needs no new npm dependency. Keep it behind a `lazy()` boundary (or inside the already-lazy EditorStack chunk). Add a per-file eager assertion like lines 27-34.

---
### 8. Options for rendering an HTML file with its relative assets and live reload

**(a) `iframe srcdoc` built from `fs_read_file` content**
- Cost: lowest; no Rust, about 100 lines of frontend.
- Fidelity is poor.
  - srcdoc inherits the parent CSP, so inline and external scripts are blocked: `script-src 'self'` has no unsafe-inline.
  - External CSS from the file's directory is blocked: `style-src` has no `asset:`.
  - Relative URLs resolve against `tauri://localhost`.
  - Images would need rewriting to asset URLs.
- Security: **never** combine srcdoc with `allow-same-origin` or with no sandbox. The frame would share the app origin and could call `parent.__TAURI_INTERNALS__`, which is full IPC (PTY, fs, shell).
- Use: a safe static fallback only.

**(b) Asset protocol URL**
- Cost: cheap; no Rust.
- Fidelity: `convertFileSrc` breaks relative URLs. Hand-building `asset://localhost/<encodeURIComponent(dir)>/<rel>` fixes same-dir and subdir links but not `../` or `/root-relative`.
- Security: **dangerous**.
  - The scope is `"**"`, so with `allow-same-origin` page scripts can `fetch('asset://localhost/%2FUsers%2F<me>%2F.ssh%2Fid_rsa')` same-origin, read any file, and exfiltrate it. Asset responses carry no CSP and bypass the deny-list.
  - Without `allow-same-origin`, ACAO is `tauri://localhost`, so fetch, XHR and module scripts fail.
  - The asset scheme may also count as "local" for IPC **[inferred]**.
- Verdict: not recommended unless the asset scope is narrowed first.

**(c) Custom async URI scheme served from Rust and scoped to a registered root — RECOMMENDED**
- Mechanism: `tauri::Builder::register_asynchronous_uri_scheme_protocol("terax-preview", …)` in `lib.rs`. A page URL looks like `terax-preview://<token>/index.html` on macOS/Linux and `http://terax-preview.localhost/<token>/…` on Windows.
- What it gives:
  - Correct relative, root-relative and `../` resolution within the root.
  - A per-root origin on macOS because the token is the host.
  - Full control of response headers: `Content-Type` from our own table with octet-stream fallback (**[upstream]** `tauri_utils::MimeType` falls back to **text/html** for unknown extensions, so avoid it or pass `OctetStream`), `X-Content-Type-Options: nosniff`, `Cache-Control: no-store`, `Access-Control-Allow-Origin: *` (needed for module scripts from an opaque origin), and a CSP header for the previewed page.
  - Traversal guard: reject `..`, NUL, absolute paths; canonicalize; require `starts_with(root)`.
  - A Rust port of the secret deny-list.
  - Optional single-range support so `<video>` can seek.
- No new crate is needed: `http`, `percent-encoding` and `url` are already in `Cargo.lock`.
- Security model:
  - Sandbox **without `allow-same-origin`** (opaque origin). This blocks reading the parent, rejects `ipc://` (Origin `null`) and keeps storage partitioned.
  - The invoke key is the barrier against the postMessage IPC path (`webkit.messageHandlers.ipc` is reachable from subframes **[inferred]**).
  - Caveat: Tauri classifies custom-scheme origins as LOCAL, so **never inject Tauri scripts into these frames and never navigate the main frame to the scheme**.
- Parent CSP needs one token: add `terax-preview:` to `frame-src`. Windows' `http://terax-preview.localhost` is already covered by `http:`.
- Cost: about 250 lines of Rust plus tests, and a small frontend frame.

**(d) Tiny 127.0.0.1 static server**
- Pros:
  - `http://127.0.0.1:port` frames are REMOTE to Tauri, which is the strongest IPC isolation.
  - The existing `PreviewPane`/`PreviewTab` can be reused almost as-is.
  - "Open in system browser" works through `openUrl` without new permissions.
  - The AI `open_preview` loopback rule already accepts it.
- Cons:
  - A network listener reachable by any local process or browser tab. It needs a random token in the path, a `Host` header check against DNS rebinding, connection caps and lifecycle management.
  - HTTP/1.1 parsing must be hand-rolled on std (`control.rs` precedent) or a crate added (binary size matters).
  - Persisted `preview` tabs would store a dead ephemeral port and token, so a file-path tab kind is needed anyway.
  - The existing loopback "cookie" hint (`PreviewPane.tsx:73-76`) would misfire.
- Verdict: viable second choice. Pick it if "open in external browser" or sharing with real browsers is a hard requirement.

**Network egress from the preview** is best effort in (c) and (d). A response CSP `connect-src 'self'` blocks fetch, but navigation and popups can still leak. The real control is the scope (one root plus the deny-list). The "scripts on/off" and "allow network" toggles are decisions for the user.

### 9. Recommended design (c), with exact seams

**Rust — new `src-tauri/src/modules/preview.rs`, registered in `modules/mod.rs`:**
- `PreviewState { roots: Mutex<HashMap<String, PreviewRoot{root: PathBuf, entry: PathBuf}>> }`, capped at about 32 entries. Use a 16-byte hex token (32 chars; a 64-char token would exceed the 63-char host-label limit).
- `#[tauri::command] preview_serve(path, root: Option<String>, workspace: Option<WorkspaceEnv>, state, registry) -> {token, url, root}`:
  - Canonicalize the file and the root (default: the file's parent).
  - Require `file.starts_with(root)` and `registry.is_authorized(root)`, reusing the `mutate.rs:39-70` pattern.
  - Apply the deny-list to root and file.
- `#[tauri::command] preview_release(token)`.
- Optional `preview_open_external(path)` that calls `tauri_plugin_opener` from Rust.
- Pure, testable helpers: `split_preview_uri`, `sanitize_rel_path`, `resolve_within_root`, `is_secret_path` (Rust port of `security.ts:31-98`), `mime_for`, `parse_range`.
- The handler does file IO in `tauri::async_runtime::spawn_blocking`.
- Live reload of subdirectories: record the directories actually served per token and add NonRecursive watches through a new `pub` helper in `fs/watch.rs` that **bypasses `SKIP_DIRS` for explicitly previewed dirs** but still requires authorization. Release them on `preview_release`.

**`lib.rs`:**
- `.manage(preview::PreviewState::default())` next to lines 234-252.
- `.register_asynchronous_uri_scheme_protocol("terax-preview", preview::handle)` on the builder before `.build()` (around 193-213).
- Add commands to `generate_handler!` (253-344).

**`tauri.conf.json:30`:** add `terax-preview:` to `frame-src`. Leave `assetProtocol` alone for this task, but flag it.

**Frontend:**
- `src/lib/utils.ts`: add `isHtmlPath` (`/\.(html?|xhtml)$/i`) plus tests.
- New lazy module `src/modules/html-preview/`, or `preview/HtmlFramePane.tsx` behind its own `lazy()`:
  - `HtmlPreviewFrame` mounts `preview_serve` and releases on unmount.
  - The iframe has `key=nonce` and `sandbox="allow-scripts allow-forms allow-modals allow-popups allow-popups-to-escape-sandbox"` (no allow-same-origin, no allow-top-navigation), plus `referrerPolicy="no-referrer"`.
  - Reuse the 30 s hidden-suspend logic from `PreviewPane.tsx:35-58`.
  - Toolbar: Reload, Open externally, Code/Preview toggle.
- `lib/htmlPreviewUrl.ts`: pure URL builder that encodes per segment and keeps `/`, with the platform host/path form.
- `useHtmlPreviewReload`:
  - `watchAdd([parentDir(file)])`.
  - `listenFsChanged` filtered by `pathAtOrUnder(canonicalRoot)` (the helper exists in `app/hooks/tabCloseGuards.ts:7-14`; move it to `src/lib` to avoid an app→module import).
  - Also listen to `fs:file-written`.
  - Trailing debounce of about 200 ms; defer the reload while hidden.
- **Tab integration — pick one:**
  - **(A) Mirror markdown.** Add a new `HtmlTab {kind:"html", path}`. Generalize `setMarkdownView` into `setDocumentView(id, mode)` with a pure `renderedKindFor(path)` returning markdown, html or null. Write a `planRenderedTabOpen` that dedupes across editor, markdown and html (this also fixes the md duplicate bug). Touch every kind site listed in §2. Add `{kind:"html", path}` to `serialize.ts`. Add a `WorkspaceSurface` layer. Make `MarkdownViewToggle` labels generic.
  - **(B) Lower risk, better UX.** Keep HTML as an `editor` tab with a new `htmlView?: "code"|"split"|"preview"` field. `EditorStack.tsx:105-120` renders `EditorPane` (kept mounted, so the undo buffer survives) next to or under `HtmlPreviewFrame`. No dirty block is needed; the preview shows the last save. Changes are limited to `EditorTab`, `EditorStack`, an optional field in serialize, and the toggle.
- **`App.tsx:666-675`:** decide the default for .html; code with a toggle is recommended. Optionally add an explorer context item "Open Preview" next to "Open" (`FileExplorer.tsx:817-824`, which needs a new prop).
- **Bonus:** extend the same reload hook to markdown tabs. `MarkdownPreviewPane` currently never reloads.
- **Docs:** update `TERAX.md` (module list, tab kinds), `docs/architecture/two-process-model.md` (new commands) and `security-model.md` (new boundary).

### 10. Test plan

**Rust (`cargo nextest`):**
- `sanitize_rel_path` rejects `..`, encoded `%2e%2e`, absolute paths, NUL and backslash, and maps empty or trailing `/` to `index.html`.
- `resolve_within_root` with a tempdir: an inside file is OK; a missing file gives 404; a `#[cfg(unix)]` symlink escaping the root is refused; a symlinked root works.
- The deny-list refuses `.env`, `id_rsa`, `.ssh/x` and case variants under the root.
- URI split for the macOS form `terax-preview://tok/a/b.css` and the Windows form `http://terax-preview.localhost/tok/a/b.css`.
- Unknown or released token gives 404; `preview_serve` refuses an unauthorized root and a file outside the root.
- MIME table: html, htm, js, mjs, css, svg, wasm, json, mp4, woff2; unknown gives octet-stream.
- Single-range parse, plus 416.
- The watch helper for preview dirs does not apply `SKIP_DIRS` but does require authorization.

**Vitest (node environment):**
- `isHtmlPath` tests.
- URL builder: spaces, `#`, `?`, unicode, nested paths.
- `shouldReloadPreview(changed, canonicalRoot)` and the debounce.
- Tab transforms: `planRenderedTabOpen` dedupe across kinds, `setDocumentView` (or `htmlView`) transitions, serialize round-trip.
- Source-regex sandbox test for the new iframe, modeled on `PreviewPane.test.ts`: sandbox present, **no** `allow-same-origin`, no `allow-top-navigation`, `no-referrer`, no `srcDoc`.
- `eager-budget.test.ts`: assert the html-preview files are not in `traceEager("src/main.tsx").files`.
- A config test that `tauri.conf.json` `frame-src` includes `terax-preview:`.

**Manual, macOS `pnpm tauri dev`:**
- A page with `./style.css`, `js/app.mjs` (module), `../shared.css`, `/root.css`, an `<img>`, and an mp4 with seeking.
- Save CSS in Terax → auto-reload. Edit externally → reload. A page under `dist/` → reload.
- A malicious page that tries:
  - `parent.__TAURI_INTERNALS__` → expect SecurityError
  - `fetch('terax-preview://<tok>/../../.ssh/id_rsa')` and an encoded variant → expect 403/404
  - `fetch('asset://localhost/%2FUsers...')` → expect CORS failure
  - `webkit.messageHandlers.ipc.postMessage({...})` without the key → expect it dropped
  - `top.location='https://x'` → expect it blocked
- Hidden tab: the frame suspends after 30 s and reloads on show.
- `pnpm build && pnpm size` stays within the 540/1505 KB budgets.
- `pnpm lint`, `pnpm check-types`, `pnpm test`, `cargo clippy -D warnings`.

## keyFiles
- /Users/frider/Documents/iamcoder/terax_claude/src/app/App.tsx: handleOpenFile dispatcher (666-675); launch/open-file events (677-714); rename follow (716-730); explorerActiveFilePath (759-762); openPreviewTab (810-820); preview handles (1062-1073); WorkspaceSurface wiring (1494-1513)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/tabs/lib/useTabs.ts: Tab union and types (33-136); planMarkdownTabOpen (174-203); planFileTabOpen (205-282); openFileTab (842-859); newPreviewTab (952-966); newMarkdownTab (972-986); setMarkdownView toggle (1000-1037); updateTab (1163-1212)
- /Users/frider/Documents/iamcoder/terax_claude/src/lib/utils.ts: isMarkdownPath (8-10); place to add isHtmlPath (tests in src/lib/utils.test.ts)
- /Users/frider/Documents/iamcoder/terax_claude/src/app/components/WorkspaceSurface.tsx: Per-kind stacked layers (67-160); needs a new layer only if a new tab kind is added
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/editor/EditorStack.tsx: Renders the raw-markdown toggle over EditorPane (105-120); seam for an HTML Code/Split/Preview layout
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/markdown/MarkdownPreviewPane.tsx: Pattern to mirror: one-shot fs_read_file (30-56), toggle (65); has no live reload
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/markdown/MarkdownStackLazy.tsx: Lazy boundary pattern for a new stack
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/markdown/MarkdownViewToggle.tsx: Toggle UI with hard-coded Rendered/Raw labels; generalize or reuse
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/preview/PreviewPane.tsx: Iframe sandbox, reload nonce, 30 s hidden suspend (35-58, 155-169); reusable frame logic
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/preview/PreviewPane.test.ts: Source-regex sandbox test template for the new iframe
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/preview/lib/previewUrl.ts: Loopback detection, used only for UI hints (not a validator)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/editor/EditorPane.tsx: Media viewers through convertFileSrc and the asset protocol (568-628); PDF iframe without sandbox (619-624)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/tauri.conf.json: CSP (line 30, frame-src needs terax-preview:); assetProtocol scope ** (31-34)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/lib.rs: Builder (187-213) where register_asynchronous_uri_scheme_protocol goes; .manage (234-252); generate_handler! (253-344)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/watch.rs: NonRecursive refcounted watcher, SKIP_DIRS (19-84) incl. dist/build/out, authorization filter (211-227), fs:changed emit (163-168)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/file.rs: fs_read_file with no deny-list (57-103); fs_write_file emits fs:file-written (127-158); atomic write (114-123)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/mutate.rs: resolve_authorized_root and starts_with(root) guard pattern (39-70) to reuse for the preview root
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/workspace.rs: WorkspaceRegistry is_authorized (20-36); bootstrap authorizes $HOME (135-140); resolve_path (339-349)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/control.rs: Precedents for a 127.0.0.1 TcpListener (109) and getrandom hex generate_token (558-567)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/ai/lib/security.ts: Secret deny-list (31-98, checkReadable 198-229) to port to Rust for the protocol handler
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/explorer/lib/watch.ts: watchAdd, watchRemove, listenFsChanged, parentDir helpers
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/editor/useEditorFileSync.ts: How editor tabs reload on fs:file-written and fs:changed; editor-only watch set (65-75)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/spaces/lib/serialize.ts: Tab persistence (18-27, 57-89, 144-200); must learn any new kind or field
- /Users/frider/Documents/iamcoder/terax_claude/src/app/hooks/tabCloseGuards.ts: pathAtOrUnder helper (7-14); open-path and deleted-path guards that enumerate editor and markdown kinds (27-33, 74)
- /Users/frider/Documents/iamcoder/terax_claude/src/app/eager-budget.test.ts: Eager startup budget test; per-file assertion pattern at 27-34
- /Users/frider/Documents/iamcoder/terax_claude/.size-limit.json: Gzip budgets: 540 KB startup, 1505 KB total
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/explorer/FileExplorer.tsx: Context menu 'Open' item (817-824), seam for an 'Open Preview' item

## integrationPoints
- src-tauri/src/modules/preview.rs (new) plus `pub mod preview;` in src-tauri/src/modules/mod.rs: PreviewState, the preview_serve and preview_release commands, the async protocol handler and pure helpers (split_preview_uri, sanitize_rel_path, resolve_within_root, is_secret_path, mime_for, parse_range)
- src-tauri/src/lib.rs:193-213: add .register_asynchronous_uri_scheme_protocol("terax-preview", preview::handle) on the Builder before .build(); add .manage(preview::PreviewState::default()) near 234-252; add the preview_* commands to generate_handler! at 253-344
- src-tauri/tauri.conf.json:30: add `terax-preview:` to frame-src. Windows http://terax-preview.localhost is already covered by `http:`
- src-tauri/src/modules/fs/watch.rs: expose a pub helper that adds and releases NonRecursive watches for preview-served dirs without the SKIP_DIRS filter, still requiring registry.is_authorized, so previews under dist/ or build/ live-reload
- src/lib/utils.ts: add isHtmlPath next to isMarkdownPath (line 8), with tests in src/lib/utils.test.ts
- src/app/App.tsx:666-675 (handleOpenFile): choose the default open mode for .html. Context-menu 'Open Preview' goes in FileExplorer.tsx:817-824 through a new prop wired at App.tsx:1447-1465
- Option B (recommended): src/modules/tabs/lib/useTabs.ts EditorTab (53-66) gains an htmlView field and a setter; src/modules/editor/EditorStack.tsx:105-120 renders the Code/Split/Preview layout with a lazy HtmlPreviewFrame; src/modules/spaces/lib/serialize.ts gets an optional persisted field
- Option A (mirror markdown): new HtmlTab kind in useTabs.ts (128-136); generalize setMarkdownView (1000-1037) into setDocumentView using a pure renderedKindFor; planRenderedTabOpen dedupes across editor, markdown and html; update WorkspaceSurface.tsx (67-160), TabBar.tsx:628, TabSwitcherHud.tsx:13, SpaceSwitcher.tsx:67, tabLabel.ts:12, tabCloseGuards.ts:30,74, App.tsx:719,760, serialize.ts and tabs/index.ts
- New lazy frontend module (e.g. src/modules/html-preview/ or a lazy file under src/modules/preview/): HtmlPreviewFrame (sandbox without allow-same-origin or allow-top-navigation, key-nonce reload, 30 s hidden suspend copied from PreviewPane.tsx:35-58), lib/htmlPreviewUrl.ts pure builder, useHtmlPreviewReload hook using explorer/lib/watch.ts plus fs:file-written with a debounce and canonical-root prefix match
- Move pathAtOrUnder from src/app/hooks/tabCloseGuards.ts:7-14 into src/lib so the new module can import it without depending on app/
- src/app/eager-budget.test.ts: add a per-file assertion (pattern at 27-34) that the html-preview files are not eagerly reachable from src/main.tsx
- Optional external open: a Rust command using the tauri_plugin_opener Rust API (opener:default lacks open-path) or a scoped opener:allow-open-path capability in src-tauri/capabilities/default.json
- Docs: TERAX.md (tab kinds and module list), docs/architecture/two-process-model.md (new commands), docs/architecture/security-model.md (new protocol boundary)

## risks
- Tauri 2.12 is_local_url treats any user-registered custom scheme origin as LOCAL, so app commands need no ACL from it [upstream]. Isolation of the preview frame then relies on main-frame-only init scripts plus the per-launch invoke key. Never inject Tauri scripts into these frames and never navigate the main frame to terax-preview://. Use a sandbox without allow-same-origin so the Origin is null and ipc:// rejects it
- srcdoc combined with allow-same-origin (or with no sandbox) shares the app origin and exposes parent.__TAURI_INTERNALS__, which is full IPC: PTY, fs and shell
- The asset protocol scope is ['**'] (tauri.conf.json:31-34) and its responses carry no CSP [upstream]. Previewing HTML through asset:// with allow-same-origin would let page scripts read any file, ~/.ssh included, and exfiltrate it. This is also a pre-existing broad file-read surface if the main webview ever has XSS
- The Rust side has no secret deny-list today (fs_read_file, file.rs:57-103). The new protocol handler must implement canonicalize plus starts_with(root), reject .. and NUL, and port the security.ts patterns, or a previewed page can read .env or keys under its root
- tauri_utils MimeType falls back to text/html for unknown extensions [upstream]. Serving arbitrary files that way enables content sniffing into HTML. Use our own table with an octet-stream fallback and X-Content-Type-Options: nosniff
- Live-reload gaps: watches are NonRecursive (watch.rs:186), so asset changes in subdirectories are missed; SKIP_DIRS (19-84) excludes dist, build and out, so built sites never get fs:changed; events carry canonical paths while tabs hold raw paths; one save yields both fs:file-written and fs:changed, so reloads must be coalesced
- Without allow-same-origin, localStorage, sessionStorage, cookies and IndexedDB throw inside the preview, and pages that rely on storage break. Adding allow-same-origin raises IPC exposure to depend solely on the invoke key
- Untrusted HTML with scripts on can still exfiltrate data within its root through navigation or popups even with a response CSP connect-src 'self'. Scope and deny-list are the real controls
- Remount-on-reload (key nonce) resets scroll position, and WKWebView may cache custom-scheme responses unless Cache-Control: no-store is sent
- Existing dedupe bug: markdown tabs flipped to Raw become kind editor, and planMarkdownTabOpen (useTabs.ts:174-203) then opens a duplicate tab. Mirroring the pattern for HTML copies the bug unless dedupe spans all kinds
- Video seeking from a custom scheme in WKWebView needs Range support. Without it, media inside previews may not seek [inferred]
- A token longer than 63 chars as a host label may be rejected; generate_token in control.rs produces 64 hex chars, so use 16 random bytes (32 hex chars)
- Option (d), a loopback server, exposes files to other local processes and to browser tabs that guess the port, unless it uses a random path token, a Host header check and connection caps. It also adds HTTP parsing surface

## openQuestions
- Default open mode for .html: code with a Preview toggle (recommended), rendered by default like markdown, or remember per file?
- UX shape: mirror markdown with a separate 'html' tab kind that replaces the code view (Option A), or keep an editor tab with Code/Split/Preview (Option B, keeps the undo buffer and allows a side-by-side live preview)?
- Serve root: the HTML file's own directory (default), the explorer or workspace root when the file sits inside it (needed for root-relative /assets links), or user-selectable?
- Scripts in the preview: enabled by default (dev-friendly) or opt-in per tab? Should a 'trusted' toggle add allow-same-origin to enable localStorage and cookies?
- Network egress from previewed pages: block with a response CSP (connect-src 'self', no CDN scripts) or allow (CDN Bootstrap and similar keep working)?
- Should the preview show unsaved buffer content (needs an in-memory overlay command in Rust) or only saved disk content (simpler, matches markdown)?
- Is 'Open in system browser' for local HTML required? It needs a Rust opener command or an opener:allow-open-path capability. If it is critical, does that tip the choice toward the 127.0.0.1 server (option d)?
- Should the same live-reload hook also fix markdown's missing live reload, and should the md duplicate-tab dedupe bug be fixed in the same change?
- Should the pre-existing assetProtocol scope ['**'] be narrowed, for example to $HOME/** with denies for .ssh, .aws and similar, as a separate hardening task?
- Forward-looking for the SSH feature: should the preview root abstraction be designed so the protocol handler can later read from a remote (SFTP) backend instead of the local fs?
