# gaps

## existingSupport
Verified present: Blocks terminal mode (Cmd+Shift+T) with a CodeMirror command line (click-to-place cursor, history ghost autosuggest via Rust history_suggest, history popover, Tab completion), opt-in only. Command history search (Cmd+P then '>'), content search (Cmd+P then '#', literal, no replace), explorer fuzzy file search (Cmd+Shift+F), scrollback search (plain query), Option and Cmd arrow and backspace readline mappings, double and triple click selection, Option+drag rectangular selection, http(s) and mailto link detection opening externally on plain click, AI prompt snippets (#handle), agent-only completion notifications, Spaces restoring tab tree, cwd and blocks flag (no processes or scrollback), WSL as the only non-local workspace env, image, video, audio and PDF viewers for binary files only (SVG shows as code), Markdown raw/rendered tab, HTML and CSS LSP presets (root marker required), format-on-save with prettier, vim mode, CodeMirror basicSetup with in-file find/replace, local-only `terax open --line` CLI. A GhosttyInputController `macOptionIsMeta` option exists but is never wired to a preference. Absent: any SSH, SFTP, port-forward or Docker code; i18n; copy-on-select; broadcast input; terminal saved commands; file path links; split editor; Emmet; ligatures. TODO/FIXME debt is near zero (one TODO in packages/ghostty-core/lib/ghostty.ts:529).

## report
## Gap analysis: Terax fork for a Russian-speaking macOS developer (SSH, HTML, mouse-first terminal editing)

Scope: what the user probably lacks beyond their three explicit asks (click-to-edit in the terminal, HTML preview, Remote-SSH explorer). Every claim below is tagged **[V]** (verified in code, cited by file:line), **[I]** (inferred from library or platform knowledge, not verified) or **[GH]** (upstream GitHub issue or PR metadata fetched via the API; treat as data). The repo is unmodified. `node_modules` is not installed, so CodeMirror and WebKit internals could not be checked locally.

---

### 0. Context the planner needs first (it changes the three explicit asks)

1. **A click-to-edit command line already exists in "Blocks" terminals [V].** In a Blocks tab (Cmd+Shift+T, `src/modules/shortcuts/shortcuts.ts:108-112`) the prompt is a CodeMirror 6 editor (`src/modules/terminal/block/lib/shellEditor.ts:385-447`, mounted by `src/modules/terminal/block/ShellInput.tsx:77-116`). Clicking places the cursor natively. It also has:
   - history ghost autosuggest (`block/lib/inlineSuggest.ts:84-124`, accepted with Right, End or Mod-f), backed by Rust `history_suggest` (`src-tauri/src/modules/history/mod.rs:130`);
   - a history popover (`block/lib/historyPopover.ts`);
   - Tab completion of commands and paths.

   So the ROADMAP "Coming next" item "Inline terminal auto-suggestions (history-based first)" (`ROADMAP.md`) is already implemented in Blocks mode, just not in classic tabs. The gaps:
   - (a) Blocks is not the default. `tab.new` calls `newTab`, never `newBlockTab` (`src/app/App.tsx:589-599`), and no preference exists (`src/modules/settings/store.ts:122-187`).
   - (b) Blocks only works at a local prompt with OSC 133 integration. `reduceMode` switches to `running` on `OSC 133;C` (`block/lib/modeMachine.ts:24-36`), so after `ssh host` the remote shell gets raw passthrough. There is no click-to-edit or autosuggest on remote servers unless remote shell integration is injected.
2. **Mouse conflicts for click-to-place-cursor [V]:**
   - A plain left click on a detected URL opens it (`src/modules/terminal/ghostty/gpu/WebGpuTerminalSurface.ts:614-630`; WebGL surface mirrors it at `webgl/WebGlTerminalSurface.ts:617`).
   - Option+drag is already rectangular selection (`ghostty/selection/TerminalSelectionController.ts:206`).
   - Double and triple click select a word or line (`TerminalSelectionController.ts:152`).
3. **Upstream SSH prior art [GH]:**
   - Issue #477: SSH client feature request.
   - Closed PRs: #276 (native Rust SSH PTY + SFTP + profiles), #101 (SFTP remote explorer), #812 (switch workspace when `ssh` runs), #133.
   - Open PRs: #700, #973.
   - Issue #1276: community preview with SFTP, jump hosts and port forwarding.
   - Issue #793: explorer shows local files after SSH on macOS.
   - Persistence: issue #134 and open PR #847.

   These are reusable references for the SSH planner.
4. **There is no SSH, SFTP, Docker or port-forward code at all [V].** `rg -i "ssh|sftp|docker|port.?forward"` over `src` and `src-tauri/src` finds only AI deny-list entries, language modes and the shell completion word list (`shellEditor.ts:157,188`). The environment union is `Local | Wsl` only (`src-tauri/src/modules/workspace.rs:313-319`, `src/modules/workspace/env.ts:5-7`).

---

### 1. Top 10, ranked for this user

| # | Gap | Exists today | Value | Size | Where it would live |
|---|---|---|---|---|---|
| 1 | Quick-connect to SSH hosts from `~/.ssh/config` (palette and new-tab menu, optional tmux auto-attach) | Nothing [V] | High | S-M | New `src-tauri/src/modules/ssh/config.rs` (pure parser, tested) + command `ssh_list_hosts` registered in `src-tauri/src/lib.rs:283-347`; UI in `src/modules/tabs/NewTabMenu.tsx:39` and `src/modules/command-palette/commands.ts`; launch via the agent-launcher pattern `App.tsx:600-636` (`newAgentGroupTab` + `whenSessionReady` + `writeToSession(leaf, "ssh alias\r")`) |
| 2 | Option-as-Meta setting (macOS) | Controller supports `macOptionIsMeta` (`ghostty/input/GhosttyInputController.ts:144,278-281`), but nothing passes it (`ghostty/useGhosttyTerminalSession.ts:844-875`) and no pref exists [V] | High | S-M | Pref in `settings/store.ts`, toggle in `src/settings/sections/GeneralSection.tsx`, pass at `useGhosttyTerminalSession.ts:844`; encode the base char from `event.code` (Option rewrites `e.key`), tests in `GhosttyInputController.test.ts` |
| 3 | Editor keybinding theft: Cmd+D (select next occurrence / multi-cursor), Cmd+[ / Cmd+] (indent) and Cmd+G (find next) are swallowed in editor tabs | Global capture-phase handler always runs `preventDefault` + `stopImmediatePropagation` (`shortcuts/lib/useGlobalShortcuts.ts:27-43`). `pane.splitRight` and `pane.focusNext/Prev` are no-ops for non-terminal tabs (`App.tsx:822-829`; `tabs/lib/useTabs.ts:1259-1269`) and are not in `shortcutsDisabled` (`App.tsx:980-1031`). Cmd+G toggles Source Control (`App.tsx:906`) [V]. CodeMirror binds Mod-d, Mod-[, Mod-] and Mod-g by default [I] | High (HTML editing, multi-cursor) | S | `App.tsx:980` `shortcutsDisabled`: return true for `pane.split*` and `pane.focus*` when `activeTab.kind !== "terminal"`; decide on `pane.source`; add a test |
| 4 | "Default terminal mode: Blocks" preference, so click-to-edit and autosuggest are on by default | Blocks exists, opt-in only via Cmd+Shift+T [V] | High | S | Pref in `settings/store.ts`; `App.tsx:589` `openNewTab` picks `newBlockTab`; split panes should inherit the mode (`useTabs.ts:1290`) |
| 5 | Layout-independent shortcuts (Russian ЙЦУКЕН) | `matchBinding` compares `e.key` and falls back to `e.code` only when the binding has alt or shift (`shortcuts/shortcuts.ts:432-454`; the no-fallback case is locked by `shortcuts.test.ts:118-125`). The recorder stores raw `e.key` (`src/settings/sections/ShortcutsSection.tsx:293-299`). Raw `e.key` checks also in `source-control/SourceControlPanel.tsx:494,627` [V]. The terminal is already robust: copy/paste use `event.code` (`GhosttyInputController.ts:628-656`), Ctrl keys map via `KEY_MAP[event.code]` (`:295`) [V] | Medium-High (verify first, see section 3) | S | `shortcuts.ts:432` (fall back to `keyFromCode` when `e.key` is a non-Latin letter, e.g. `/\p{L}/u && !/\p{Script=Latin}/u`, which keeps the AZERTY/Dvorak test green); recorder stores the code-derived key; fix the SourceControlPanel handlers |
| 6 | Clickable `path:line:col` in terminal output, opening in the editor | Links match only `https?://` and `mailto:` (`ghostty/core/terminalLinks.ts:3-38`); click goes to `openExternalUrl` (`useGhosttyTerminalSession.ts:709-711`; `src/lib/external-link.ts:7-21`) [V] | Medium-High | S-M | Extend `detectTerminalLinks` (pure, tested); resolve relative paths against `leafCwd` (`terminal/lib/terminalSessionApi.ts:59`); open via `openControlFile` (`App.tsx:1313`), which already supports `line`. Consider requiring Cmd+click (conflicts with click-to-place, section 0.2) |
| 7 | Saved commands/snippets for the terminal, plus broadcast input to all panes (multi-server ops) | AI prompt snippets only (`ai/store/snippetsStore.ts`, `ai/lib/snippets.ts`); history palette `>` (`command-palette/lib/mode.ts:9-13`, `CommandPalette.tsx:332-363`); no broadcast (`rg broadcast` hits only AI stores) [V] | Medium-High | M | Store in `tauri-plugin-store`; palette mode (new sigil in `lib/mode.ts`); send via `writeToSession` (`terminalSessionApi.ts:43`, used by `insertHistoryCommand` at `App.tsx:1358-1367`); broadcast toggle per tab in `PaneTreeView.tsx`, loop over `leafIds(paneTree)` (max 4 panes, `useTabs.ts:31`) |
| 8 | "Long command finished" notifications (builds, deploys) | Only coding agents are detected: `DEFAULT_AGENTS` = claude, codex, gemini, pi, opencode, grok (`src-tauri/src/modules/pty/agent_detect.rs:8,201-215`). Block exit codes exist (`ghostty/GhosttyBlocks.ts:120-131`) [V] | Medium | S-M | Reuse the router `agents/lib/route.ts` (focused/unfocused logic); trigger on OSC 133 C to D duration over a threshold. Remote shells give no OSC 133, same caveat as section 0.1 |
| 9 | VS Code-style Cmd+P file quick-open; project-wide replace | Cmd+P palette has command, `>` history and `#` content modes, but no file list (`commands.ts` ids `:89-297`, `lib/mode.ts`). Fuzzy file search lives in the explorer (Cmd+Shift+F, `explorer/ExplorerSearch.tsx:117` to `fs_search`). Content search is literal, smart-case, with no replace (`src-tauri/src/modules/fs/grep.rs:215-240`) [V] | Medium | S (quick open) / M (replace) | Palette default mode merges `fs_search` hits; replace needs a new Rust command with workspace auth plus a preview UI |
| 10 | Rendered previews beyond Markdown: SVG, CSV table, JSON tree; Emmet for HTML | SVG is text, so it never reaches the image branch (`editor/EditorPane.tsx:567-583` only renders images when the doc is `binary`/`toolarge`; binary is detected by NUL or invalid UTF-8 in `src-tauri/src/modules/fs/file.rs:91-101`), so it shows as XML source. CSV/JSON show as text. Emmet is absent from `package.json` [V]. HTML/CSS LSP presets exist (`lsp/lib/presets.ts:207-230`) but need `package.json` or `.git` as a root marker [V] | Medium | S each once the HTML raw/rendered toggle exists; Emmet S-M | Same pattern as the Markdown tab (`App.tsx:668-674`, `isMarkdownPath` `src/lib/utils.ts:8-10`, `MarkdownTab` `useTabs.ts:75-80`); Emmet lazy-loaded with the HTML language (`editor/lib/languageDefinitions.ts:170-178`) |

#### Next tier (real gaps, lower ratio of value to size)
- **Persistent sessions / tmux integration.** Spaces restore only the tab tree, cwd, blocks flag and title; private tabs are dropped and there is no scrollback or process persistence (`spaces/lib/serialize.ts:15-80`) [V]. ROADMAP lists "Persistent terminal processes" [V]; issue #134 and PR #847 are open [GH]. Size L. Cheap substitute: the host launcher (#1) offers "attach tmux on connect" (`ssh -t host tmux new -A -s terax`).
- **Port forwarding UI.** Size L, depends on SSH. Preview accepts only localhost hosts (`preview/lib/previewUrl.ts:10-11`) [V], so remote dev servers need `-L` forwarding to preview.
- **Russian UI localization.** No i18n library or `useTranslation` [V]. Hardcoded English across about 110 non-UI `.tsx` files, about 217 `label: "..."` strings, about 4.9k lines in `src/settings/**`, 23 toast sites, and `Intl` pinned to `en-US` in `src/components/ai-elements/context.tsx:105-208` [V]. Size L. Only `docs/readme/README.ru.md` is translated. The ROADMAP welcomes translations only for docs.
- **Docker containers as environments.** Nothing exists. Size L, low value.
- **Copy-on-select** (S, low). **Terminal search regex/case toggles** (S-M, low): the UI passes a plain query (`header/SearchInline.tsx:106-130`, `ghostty/search/GhosttySearchController.ts`).
- **Split editor** (M): `EditorTab` has no `paneTree` (`useTabs.ts:53-66`), splits are terminal-only (`App.tsx:825`). **Compare two files** (S-M): `@codemirror/merge` is already used by `GitDiffPane`/`AiDiffPane`.
- **Font ligatures, minimap, editor breadcrumbs.** Absent, low value. Ligatures are size L on a cell-based GPU renderer.
- **Explorer context menu** has Open, Open in Terminal, Source Control, Git History, Reveal in Finder, New File/Folder, Copy Path, Copy Relative Path, Attach to Agent and Delete (`explorer/FileExplorer.tsx:795-925`) [V]. Missing: Duplicate, Compare, Open preview, Download/Upload (the last two matter once SSH exists; OS drop into the explorer already copies, `explorer/lib/useExplorerFileDrop.ts`).

#### Verified present (not gaps)
- Scrollback search: header inline search, Ghostty search controller.
- Option+Left/Right word jumps, Cmd+Left/Right line jumps, Option/Cmd+Backspace (`terminal/lib/keymap.ts:8-57`).
- Command history search (Cmd+P then `>`).
- Cmd+K clear on macOS.
- Block navigation (Cmd+Up/Down, Blocks tabs only, `App.tsx:1011-1016`).
- Tab rename via `customTitle`.
- Vim mode; CodeMirror find/replace in a file (`search.focus` to `openSearch`, `App.tsx:914-917`).
- Format-on-save including prettier for HTML (`editor/lib/externalFormat.ts:34-46`).
- `terax open <file> --line` CLI for local panes (`docs/architecture/cli-control.md`).
- TODO/FIXME debt is near zero: only `packages/ghostty-core/lib/ghostty.ts:529` ("cursor color support") [V].

---

### 2. Notes on the top items

**#1 SSH quick-connect.** Best as phase 0 of the SSH epic. It uses the system `ssh` binary in a normal PTY, so `~/.ssh/config`, the agent, ProxyJump, Keychain and known_hosts all work for free, with no new crates. The parser must be pure Rust: Host/HostName/User/Port/Include, skipping wildcard hosts. It reads only `~/.ssh/config`, never keys, and must not be reachable from AI tools; the AI deny-list (`src/modules/ai/lib/security.ts:48,67`) is AI-only (`docs/architecture/security-model.md` "Secret-path deny-list") and must stay intact. Spawning reuses the `whenSessionReady` + `writeToSession` flow (`App.tsx:626-634`). Mark these tabs as private by default, since `StatusBar.tsx:66` already recommends private tabs for SSH.

**#2 Option-as-Meta.** Today, with `macOptionIsMeta` undefined, Option+letter is treated as printable on macOS (`GhosttyInputController.ts:273-293`). Option+B types "∫" instead of readline backward-word (Option+arrows are special-cased in `keymap.ts:8-13`). When enabled, the non-printable path encodes via `KEY_MAP[event.code]` with `utf8 = event.key` (`:295-310`), which is still the Option-rewritten char, so the implementation must supply the base character derived from `event.code`, as `unshiftedCodepoint` does (`:589-605`). The adapted encoder passes `consumed_mods` and `unshifted_codepoint` through (`packages/ghostty-core/adapted/wasm/src/terax.zig:1592-1621`). Encoder output with Alt set needs a unit test [I]. Users of the Russian layout often type «», — and similar via Option, which argues for a Left Option only = Meta variant (track `AltLeft`/`AltRight` keydown, both already in `KEY_MAP` `:121-122`).

**#3 Editor shortcut theft.** This is the cheapest high-impact fix for HTML editing. `matchBinding` matches Cmd+D, `isDisabled` returns false, the handler no-ops for editor tabs, and the event is still consumed. The same applies to Cmd+[ and Cmd+]. Cmd+F is deliberately rerouted to the CodeMirror panel (TERAX.md editor section), so that one is fine.

**#5 Russian layout, precise status.** Verified fragility: plain Mod+letter and Mod+punctuation bindings (Cmd+T, W, D, P, F, I, B, E, G, U, J, R, K, S, Z, Y, `,`, `[`, `]`) match on `e.key` only. Mod+Shift+X and Option combos fall back to `e.code`. Platform behavior is unverified [I]: Apple's Russian layouts probably emit Latin characters while Command is held, and WebKit derives `key` from `NSEvent.characters` without Ctrl, so Cmd shortcuts probably work on macOS. Ctrl+letter would produce Cyrillic `e.key`, but macOS defaults bind no Ctrl+letter (only Ctrl+Tab and Ctrl+Space). What definitely breaks across layouts: custom shortcuts recorded on one layout (stored `e.key`, `ShortcutsSection.tsx:294`) and third-party layouts. Terminal Ctrl+C on Cyrillic works [I]: Ghostty's legacy encoder falls back to the logical key when `utf8` isn't one ASCII byte, and the Zig upstream isn't vendored here; there are no Cyrillic test cases anywhere in the repo [V]. CodeMirror keymaps fall back to `keyCode` for non-ASCII keys with modifiers [I]. Upstream issue #699 (Cyrillic characters duplicated) [GH] is Linux/IBus on v0.7.3 (pre-Ghostty, fix PR #716), so it is unlikely on macOS, but worth a spot-check in the terminal, Blocks editor and AI composer.

**#6 File links.** The detector runs only on pointer demand, over up to 8 wrapped rows (`ghostty/AdaptedGhosttyTerminalModel.ts:373-434`), so path detection adds no idle cost. Paths printed by a remote shell will need remote resolution once SSH lands.

---

### 3. Verification checklist (5 minutes, before implementing #5)
`pnpm dev:inspect`, open Web Inspector, run `addEventListener('keydown', e => console.log(e.key, e.code, e.metaKey, e.ctrlKey), true)`, switch to Russian and Russian-PC layouts, then press Cmd+T, Cmd+], Cmd+`,`, Cmd+Shift+T, Ctrl+Tab, Option+B. Also type Russian text into a classic terminal, a Blocks prompt and the AI composer to rule out #699.

