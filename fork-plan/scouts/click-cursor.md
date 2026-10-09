# click-cursor

## existingSupport
Verified in code:
- **Block tabs** are opt-in per tab: Cmd+Shift+T, command palette "New block terminal", or the header button. There is no global default. Their shared command bar is a CodeMirror 6 EditorView (shellEditor.ts:357-454), so clicking to place the caret, drag-select, word jumps (Option+Arrows), Cmd+Arrows and undo/redo already work there.
- **When the command bar appears:** it activates after OSC 133 B or C. That covers zsh, bash 4.4 and newer, fish and pwsh. bash older than 4.4 (macOS /bin/bash 3.2) stays in plain mode via `terax_blocks=0`.
- **Classic tabs:** no click-to-move exists. A plain click focuses the terminal, makes and then discards a provisional selection pin, or opens a link. Plain-click link opening also applies to plain-text URLs.
- **Ghostty core:** the adapted core (terax.zig, TeraxGhostty.ts) has no prompt click-to-move. It does not export the input-start position (the B event has no cursor data). Semantic marker pins are captured only on C and D, and only in block sessions. The JS side does not use `restty_active_cursor_x`.
- **B marker position:** OSC 133 B sits at the start of PS1 for classic zsh and bash. Fish and PowerShell put it at the end.
- **Mouse reporting for TUI apps:** works for DECSET 9/1000/1002/1003 with SGR 1006 or legacy encoding (GhosttyInputController.ts:404-540). Missing: 1005, 1015, 1016, horizontal wheel and alternate scroll (1007).
- **Readline key mappings present** (keymap.ts): Option+Left/Right word jumps, Cmd+Left/Right line start/end, Cmd+Backspace (^U), Option+Backspace (^W). They are skipped on the alternate screen.
- **Readline key mappings missing:** Cmd+Z undo (^_), Option and Cmd forward-delete, and Home/End mapped to line start/end.

## report
## Scope

This maps how terminal input works today and where a mouse click could move the text cursor. "Verified" means I read the code. "Inferred" means it is reasoning or knowledge of upstream projects that I could not check against source in this repo.

---

## 1. Block mode (shared command bar): click-to-place caret already works

**Editor.** The command bar is CodeMirror 6, not a textarea (verified). `createShellEditor` builds an `EditorView` in `src/modules/terminal/block/lib/shellEditor.ts:357-454`. Its extensions are:
- `history()`
- `drawSelection`, `rectangularSelection()`, `crosshairCursor()`
- `EditorView.lineWrapping`
- `autocompletion`, inline suggestion and history popover
- Enter / Shift-Enter / Tab / Ctrl-C keymaps (`submitKeys`, lines 367-399)
- `keymap.of([...defaultKeymap, ...historyKeymap])` (line 436)

CodeMirror's own mouse handling places the caret on click, selects on drag, selects a word on double-click and makes a rectangular selection on Alt-drag. Nothing in the repo overrides this: `block.css` only styles tooltips, and there is no `domEventHandlers` or `mousedown` override in `block/lib`. So **click-to-place caret already works in block tabs.** The caret behaviour itself is standard CodeMirror (inferred from CM6 defaults, not tested here).

**macOS keys inside the editor** (verified in `node_modules/@codemirror/commands/dist/index.js:1745-1769`):

| Keys | Action |
|---|---|
| Option+Left/Right | word |
| Cmd+Left/Right | line boundary |
| Cmd+Backspace / Cmd+Delete | delete to line boundary |
| Option+Backspace / Option+Delete | delete word |
| Cmd+Z / Cmd+Shift+Z | undo / redo (lines 570-571) |
| Ctrl-A/E/K and other emacs keys | emacs-style bindings |

Cmd+Up/Down never reach the editor. They are taken in the capture phase by `blocks.prev`/`blocks.next` (`src/modules/shortcuts/shortcuts.ts:208-220`, `src/modules/shortcuts/lib/useGlobalShortcuts.ts:43`, guard in `src/app/App.tsx:1012-1018`).

**Where it is mounted.**
- `src/app/components/WorkspaceInputBar.tsx:151-173` lazy-loads `ShellInput` (`block/ShellInput.tsx`).
- It renders only when `isBlockTab && blockMode !== "plain"`. When the mode is `plain`, it shows the button "Type directly in the terminal" instead.
- One editor instance is retargeted per leaf. `ShellInput.tsx:119-147` registers `setLeafInputFocus`, `setLeafInputKeyDown` and `setLeafInputPaste` and swaps drafts.

