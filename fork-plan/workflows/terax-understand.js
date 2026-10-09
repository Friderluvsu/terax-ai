export const meta = {
  name: 'terax-understand',
  description: 'Map Terax codebase seams for click-to-cursor, HTML preview, SSH remote explorer, plus upstream research, build baseline and gap analysis',
  phases: [
    { title: 'Scout', detail: 'parallel readers per feature area + upstream + build env + gaps' },
  ],
}

const REPO = '/Users/frider/Documents/iamcoder/terax_claude'

const PREAMBLE = `You are scouting the Terax codebase at ${REPO} (a fresh clone of https://github.com/crynta/terax-ai, Tauri 2 + Rust backend, React 19 + TypeScript frontend, libghostty-vt WASM terminal model with WebGPU/WebGL renderers).
Read ${REPO}/TERAX.md first (it is the living architecture doc; CLAUDE.md just points to it). Long-form docs are in ${REPO}/docs/architecture/.
Shell note: in this user's zsh, \`ls\` is aliased to eza with icon glyphs; use /bin/ls or find/rg instead.
You are READ-ONLY: do not modify, create or delete any file in the repo.

The user owns a personal fork on macOS (Apple Silicon) and wants to customize Terax for their own needs:
 1. In the terminal, edit the text being typed by CLICKING anywhere to place the cursor, instead of moving with arrow keys.
 2. Open HTML files not only as source code but also as a rendered preview (like the existing Markdown raw/rendered toggle).
 3. A proper SSH connection feature where the explorer shows the REMOTE server's file tree, like VS Code Remote-SSH (open/edit remote files, remote terminal).
 4. Other features that may be missing for them.
Your job is to produce a precise, evidence-based map for ONE area so a planner can later split implementation into subagent tasks. Cite concrete file paths with line numbers. Distinguish what you verified in code from what you infer. Be exhaustive within your area.`

const SCOUT_SCHEMA = {
  type: 'object',
  properties: {
    report: { type: 'string', description: 'Detailed markdown report with file:line references, data flow, and how a change would be implemented' },
    existingSupport: { type: 'string', description: 'What already exists today in this area (verified)' },
    keyFiles: { type: 'array', items: { type: 'object', properties: { path: { type: 'string' }, why: { type: 'string' } }, required: ['path', 'why'] } },
    integrationPoints: { type: 'array', items: { type: 'string' }, description: 'Exact seams where new code should hook in' },
    risks: { type: 'array', items: { type: 'string' } },
    openQuestions: { type: 'array', items: { type: 'string' }, description: 'Decisions the user or planner must make' },
  },
  required: ['report', 'existingSupport', 'keyFiles', 'integrationPoints', 'risks', 'openQuestions'],
}