### 4. Suggested subagent sizing (for the planner)
- S, mechanical, well-tested seams (#3, #4, the #5 code change, copy-on-select): a small or fast model.
- S-M/M feature work touching one module plus tests (#1 UI, #2, #6, #7, #8, #9, #10): a mid-tier model.
- Cross-cutting or security-sensitive work (the `~/.ssh/config` Rust parser and its boundary, i18n infrastructure, persistence/tmux, port forwarding, remote shell-integration injection for Blocks over SSH): the strongest model, plus a review pass.

Every task must keep `pnpm lint`, `pnpm check-types`, `pnpm test`, `cargo clippy -D warnings` and `cargo nextest` green, plus the bundle budgets in `.size-limit.json` (eager 540 KB, total 1505 KB gzip) and `src/app/eager-budget.test.ts`.

## keyFiles
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/shortcuts/shortcuts.ts: Shortcut registry and matchBinding (lines 432-454): e.key matching with e.code fallback only for alt/shift; root of Russian-layout fragility
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/shortcuts/shortcuts.test.ts: Lines 118-125 lock the 'no code fallback without alt/shift' behavior; must be extended, not broken, by a layout fix
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/shortcuts/lib/useGlobalShortcuts.ts: Capture-phase window keydown; preventDefault+stopImmediatePropagation on any match, which causes editor key theft
- /Users/frider/Documents/iamcoder/terax_claude/src/app/App.tsx: shortcutsDisabled (980-1031), shortcut handlers (877-920), splitActivePaneInActiveTab no-op for editors (822-829), openNewTab (589), agent launch pattern whenSessionReady+writeToSession (600-636), openControlFile (1313), insertHistoryCommand (1358)
- /Users/frider/Documents/iamcoder/terax_claude/src/settings/sections/ShortcutsSection.tsx: Shortcut recorder stores raw e.key (line 294), so layout-dependent custom bindings
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/input/GhosttyInputController.ts: Terminal key and mouse input; macOptionIsMeta option (144, 278-281) unused; code-based copy/paste (628-656); KEY_MAP by event.code
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/useGhosttyTerminalSession.ts: createGhosttyInput (833-875) is where Option-as-Meta must be passed; onOpenLink (709-711) is where file-path links would route
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/lib/keymap.ts: Existing Option/Cmd arrow and backspace readline mappings
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/block/lib/shellEditor.ts: Blocks CodeMirror command line (click-to-edit already works here)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/block/lib/inlineSuggest.ts: History-based inline autosuggest already implemented for Blocks
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/block/lib/modeMachine.ts: Blocks mode switches to 'running' on OSC 133;C, so ssh sessions lose the CodeMirror prompt
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/history/mod.rs: Rust history index (zsh/bash/fish) behind autosuggest and the palette history mode
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/core/terminalLinks.ts: Link detector: only http(s)/mailto; extension point for path:line:col
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/gpu/WebGpuTerminalSurface.ts: Plain-click link activation (614-630) conflicts with click-to-place-cursor
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/selection/TerminalSelectionController.ts: Option+drag = rectangular selection (206); word/line multi-click (152)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/lib/terminalSessionApi.ts: writeToSession/submitToLeaf/leafCwd: seams for saved commands, broadcast, SSH launch, path resolution
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/settings/store.ts: Preferences schema (122-187) and defaults (310-373); add terminalMacOptionIsMeta, defaultTerminalMode, copyOnSelect, etc.
- /Users/frider/Documents/iamcoder/terax_claude/src/settings/sections/GeneralSection.tsx: Terminal settings UI rows where new toggles go
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/tabs/NewTabMenu.tsx: New-tab menu: natural place for an SSH hosts submenu
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/tabs/lib/useTabs.ts: Tab union (33-130; EditorTab has no paneTree, so no split editor), newBlockTab (750), newAgentGroupTab (~770-802), MAX_PANES_PER_TAB=4 (31)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/command-palette/lib/mode.ts: Palette sigils (> history, # content); add file quick-open and saved-command modes
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/command-palette/CommandPalette.tsx: Palette UI and history-run flow
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/grep.rs: Content search: literal, smart-case (215-240); no replace command
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/editor/EditorPane.tsx: Media viewer branch only for binary/toolarge docs (567-630), so SVG renders as code; basicSetup (663-673); editor keymap (379-390)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/source-control/SourceControlPanel.tsx: Raw e.key letter checks (494, 627), layout-dependent
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/agent_detect.rs: Notifications armed only for agent commands (DEFAULT_AGENTS line 8); generalization point for long-command notifications
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/agents/lib/route.ts: Notification router (focused/unfocused/OS notify) reusable for command-finished alerts
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/spaces/lib/serialize.ts: What persists across restarts (tab tree, cwd, blocks, title; no processes or scrollback)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/workspace.rs: WorkspaceEnv enum Local|Wsl (313-319): seam for SSH/Docker envs
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/ai/lib/security.ts: AI-only secret deny-list (.ssh, known_hosts); any ~/.ssh/config reader must live in Rust and stay out of the AI tool surface
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/preview/lib/previewUrl.ts: Preview restricted to localhost (10-11), so remote dev servers need port forwarding
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/lsp/lib/presets.ts: HTML/CSS LSP presets exist (207-230), require package.json or .git root
- /Users/frider/Documents/iamcoder/terax_claude/.size-limit.json: Bundle budgets any new dependency (i18n, Emmet) must respect
- /Users/frider/Documents/iamcoder/terax_claude/ROADMAP.md: Coming next: SSH, inline autosuggestions (already in Blocks), persistent terminal processes

## integrationPoints
- src/app/App.tsx:980 shortcutsDisabled: return true for pane.splitRight/splitDown/focusNext/focusPrev (and decide pane.source) when activeTab.kind !== 'terminal', so CodeMirror Mod-d / Mod-[ / Mod-] / Mod-g fire in the editor
- src/modules/shortcuts/shortcuts.ts:432 matchBinding: add e.code fallback when e.key is a non-Latin letter (/\p{L}/u && !/\p{Script=Latin}/u); extend shortcuts.test.ts with Cyrillic cases while keeping the 'ç' case at lines 118-125
- src/settings/sections/ShortcutsSection.tsx:293-299 recorder: store the keyFromCode-derived key for letters and digits instead of raw e.key
- src/modules/source-control/SourceControlPanel.tsx:494,627: replace raw e.key letter checks with a code-aware helper
- src/modules/settings/store.ts (Preferences type 122-187, defaults 310-373): new prefs terminalMacOptionIsMeta, defaultTerminalMode ('classic'|'blocks'), copyOnSelect, notifyLongCommands
- src/modules/terminal/ghostty/useGhosttyTerminalSession.ts:844 createGhosttyInput: pass macOptionIsMeta from preferences; GhosttyInputController.ts:295-310 supply the base char from event.code when Option is Meta
- src/app/App.tsx:589 openNewTab: honor defaultTerminalMode by calling newBlockTab
- New src-tauri/src/modules/ssh/config.rs (pure ~/.ssh/config parser, tested) + #[tauri::command] ssh_list_hosts registered in src-tauri/src/lib.rs invoke_handler (around lines 283-347)
- src/modules/tabs/NewTabMenu.tsx:39 and src/modules/command-palette/commands.ts: SSH hosts entries; launch via the App.tsx:600-636 pattern (newAgentGroupTab, whenSessionReady, writeToSession(leaf, 'ssh <alias>\r'))
- src/modules/terminal/ghostty/core/terminalLinks.ts:3 detectTerminalLinks: add file path:line:col detection (pure, tested); route in useGhosttyTerminalSession.ts:709 onOpenLink to openControlFile (App.tsx:1313) with leafCwd (terminalSessionApi.ts:59)
- src/modules/command-palette/lib/mode.ts:9-27: add sigils for saved commands and file quick-open; send via writeToSession (terminalSessionApi.ts:43)
- src/modules/terminal/PaneTreeView.tsx + terminalSessionApi.writeToSession: broadcast-input toggle across leafIds(paneTree)
- src-tauri/src/modules/pty/agent_detect.rs and src/modules/agents/lib/route.ts: generalize OSC 133 C->D timing into long-running command notifications
- src/modules/editor/EditorPane.tsx:567 and the Markdown-tab pattern (App.tsx:668-674, src/lib/utils.ts:8 isMarkdownPath, useTabs.ts:75 MarkdownTab): add rendered SVG/CSV/JSON views alongside the HTML preview toggle
- src/modules/editor/lib/languageDefinitions.ts:170-178: lazy-load Emmet together with the HTML language
- src-tauri/src/modules/workspace.rs:313 WorkspaceEnv and src/modules/workspace/env.ts:5: enum seam for future Ssh/Docker environments

## risks
- The editor key-theft fix changes global shortcut semantics. If pane.* is disabled outside terminals, Cmd+D in non-editor tabs (preview, git) falls through to WebKit; check that no browser default fires.
- A Russian-layout fix in matchBinding must not regress AZERTY/Dvorak/Option-character users (test at shortcuts.test.ts:118-125). Actual WKWebView e.key behavior with Cmd on Russian layouts is unverified; run the verification checklist first.
- Option-as-Meta: the Option-rewritten e.key must not reach the encoder as utf8. Users who type «» or — via Option need a Left-Option-only mode. The Ghostty encoder's Alt handling in the adapted WASM must be covered by unit tests.
- Making Blocks the default changes behavior for bare shells and Bash < 4.4 (TERAX.md: command bar activates only after OSC 133 confirms prompt input). Blocks UX does not apply inside ssh sessions without remote shell integration, so click-to-edit and autosuggest will not work on remote servers unless integration is injected remotely.
- Plain-click URL activation (WebGpuTerminalSurface.ts:614-630) and Option+drag rectangular selection conflict with the click-to-place-cursor feature; file-path links would add more click targets. Consider requiring Cmd+click for links.
- Security: an ~/.ssh/config reader must be Rust-side, read-only, never touch keys, and never be exposed to AI tools. The AI deny-list in src/modules/ai/lib/security.ts must not be weakened. Saved commands and broadcast input could leak secrets into history or other panes; respect private tabs.
- Bundle budgets (.size-limit.json: 540 KB eager, 1505 KB total gzip) and src/app/eager-budget.test.ts will fail if i18n, Emmet or preview renderers are imported eagerly; everything new must be lazy.
- Fork maintenance: hot files (App.tsx ~1600 lines, useGhosttyTerminalSession.ts 1238 lines, shortcuts.ts, settings/store.ts) churn upstream, so merge conflicts will be frequent. Keep logic in new pure modules and keep edits to these files thin.
- Long-command notifications and Blocks features depend on OSC 133, which remote shells do not emit, so these features silently do nothing over SSH.
- Full Russian UI localization touches about 110 TSX files plus settings and toasts; it is large, conflict-prone against upstream, and of uncertain value for a developer user.

## openQuestions
- Does the user want a Russian-language UI, or only robust shortcuts on the Russian keyboard layout? This decides whether i18n (size L) is in scope.
- Should Blocks become the default terminal mode (it already gives click-to-place cursor and history autosuggest), or should click-to-edit be built for classic terminals too?
- For click-to-edit and autosuggest over SSH: is injecting shell integration into remote shells acceptable (it modifies the remote session environment), or is a local-only experience fine?
- SSH quick-connect: launch the system ssh binary in a PTY (respects ~/.ssh/config, agent, ProxyJump; zero new crates) or a native Rust SSH client (needed later for SFTP explorer and port forwarding)? Should SSH tabs be private by default?
- Is tmux present on the user's servers, and should the host launcher offer auto-attach (ssh -t host tmux new -A -s terax) as a stand-in for persistent sessions?
- Option-as-Meta: both Option keys, or Left Option only (keeps Right Option for typing «» — etc.)?
- For editor shortcut conflicts: should Cmd+G stay 'toggle Source Control', or go back to CodeMirror find-next inside editor tabs?
- Which extra previews matter beyond HTML: SVG, CSV table, JSON tree? Is Emmet wanted for HTML editing?
- Should Cmd+P become VS Code-style file quick-open (with commands behind '>'), or keep the current palette and add a file mode?
- Should the fork stay upstream-mergeable (same quality bar, bundle budgets, tests), or is a looser personal-fork bar acceptable?