**When it activates (verified).**
- `useBlockController` (`lib/blockController.ts:18-40`) reads `getLeafBlockMode`, which comes from `GhosttyBlockSession` (`ghostty/ghosttyBlockSessions.ts:69-77`), which comes from `GhosttyBlocks.mode` (`ghostty/GhosttyBlocks.ts:58-61`).
- The mode is `prompt` only after an OSC 133 `prompt-end` (B) or `end-of-input` (C) sets `integrated` (`GhosttyBlocks.ts:80-89`). It is never `prompt` when B carried `terax_blocks=0` (`blockInputDisabled`), and it becomes `alt` on the alternate screen.

**Shells (verified).**
- The PTY gets `TERAX_BLOCKS=1` (`src-tauri/src/modules/pty/shell_init.rs:154-156`).
- zsh: `zshrc.zsh:50-61`, prompt replaced by B only.
- bash 4.4 and newer: `bashrc.bash:55-62`.
- fish: `init.fish:85-92`.
- PowerShell: `profile.ps1:99-102`.
- bash older than 4.4 (macOS `/bin/bash` 3.2) emits `133;B;terax_blocks=0` (`bashrc.bash:27-32`), so the tab stays in `plain` mode with direct terminal input.

**Enabled by default? No (verified).**
- Normal tabs (`tab.new`, Cmd+T) are classic.
- A block tab is opt-in per tab:
  - "New Blocks terminal", Cmd+Shift+T (`shortcuts.ts:108-111`)
  - command palette `tab.newBlock` (`command-palette/commands.ts:148-155`)
  - header `onNewBlock` (`App.tsx:1391`)
  - `useTabs.newBlockTab` (`modules/tabs/lib/useTabs.ts:750-768`)
- The flag persists per space (`spaces/lib/serialize.ts:77,164`).
- Block tabs cannot be split (`useTabs.ts:1295`).
- There is no "blocks by default" setting: no block key in `settings/store.ts` or `GeneralSection.tsx`.

**Click on the terminal grid in a block tab (verified).**
- `TerminalPane.tsx:113-156`: an unmodified single click of 4px or less with no grid selection calls `GhosttyBlocks.selectAtLine` (block selection, `GhosttyBlocks.ts:243-255`), then focuses the editor.
- The surface `pointerdown` handler also calls `onRequestFocus`, then `focusSessionSurface`, which focuses the editor in prompt mode (`useGhosttyTerminalSession.ts:883-890`).

---

## 2. Classic / bare mode today: a click never moves the shell cursor (verified)

Pointer stack on the surface root, the same in both renderers.

**WebGPU** (`ghostty/gpu/WebGpuTerminalSurface.ts`):
- `TerminalSelectionController` created at 213-223.
- Listeners at 250-254: `pointerdown` for focus, `mousemove`/`mousedown`/`mouseup`/`mouseleave` for links.
- Link activation at 615-630: a plain left click opens the link, no modifier required.
- Hover detection at 654-684 calls `model.hyperlinkAtViewportCell`. That also detects plain-text URLs on demand (`AdaptedGhosttyTerminalModel.ts:373-435`).

**WebGL**: identical (`webgl/WebGlTerminalSurface.ts:157-167, 194-198, 486-516`).

**`TerminalSelectionController`** (`ghostty/selection/TerminalSelectionController.ts`):
- `pointerdown` (130-166): if `shouldStartTerminalSelection` passes (left button, not the scrollbar, and either no mouse tracking or Shift held; 308-322), it calls `preventDefault` and `stopPropagation`, captures the pointer and sets a provisional native pin.
  - `meaningful = shiftKey || mode !== "character"` (line 163).
  - `event.detail` picks the mode: 2 is word, 3 or more is line (151-152).
  - `altKey` gives a rectangular selection (161).
- `pointerup` (177-192): if the selection never became meaningful (an unmoved click), it clears the pin. **This is the exact "unmoved click" seam.**

**`TerminalNativeSelection`** (`ghostty/input/TerminalNativeSelection.ts:38-57`) handles right-click and Ctrl+click on macOS for the native context menu.

**`GhosttyInputController`** (`ghostty/input/GhosttyInputController.ts`) is built in `useGhosttyTerminalSession.ts:833-876` with `pointerTarget = surface.eventTarget()`. It handles keys, mouse reporting and wheel. It has no click-to-cursor code.

**Net result today:** a plain click focuses the terminal, makes and then discards a provisional selection pin, or opens a link. Option+click without a drag is a no-op. That makes it a free gesture.

### OSC 133 surfacing to JS (verified)

Parser side, `packages/ghostty-core/adapted/wasm/src/terax.zig:812-839`:
- A / fresh-line is emitted as `prompt_start` (no payload).
- B is emitted as `prompt_end` with a 1-byte `blockInput` flag. **No cursor position** is included.
- C is emitted as `end_of_input` with a semantic marker id and the command.
- D is emitted as `end_of_command` with a marker and the exit code.
- Markers (tracked pins, ring of 2,048) are captured only on C and D, and only when enabled (`SemanticMarkers.capture`, 883-898).
- They are enabled only by `GhosttyBlocks` (`GhosttyBlocks.ts:55`), so only in block sessions.