const SCOUTS = [
  {
    key: 'click-cursor',
    prompt: `${PREAMBLE}

AREA: click-to-place-cursor while editing terminal input.
Determine exactly how terminal input works today and where a mouse click can move the text cursor:
- Block mode: the "editor-like command input" / shared command bar (look at src/modules/terminal/block/ShellInput.tsx, block/lib, lib/blockController.ts, ghostty/GhosttyBlockOverlay.tsx, ghostty/ghosttyBlockSessions.ts). What editor implementation is it (CodeMirror, textarea, custom)? Does clicking inside it already move the caret? Is it enabled by default, when does it activate (OSC 133 shell integration), which shells? Settings toggles for blocks?
- Classic/bare mode (no block command bar, or inside the shell prompt of zsh/bash/fish via zle/readline): what happens on a click today (selection pins, src/modules/terminal/ghostty/selection, ghostty/input)? Design how click-to-move would work: compute the cell delta between the clicked cell and the cursor on the current prompt line(s) bounded by OSC 133 B (prompt end / input start) and send Left/Right arrow key sequences (respecting wide chars/graphemes, wrapped lines, multi-line input, DECCKM application cursor mode). Check whether the adapted libghostty-vt core in packages/ghostty-core already contains Ghostty's prompt click-to-move logic (search for click_events, cursor-click-to-move, cl=line, promptClick, OSC 133 options parsing, semantic prompt). Check how OSC 133 A/B/C/D are surfaced to JS (pins, events).
- Full-screen TUI apps (vim, nano, micro, htop): verify that mouse tracking modes (DECSET 1000/1002/1003/1006) are encoded and forwarded to the PTY so apps with mouse support get clicks. Find the mouse encoding code.
- Interaction conflicts: click vs drag-selection vs link-click vs block selection; modifier keys (iTerm2 uses Option+click, Ghostty uses plain click on prompt). Keyboard shortcuts for word jumps (Option+Left/Right, Cmd+Left/Right, Cmd+Backspace) - are they mapped in lib/keymap.ts? Missing ones are relevant too since the user dislikes arrow-key navigation.
- Tests: where existing tests for input/selection/keymap live, and patterns used.
Report the recommended implementation approach per mode with exact integration points.`,
  },
  {
    key: 'html-preview',
    prompt: `${PREAMBLE}

AREA: rendered HTML preview for local .html files.
Map:
- How opening a file chooses the tab kind (editor vs markdown vs image/video/audio/pdf viewer). Find the dispatcher (src/modules/tabs/lib/useTabs.ts, explorer open actions, App.tsx, editor/lib/languageResolver.ts etc.).
- The Markdown raw/rendered toggle end to end (src/modules/markdown/*, MarkdownViewToggle.tsx, the markdown tab kind, how the toggle state is stored, how it re-renders on file change/save, and lazy-loading/eager-bundle budget tests). This is the pattern to mirror.
- The preview module (src/modules/preview/*): native child webview vs sandboxed iframe, how URLs are validated (lib/previewUrl.ts), the address bar, reload, external-browser fallback, Rust side for child webview if any (grep src-tauri for webview/child/preview).
- How image/video/audio/PDF viewers load local file bytes (convertFileSrc/asset protocol, blob URLs, base64 over IPC?). Inspect src-tauri/tauri.conf.json security.csp, assetProtocol config and scope, capabilities/default.json, any register_uri_scheme_protocol / register_asynchronous_uri_scheme_protocol usage in Rust.
- Evaluate concrete options for rendering an HTML file WITH its relative assets (css/js/img) and live reload on save: (a) iframe srcdoc, (b) asset protocol URL, (c) custom URI scheme protocol served from Rust scoped to the file's directory/workspace, (d) a tiny 127.0.0.1 static server feeding the existing preview pane. For each, state security implications (scripts in the preview, access to Tauri IPC, CSP, sandbox attributes, path traversal, secret deny-list in lib/security.ts and Rust fs guards), and what the existing code makes cheapest.
- File watching: how editor/markdown tabs get notified of disk changes (explorer/lib/watch.ts, src-tauri fs/watch.rs, useEditorFileSync.ts) so the preview can live-reload.
- Bundle budget enforcement (.size-limit.json, eager-graph scripts, eager-budget tests) that a new lazy chunk must respect.
Recommend the best approach with exact integration points and a test plan.`,
  },
  {
    key: 'ssh-seams',
    prompt: `${PREAMBLE}

AREA: architecture seams for a remote (SSH) workspace where the explorer, editor and terminal operate on a remote host.
The closest existing analog is the WSL workspace environment (Local vs WSL distro). Map in depth:
- src/modules/workspace/env.ts and src-tauri/src/modules/workspace.rs: the environment model, how a tab/space carries its env, how PTY spawn differs per env (pty/session.rs, pty/shell_init.rs), how fs commands receive paths for WSL (UNC \\\\wsl$ paths? or wsl.exe exec?), the workspace authorization registry (workspace_authorize) and how every fs/git/shell command is gated by it.
- Explorer data source: src/modules/explorer/lib/useFileTree.ts, watch.ts, useGitStatus.ts; the Rust fs commands (fs/tree.rs, fs/file.rs, fs/mutate.rs, fs/search.rs, fs/grep.rs, fs/watch.rs) and their exact signatures. Identify whether there is (or could cheaply be) a frontend "filesystem provider" abstraction so the explorer/editor can switch between local and remote backends, and list every call site that would need routing (invoke('fs_*') etc.).
- Editor IO: open/save with mtime conflict checks (useDocument.ts, useEditorFileSync.ts), how paths are used as tab identity.
- Spaces (src/modules/spaces): persisted shape, where a remote host/root could be stored.
- Terminal: how a terminal tab is spawned (pty_open args, cwd, env), and what it would take to spawn \`ssh -tt host\` (system OpenSSH) in a PTY with shell integration on the remote (OSC 7/133 scripts are injected locally today; how could remote shells get them?). How OSC 7 cwd with a remote hostname is parsed today (lib/osc-handlers.ts, parseOsc7) - is the host part dropped/validated?
- Git, LSP, AI tools, file search: which features would break or must be disabled for a remote workspace; list them.
- Security model (docs/architecture/security-model.md, ai/lib/security.ts, Rust path guards): constraints a remote FS provider must respect.
- src-tauri/Cargo.toml: current dependencies relevant to SSH/crypto/async runtime (tokio features, any ssh crates), and release profile settings (opt-level, lto, strip) that affect binary size.
Produce a concrete seam map: the minimal set of interfaces to introduce (frontend FsProvider, Rust remote module, env kind 'ssh'), and the ordered refactor steps.`,
  },
  {
    key: 'upstream-and-ssh-ecosystem',
    model: 'sonnet',
    prompt: `${PREAMBLE}

AREA: upstream project research + SSH implementation ecosystem. You may use the gh CLI (read-only queries only, never comment/star/fork/create anything) and web search/fetch.
1) Upstream GitHub crynta/terax-ai: search issues, PRs (all states) and discussions for: ssh, sftp, remote, click cursor / mouse cursor / move cursor, html preview, and gather the most-requested open feature requests (sort by reactions/comments; use \`gh issue list -R crynta/terax-ai --state all --search ...\`, \`gh pr list -R crynta/terax-ai --state all --search ...\`, \`gh api\` as needed). Note any in-progress SSH branch/PR or maintainer statements about how SSH should be done (roadmap says: "SSH support (PTY auth and known_hosts first; SFTP and port forwarding later)"). Also check the git branches of the clone (git -C ${REPO} branch -a) for ssh work.
2) Rust SSH ecosystem as of 2026: compare russh (+ russh-sftp), ssh2 (libssh2 bindings), openssh + openssh-sftp-client (wrap system ssh with ControlMaster), and the approach of shelling out to the system ssh/sftp binaries. Criteria: binary size impact (the app is ~7-8 MB and size-sensitive), ~/.ssh/config support (Host aliases, ProxyJump, IdentityFile, IdentityAgent), ssh-agent and 1Password/Secretive agent support, known_hosts verification, keyboard-interactive/2FA, macOS/Linux/Windows support, async (tokio) fit, maintenance status and latest versions.
3) How comparable apps do remote file trees: VS Code Remote-SSH (installs vscode-server), Zed remote development (remote server binary over ssh), Warp, Tabby/Termius/WindTerm SFTP panels, Wave Terminal (wsh remote connections). Summarize the trade-offs of "SFTP-only provider" vs "deploy a small helper binary on the remote" (file watching, search/grep, git status, latency).
Return a recommendation for a personal macOS-focused fork that still keeps cross-platform builds compiling.`,
  },
  {
    key: 'build-env',
    model: 'sonnet',
    prompt: `${PREAMBLE}

EXCEPTION to read-only: you MAY install dependencies and produce build artifacts (node_modules, src-tauri/target), but must NOT edit any tracked source file.
AREA: establish a working local dev/build baseline on this Mac so implementation agents can verify their work.
- Check toolchain versions vs requirements: node (>=22; note node/pnpm are nvm-lazy-loaded shell functions here, so run them through zsh -lic or source nvm), pnpm (packageManager pins pnpm@11.9.0; corepack?), rustc/cargo (check rust-toolchain files), tauri CLI (@tauri-apps/cli in devDependencies?), Xcode CLT, cargo-nextest availability, zig (is it needed to build packages/ghostty-core WASM or are artifacts committed?).
- Run \`pnpm install\` (frozen lockfile if possible). Then run \`pnpm check-types\`, \`pnpm lint\`, \`pnpm test\` and record pass/fail counts and the time each took.
- Run \`cargo check --locked\` (or clippy as TERAX.md specifies) in src-tauri; it may take a while, use generous timeouts (up to 10 minutes per command, run in background and poll if needed). Record results.
- Determine the exact commands to launch the app in dev mode (pnpm tauri dev) and to build a release .app on macOS, and anything special (signing, entitlements, env vars). Do NOT leave a dev server running when you finish.
In the report include: a copy-pasteable command list, baseline pass/fail per check (so later regressions can be attributed), durations, and any blockers with fixes.`,
  },
  {
    key: 'gaps',
    prompt: `${PREAMBLE}

AREA: gap analysis - what else this user is likely missing. The user is a Russian-speaking developer on macOS who works with remote servers over SSH, edits HTML, and dislikes keyboard-only cursor navigation in the terminal.
Investigate the codebase for concrete, verifiable gaps and pain points, for example (verify each, do not assume):
- Keyboard shortcuts with a non-Latin (Russian ЙЦУКЕН) layout: do shortcut handlers match on KeyboardEvent.key (breaks on Russian layout: Cmd+T arrives as key 'е') or on KeyboardEvent.code? Check src/modules/shortcuts/*, terminal keymap, CodeMirror keymaps, command palette. This is a very common real pain point.
- UI localization: is there any i18n system? Russian UI feasibility.
- Terminal: history-based autosuggestions (on roadmap), command history search UI, saved commands/snippets, quick SSH host launcher from ~/.ssh/config, broadcast input to panes, session persistence / tmux integration, search in scrollback, semantic word jumps, Option-as-Meta setting on macOS, copy-on-select, font ligatures.
- Explorer/editor: quick open (Cmd+P), global search & replace UI, multi-cursor, minimap, breadcrumbs, split editor, compare files, upload/download via drag and drop, file preview for more types (SVG rendered, CSV table, JSON tree).
- Remote-adjacent: port forwarding UI, Docker containers as environments, remote file sync.
- Anything noted as TODO/FIXME or obviously half-finished (rg TODO|FIXME|XXX), and the ROADMAP.md "Coming next" list.
For each candidate, record: what exists today (with evidence), what is missing, user value for THIS user (high/medium/low), implementation size (S/M/L), and where it would live. Rank the top 10.`,
  },
]

phase('Scout')
const results = await parallel(SCOUTS.map(s => () =>
  agent(s.prompt, { label: `scout:${s.key}`, phase: 'Scout', schema: SCOUT_SCHEMA, ...(s.model ? { model: s.model } : {}) })
    .then(r => r ? ({ key: s.key, ...r }) : null)
))
const ok = results.filter(Boolean)
const missing = SCOUTS.map(s => s.key).filter(k => !ok.find(r => r.key === k))
if (missing.length) log(`Scouts with no result: ${missing.join(', ')}`)
return { scouts: ok, missing }