JS side:
- Decoding: `packages/ghostty-core/lib/adapted/TeraxGhostty.ts:1198-1257` (prompt-end at 1234-1237).
- Types: `lib/types.ts:589-591`.
- Events drain **after the whole PTY chunk is parsed** (`AdaptedGhosttyTerminalModel.ts:213-231` `write`, then `drainEvents` at 870-876). A cursor read inside an event handler therefore sees the end-of-chunk cursor, not the cursor at the B position.
- Every session, block or classic, routes events in `useGhosttyTerminalSession.ts:672-678`:
  - `GhosttySemanticEventRouter` (`core/GhosttySemanticEventRouter.ts:47-63` tracks prompt-start, prompt-end, end-of-input and end-of-command).
  - `GhosttyBlocks.handle`, which only exists for block tabs.
  - `PromptPresentationGate` suppresses render notifications between A and B (`core/PromptPresentationGate.ts`).
- **Classic tabs have no prompt-phase tracker** other than the router's private `inCommand` flag.

**Where B sits in the prompt (verified, important for classic mode):**

| Shell | B position | Source |
|---|---|---|
| zsh, classic | **start** of PS1 | `zshrc.zsh:62-65` |
| bash, classic | **start** of PS1 | `bashrc.bash:63-66` |
| fish | end of the prompt | `init.fish:100` |
| PowerShell | end of the prompt | `profile.ps1:112` |

- A is printed in precmd just before PS1 (`zshrc.zsh:66`, `bashrc.bash:67`).
- So for zsh and bash, B does **not** mark where input starts.
- The test `ghostty/shellBlockCompatibility.test.ts:29` pins B before `test> ` for bash 3.2.
- PowerShell and fish already follow the OSC 133 spec.

### Ghostty core: no click-to-move logic (verified absence)

- No matches for `click_events`, `cl=`, `promptPath`, `promptClick`, `semantic_content` or `click_move` anywhere in the repo (wasm excluded).
- The list of `pub export fn` in `terax.zig` has no prompt or click export.
- The upstream Ghostty source is not vendored: `build.zig.zon` pins commit `f426f6f` (1.3.2-dev) by URL, and there is no local copy.
- **Zig is not installed** on this machine (`which zig` returns nothing). Rebuilding the WASM (`adapted/UPSTREAM.md` "Rebuild") needs Zig 0.16.
- The official upstream ABI enum has `CursorAtPrompt = 39` (`packages/ghostty-core/lib/official/types.ts:51`), but nothing uses it.

Inferred:
- Upstream Ghostty's `cursor-click-to-move` lives in the app-level `Surface.zig` (`clickMoveCursor`, using `Screen.promptPath` and per-row/per-cell semantic prompt state). It turns the click into synthetic arrow keys, triggered by Option/Alt+click.
- The kitty / fish 4.1 `OSC 133;A;click_events=1` protocol is not handled. Terax also disables fish's own markers (`fish_features=no-mark-prompt`).

### Model APIs a JS-only implementation can use (verified)

Usable today:
- `model.cursor()` gives the viewport x/y from render state (`AdaptedGhosttyTerminalModel.ts:336-346`). Zig sets the viewport cursor to (0,0) / not visible when the cursor is outside the viewport (`terax.zig:2161-2170`), so callers must require `scrollPosition().offset === 0`.
- `bufferCursorLine()` (539-542, from `TeraxGhostty.ts:469-473`).
- `bufferLineAtViewportRow` (490-496).
- `encodeKey` (635-638), which handles DECCKM and Kitty keyboard flags natively.
- `mode(n)` (640-643) and `modes()` (645-649; the bits are in `terax.zig:1562-1574`).
- `hyperlinkAtViewportCell` (373).
- `readCellLine` (535-537): **drops the `wrapped` flag** and scrolls the viewport through `readBufferLines` (813-862), so it is expensive.

Not public yet:
- Row-wrap continuation flags exist only internally: `state.rowWrapped[row]` holds Ghostty `wrap_continuation` (`terax.zig:2056`), and `snapshotRow()` (`AdaptedGhosttyTerminalModel.ts:979-998`) already returns `{cells, wrapped}`, with `""` for wide-char spacer cells.
- `restty_active_cursor_x` is exported by the WASM (`terax.zig:2376-2379`) but **not wired in JS**: it is absent from the required-export list starting at `TeraxGhostty.ts:59` and from `adapted/types.ts:122`. Wiring it needs no WASM rebuild.

---

## 3. Full-screen TUI apps: mouse tracking (verified)

- **Mode detection:** `mouseTracking` is true for DECSET 9, 1000, 1002 or 1003 (`terax.zig:1568-1571`, decoded in `TeraxGhostty.ts:527-540` and `AdaptedGhosttyTerminalModel.ts:1099-1102`).
- **Encoding, in JS** (`GhosttyInputController.ts`):
  - `handleMouseDown` / `handleMouseUp` (404-422) only while tracking is on and Shift is not held.
  - `handleMouseMove` (424-454) for 1002 with a button held, or 1003 for any motion; deduplicated per cell.
  - `handleWheel` (456-469) sends button 64/65.
  - `sendMouse` (513-540) writes SGR 1006 `CSI < b;x;y M/m`, or legacy X10 bytes clamped to 255.
  - Modifiers: Shift 4, Alt 8, Ctrl 16 (`terminalMouseModifiers`, 607-615).
  - Output goes to `onData`, then `session.writer.enqueue` (`useGhosttyTerminalSession.ts:852-859`).
- **Selection stays out of the way:** while tracking is on, the selection controller does not start (no `preventDefault` on pointerdown), so `mousedown`/`mouseup` events still reach the input controller. Shift+drag still selects (`shouldStartTerminalSelection`).
- **Result:** vim, htop and micro with mouse support do receive clicks.
- **Gaps:**
  - No 1005, 1015 or 1016 encodings.
  - Mode 9 (X10) still sends releases.
  - Legacy encoding drops modifiers on release.
  - No horizontal wheel (66/67).
  - Drags are not pointer-captured, so motion stops outside the canvas.
  - **No alternate-scroll (DECSET 1007).** On the alternate screen without tracking (`less`, `man`, git pager), the wheel does nothing: `handleWheel` returns when `history === 0` (line 471), and `scrollTo` forces offset 0 on alt-screen (`AdaptedGhosttyTerminalModel.ts:456-458`). There are no 1007 references in the repo.

---

## 4. Interaction conflicts (verified unless marked)

- **Click vs drag:** use `TerminalSelectionController`'s "not meaningful" pointerup (177-192) as the click signal. Double and triple clicks start with a detail-1 down/up pair. Inferred: `PointerEvent.detail` is populated by WebKit; on Chromium/WebView2 it is 0, so gate on `detail <= 1`.
- **Click vs link:** the plain-click link opener runs on `mouseup`, which fires after `pointerup`. It also matches **plain-text URLs typed in the command line**, so clicking inside `curl https://...` while editing opens the browser today. Click-to-move must check `hyperlinkAtViewportCell` and pick a precedence (suggestion: inside the active input, cursor move wins and Cmd+click opens links, as in iTerm2 and VS Code).
- **Compat mouse events (inferred):** the selection controller calls `preventDefault` on `pointerdown`. In Chromium that suppresses `mousedown`/`mouseup`, so new click logic should use pointer events, not mouse compat events.
- **Click vs block selection:** in block tabs a plain grid click selects a block (`TerminalPane.tsx:128-156`). Classic click-to-move should no-op whenever `ghosttyBlocks(leaf)?.getMode() === "prompt"` (the editor owns input). PTY bytes are already dropped in that mode (`useGhosttyTerminalSession.ts:853-857`).
- **Modifiers in use:**
  - Shift: extends selection, or bypasses app mouse tracking.
  - Alt: rectangular drag.
  - Ctrl (macOS) and right-click: native menu.
  - Unmoved Option+click and unmoved plain click are both unused. iTerm2 uses Option+click; Ghostty uses Option/Alt+click (inferred).

### Keyboard shortcuts

Present today (`src/modules/terminal/lib/keymap.ts`):

| Keys | Sends | Line |
|---|---|---|
| Option+Left/Right | `ESC b` / `ESC f` | 8-13 |
| Cmd+Left/Right (macOS) | `^A` / `^E` | 17-28 |
| Cmd+Backspace | `^U` | 39 |
| Option+Backspace | `^W` | 40 |
| Ctrl+Backspace (non-mac) | `^W` | 43 |

- These are skipped on the alternate screen (`terminalReadlineSequence`, 47-57) and applied in `GhosttyInputController.handleKeyDown` (250-258), before the native-menu passthrough for other Cmd keys (260).
- Shift+Enter sends `ESC CR` (261-271).
- zsh rebinds `\ef` to `emacs-forward-word` only when the stock binding is still in place (`zshrc.zsh:87-90`).

Missing:
- Cmd+Z: undo, send `\x1f` (Ctrl+_), which works in readline, zle and fish.
- Fn+Option+Delete: forward kill-word, send `ESC d`.
- Fn+Cmd+Delete: kill to end of line, send `^K`.
- Home/End (Fn+Left/Right): could map to `^A`/`^E` on the primary screen.
- Inferred: zsh's stock `^U` is `kill-whole-line`, not kill-to-start, so Cmd+Backspace deletes the whole line in zsh. A guarded `bindkey '^U' backward-kill-line` in `zshrc.zsh`, following the 87-90 pattern, would fix it.
- Redo has no portable sequence.

---

## 5. Tests and patterns (verified)

- Vitest runs in the node environment (no jsdom) and uses fake DOM objects.
- `src/modules/terminal/lib/keymap.test.ts`: pure functions with an `evt()` helper.
- `ghostty/input/GhosttyInputController.test.ts`:
  - fakes: `FakeElement`, `FakeTextArea`, `keyboardEvent()`, `mouseEvent()` (502-512), `inputModel()` (441-460)
  - real-WASM tests through `TeraxGhostty.loadBytes(readFile(.../adapted/ghostty-vt.wasm))` (29-80)
  - mouse dedup test at 411-436
- `ghostty/selection/TerminalSelectionController.test.ts`:
  - `dragFixture()` (247-290) with a fake target plus pointer capture, or a real `AdaptedGhosttyTerminalModel` (107-145)
  - click versus drag at 147-171
- Also relevant: `ghostty/input/TerminalNativeSelection.test.ts`, `GhosttyBlocks.test.ts` (writes `\x1b]133;B\x07` against the real model, 112-116), `core/GhosttySemanticEventRouter.test.ts`, `core/PromptPresentationGate.test.ts`, `shellBlockCompatibility.test.ts` (spawns a real bash against `bashrc.bash`).
- Rust script tests: `src-tauri/src/modules/pty/shell_init.rs:430-480`.

---

## 6. Recommended implementation

### A. Block mode
Nothing is needed for clicking. Optional: add a preference "open new terminals as block terminals", which would give a full editor everywhere.

### B. Classic mode, v1 (JavaScript only, no WASM rebuild)

1. **Pure core** in a new `src/modules/terminal/ghostty/input/promptClickMove.ts`:
   - Signature: `promptClickDelta({rows, cols, cursor:{row,col}, click:{row,col}, line:(row)=>{cells:readonly string[]; wrapped:boolean}}): number | null`.
   - Group: extend from the cursor row upward while `line(r).wrapped`, and downward while `line(r+1).wrapped` (wrapped means continuation of the previous row).
   - If the click is outside that soft-wrapped group, return null. This covers v1: single logical line, which includes long wrapped commands. Multi-line buffers can come later.
   - Linearize as `(r - top) * cols + c`.
   - Clamp the target to `max(cursor, last non-blank, non-spacer cell + 1)`.
   - If the click lands on a spacer cell (`""`), snap back to its wide head.
   - Count non-spacer cells between cursor and target; return a signed delta.
   - Overshooting left into the prompt text is harmless: the line editor stops at buffer start, and bells are not surfaced because `onBell` is not wired (`useGhosttyTerminalSession.ts:639-656`).
   - Encoding helper: repeat `model.encodeKey({action: KeyAction.PRESS, key: Key.LEFT or Key.RIGHT, mods: Mods.NONE})` and concatenate. This respects DECCKM and Kitty flags. Cap the total (the input writer limit is 256 KiB, line 50).
2. **Model accessor:** add `viewportLine(row)` to `AdaptedGhosttyTerminalModel`, returning `snapshotRow(this.terminal, this.ensureRenderState(), row)` (979-998). Add it as an optional method on `GhosttyTerminalModelApi` (`GhosttyTerminalModel.ts:50-109`). This avoids the viewport-scrolling `readBufferLines`.
3. **Prompt phase tracker:** a pure class, for example `core/PromptInputTracker.ts`.
   - `prompt-start` sets phase `prompt`; `prompt-end` sets `input`; `end-of-input` sets `running`; `end-of-command` sets `idle`; `screen-cleared` and `overflow` reset.
   - Store it on `GhosttySession` (type at 61-86).
   - Feed it in the `onEvent` handler (672-678) and reset it in `respawnGhosttySession` (414-444).
   - This works for zsh and bash even though B sits at the start of PS1, because B still comes before any input.
4. **Click seam:**
   - Add `onClick?(point:{line,column,viewportRow}, event)` to `TerminalSelectionControllerOptions` (19-25). Call it in `handlePointerUp` when the selection is not meaningful, the button is 0 and `detail <= 1`.
   - Forward it through a new `onCellClick` on `WebGpuTerminalSurfaceOptions` (62-74) and `WebGlTerminalSurfaceOptions` (42-55), plus their selection construction (213-223 and 157-167).
   - Wire it in `surfaceBaseOptions` (`useGhosttyTerminalSession.ts:697-712`), which is shared by the fallback surface.
   - Alternative with no surface changes: a `pointerdown`/`pointerup` listener in `GhosttyInputController` that checks `model.trackedSelection?.() === null`. This relies on the selection controller's listener being registered first: the surface is created at 729/735, before `createGhosttyInput` at 762. That ordering is fragile.
5. **Handler gates:**
   - pass: phase `input`, primary screen, mouse tracking off, `scrollPosition().offset === 0`, no hyperlink at the cell (or Cmd not held, depending on the chosen precedence)
   - no-op: block mode is `prompt`, or IME composition is active
   - Then write through `session.writer.enqueue`.
   - Option+click fallback with no integration required (SSH, REPLs, `read`): same algorithm, ignore phase, keep the cursor-row group restriction.
6. **Preference:** `terminalClickToMoveCursor` in `src/modules/settings/store.ts`, following the pattern at 160-169, 256-265, 348-357, 505-525 and 1004. Add a switch in `src/settings/sections/GeneralSection.tsx` near 269-286. Read it with `usePreferencesStore.getState()` at click time.

### C. Classic mode hardening (v2)
- **Move B to the end of PS1** in `zshrc.zsh:62-65` and `bashrc.bash:63-66`. Update `shellBlockCompatibility.test.ts:29`. This lengthens the `PromptPresentationGate` window (it has a 1 s watchdog); verify with p10k and starship.
- **Exact input start:** extend the Zig `prompt_end` payload (`terax.zig:816-819`) with the cursor x/y, or a tracked pin, captured at parse time. Decode it in `TeraxGhostty.ts:1234-1237`. This enables multi-row and hard-newline input with a precise left bound, and survives redraws, chunk splits and reflow. It needs Zig 0.16 and a rebuild of both SIMD and scalar artifacts, plus UPSTREAM checksum updates.
- **JS-only partial alternative:** wire `restty_active_cursor_x` (no rebuild) and record the cursor at the `prompt-end` drain. This is only correct at chunk end and breaks on zle or fish prompt redraws.

### D. TUI apps
Mouse reporting already works. Recommended additions:
- DECSET 1007 alternate scroll in `handleWheel` (`GhosttyInputController.ts:456-487`): when on the alternate screen with tracking off, send `encodeKey` Up/Down. Inferred: Ghostty's default for `mouse_alternate_scroll` is on; check with `model.mode(1007)`.
- Optional: horizontal wheel (66/67) and SGR-pixel encoding.

### Suggested task slices

| Task | Model | Work |
|---|---|---|
| T1 | Sonnet | Pure `promptClickMove` plus tests (fake grid and real WASM) |
| T2 | Sonnet | `viewportLine` model API plus tests |
| T3 | Sonnet | `PromptInputTracker` plus session wiring plus tests |
| T4 | Sonnet | Click seam (selection controller, both surfaces, session handler, link precedence) plus `dragFixture` tests |
| T5 | Haiku | Preference and Settings switch |
| T6 | Haiku | keymap additions plus `keymap.test.ts`; guarded zsh `^U` rebind |
| T7 | Sonnet | Alternate-scroll 1007 plus tests |
| T8 (optional) | Haiku | B-at-end of PS1 in scripts plus test update |
| T9 (optional, needs Zig) | Opus | Zig `prompt_end` payload |

## keyFiles
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/block/lib/shellEditor.ts: CodeMirror 6 block command editor (extensions 401-451, submit keys 367-399); click-to-caret already native here
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/block/ShellInput.tsx: Single shared editor retargeted per leaf; registers focus/keydown/paste hooks (119-147)
- /Users/frider/Documents/iamcoder/terax_claude/src/app/components/WorkspaceInputBar.tsx: Mounts ShellInput only for block tabs when blockMode != plain (151-173)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/GhosttyBlocks.ts: Block mode state machine from OSC 133 events (58-61, 79-126); enables semantic markers; selectAtLine (243-255)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/selection/TerminalSelectionController.ts: Pointer selection; unmoved-click detection in handlePointerUp (177-192) is the click-to-move seam; Alt=rectangular, detail=word/line
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/input/GhosttyInputController.ts: Key/IME/paste/mouse-report/wheel encoding; readline mapping hook (250-258); mouse tracking (404-540); wheel/alt-screen gap (456-487)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/lib/keymap.ts: macOS readline-style shortcut mappings; place for missing Cmd+Z, forward-delete, Home/End mappings
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/useGhosttyTerminalSession.ts: Session wiring: semantic event router (639-656), onEvent fan-out (672-678), surfaceBaseOptions (697-712), PTY write (796-802), createGhosttyInput + block-prompt byte filter (833-876), focus routing (883-906)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/gpu/WebGpuTerminalSurface.ts: Creates selection controller (213-223), focus pointerdown (606-609), plain-click link activation on mouseup (615-630), hover link detection (654-684), options type (62-74)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/webgl/WebGlTerminalSurface.ts: Same pointer/link/selection wiring for WebGL fallback (42-55, 157-167, 194-198, 486-516)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/AdaptedGhosttyTerminalModel.ts: Model API: write+drainEvents ordering (213-231, 870-876), cursor() (336-346), hyperlinkAtViewportCell incl. plain URL detection (373-435), alt-screen scroll clamp (453-468), bufferCursorLine (539-542), encodeKey/mode/modes (635-649), snapshotRow with wrapped flag (979-998)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/GhosttyTerminalModel.ts: GhosttyTerminalModelApi interface where a viewportLine()/bufferCursorColumn() accessor would be added (50-109)
- /Users/frider/Documents/iamcoder/terax_claude/packages/ghostty-core/adapted/wasm/src/terax.zig: OSC 133 handling (812-839; prompt_end payload has no cursor), SemanticMarkers (862-910), mode bits (1562-1574), row wrap_continuation (2056), cursor exports (2161-2170, 2376-2384); no click-to-move code
- /Users/frider/Documents/iamcoder/terax_claude/packages/ghostty-core/lib/adapted/TeraxGhostty.ts: JS bridge: required exports list (59+; restty_active_cursor_x missing), bufferCursorLine (469-473), modes (527-540), event decoding (1198-1257)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/scripts/zshrc.zsh: Classic zsh puts OSC 133 B at START of PS1 (62-65); block-mode PS1 (50-61); zle word rebind pattern (87-90)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/scripts/bashrc.bash: Classic bash B at START of PS1 (63-66); bash<4.4 terax_blocks=0 (27-32)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/scripts/init.fish: fish emits B after prompt (100) and re-emits D/A/B on every fish_prompt repaint
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/TerminalPane.tsx: Block-tab grid click => selectAtLine + editor focus (113-156)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/core/PromptPresentationGate.ts: Suppresses rendering between A and B; affected if B moves to end of PS1
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/selection/TerminalSelectionController.test.ts: dragFixture (247-290) and real-model click/drag tests to extend for onClick
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/input/GhosttyInputController.test.ts: Fake DOM + real WASM test patterns for input/mouse encoding
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/lib/keymap.test.ts: Pure keymap test pattern for new shortcuts
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/shellBlockCompatibility.test.ts: Pins B-before-prompt for bash 3.2 (line 29); must change if B moves to end of PS1
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/settings/store.ts: Terminal preference pattern (160-169, 256-265, 348-357, 505-525, 1004) for a click-to-move toggle
- /Users/frider/Documents/iamcoder/terax_claude/src/settings/sections/GeneralSection.tsx: Terminal Switch rows (269-286) for the new setting

## integrationPoints
- TerminalSelectionController.handlePointerUp (src/modules/terminal/ghostty/selection/TerminalSelectionController.ts:177-192): call a new options.onClick(point, event) when !this.meaningful, button 0, detail <= 1. Add onClick to TerminalSelectionControllerOptions (19-25).
- WebGpuTerminalSurface constructor selection setup (gpu/WebGpuTerminalSurface.ts:213-223) and WebGlTerminalSurface (webgl/WebGlTerminalSurface.ts:157-167): forward a new onCellClick option. Add it to WebGpuTerminalSurfaceOptions (62-74) and WebGlTerminalSurfaceOptions (42-55).
- useGhosttyTerminalSession.ts surfaceBaseOptions (697-712): add onCellClick, which calls handlePromptClick(session, click). It writes via session.writer.enqueue and no-ops when ghosttyBlocks(leaf)?.getMode() === 'prompt'.
- useGhosttyTerminalSession.ts onEvent (672-678): feed a new PromptInputTracker stored on GhosttySession (type 61-86). Reset it in respawnGhosttySession (414-444) and on model replacement.
- AdaptedGhosttyTerminalModel: add public viewportLine(row) returning snapshotRow(this.terminal, this.ensureRenderState(), row) (snapshotRow at 979-998). Declare it as optional in GhosttyTerminalModelApi (GhosttyTerminalModel.ts:50-109).
- New pure module src/modules/terminal/ghostty/input/promptClickMove.ts: promptClickDelta(...) plus an arrow encoder using model.encodeKey({action: KeyAction.PRESS, key: Key.LEFT/RIGHT, mods: Mods.NONE}). This respects DECCKM and Kitty flags.
- Link precedence: check model.hyperlinkAtViewportCell(row, col) in the click handler. Optionally gate the link opener on metaKey inside the active prompt, in WebGpuTerminalSurface.handleLinkMouseUp (615-630) and the WebGL equivalent (502-511).
- keymap.ts terminalReadlineSequence (47-57): add Cmd+Z -> \x1f, Option+Delete(forward) -> \x1bd, Cmd+Delete(forward) -> \x0b, and optionally Home/End -> ^A/^E. Add tests in keymap.test.ts.
- GhosttyInputController.handleWheel (456-487): add alternate-scroll (DECSET 1007). On the alternate screen with mouse tracking off, send encodeKey Up/Down instead of returning at history === 0.
- Optional hardening: zshrc.zsh:62-65 and bashrc.bash:63-66 move OSC 133 B to the end of PS1. Update shellBlockCompatibility.test.ts:29.
- Optional (needs Zig 0.16): in terax.zig:816-819, append the input-start cursor or a tracked pin to the prompt_end payload. Decode it in TeraxGhostty.ts:1234-1237 and extend the GhosttyTerminalEvent prompt-end type (lib/types.ts:591).
- JS-only alternative accessor: wire the WASM export restty_active_cursor_x into TeraxGhostty.ts requiredExports (59+) and adapted/types.ts (~122) as bufferCursorColumn(). No rebuild needed.
- Settings: add terminalClickToMoveCursor in settings/store.ts and a Switch in settings/sections/GeneralSection.tsx (~269-286). Read it at click time via usePreferencesStore.getState().

## risks
- Autosuggestions (zsh-autosuggestions, fish) render grey text that looks like ordinary cells. Clicking inside it sends Right arrows at the end of the buffer, which accepts the suggestion. The grid cannot tell typed text from suggestion text.
- Events drain after the whole PTY chunk is parsed (AdaptedGhosttyTerminalModel.ts:213-226). Any 'record input start at prompt-end' JS approach is only correct at chunk end. It breaks on prompt redraws (zle reset-prompt, p10k async, fish repaint, which re-emits D/A/B) and on split chunks. A precise input start needs the Zig payload change.
- Zig is not installed on this machine. Rebuilding the adapted WASM needs Zig 0.16 plus SIMD and scalar artifacts, checksum updates in adapted/UPSTREAM.md, and the Rust ghostty_artifacts test.
- Moving OSC 133 B to the end of PS1 widens the PromptPresentationGate render-suppression window (it has a 1 s watchdog) and breaks the expectation in shellBlockCompatibility.test.ts:29. It needs validation with p10k, starship and transient prompts.
- Plain-click links currently also match plain-text URLs typed in the command line, so click-to-move and the link opener conflict unless precedence changes.
- The selection controller calls preventDefault on pointerdown, which may suppress mousedown/mouseup compat events on Chromium/WebView2. Implement on pointer events. PointerEvent.detail may be 0 on Chromium, so gate single clicks with detail <= 1.
- Character counting is approximate. One arrow per grapheme cluster cell, but zle may move by code point for ZWJ or combining sequences. Pending-wrap cursor at the last column can be off by one. Type-ahead echo that has not arrived yet skews the delta.
- Kitty keyboard modes with event-type reporting may expect key releases after the synthetic PRESS events.
- Multi-line buffers (Shift+Enter inserts ESC CR, which zsh treats as a newline in the buffer) and PS2 continuation rows are excluded in v1. Using Up/Down to cross rows could trigger history navigation.
- Remote shells over SSH and REPLs emit no OSC 133, so plain-click is disabled there. Only the Option+click fallback applies.
- Performance bar (TERAX.md): the design must add no per-chunk work. The tracker is O(1) per event and grid reads happen only on click.

## openQuestions
- Default gesture in classic tabs: plain click inside the active prompt input (the user's stated wish), Option+click like iTerm2/Ghostty, or both (plain click when shell integration is active, Option+click as a universal fallback)?
- Link precedence inside the command line being edited: should click-to-move win and links require Cmd+click there, or should Cmd+click be required for links everywhere?
- Is it acceptable that clicking on grey autosuggestion text accepts the suggestion?
- Is v1 limited to the cursor's soft-wrapped line, or must multi-line buffers (Shift+Enter, PS2) be supported? Multi-line support effectively requires the Zig input-start change.
- Install Zig 0.16 and own a custom WASM rebuild for a precise input start? Or stay JavaScript-only?
- Move OSC 133 B to the end of PS1 for zsh and bash (spec-correct, matches fish and pwsh)? This changes prompt presentation gating and needs an updated test.
- Would the user rather make block terminals the default? Click-to-caret already works there today; this would be a new preference.
- Which missing shortcuts to add: Cmd+Z undo (^_), forward kill-word or kill-line, Home/End, rebinding zsh ^U to backward-kill-line, and alternate-scroll for less/man?
