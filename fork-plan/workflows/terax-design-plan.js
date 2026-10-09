export const meta = {
  name: 'terax-design-plan',
  description: 'Design per-epic subagent task cards for the Terax fork, adversarially verify each against code, then build a cross-epic wave schedule',
  phases: [
    { title: 'Plan', detail: 'one planner per epic produces task cards with model assignment' },
    { title: 'Critique', detail: 'adversarial verifier checks each epic plan against the code and revises it' },
    { title: 'Integrate', detail: 'cross-epic waves, hot-file conflicts, dependencies' },
  ],
}

const REPO = '/Users/frider/Documents/iamcoder/terax_claude'
const SP = '/Users/frider/Documents/iamcoder/terax_claude/fork-plan'
const S = `${SP}/scouts`

const CONTEXT = `CONTEXT
- Repo: ${REPO} (clean clone of crynta/terax-ai 0.9.0-beta, HEAD 3301de2, origin = upstream). Tauri 2 + Rust backend, React 19 + TS frontend, libghostty-vt WASM terminal. Read ${REPO}/TERAX.md (living architecture doc and quality bar; the repo CLAUDE.md is literally just the text "TERAX.md", not an import).
- The user is a Russian-speaking developer on macOS 27 (Apple Silicon) building a PERSONAL fork. Their asks: (1) click anywhere in the terminal input to place the cursor instead of arrow keys, (2) open .html files as a rendered preview, not only as code, (3) proper SSH with the REMOTE file tree in the explorer like VS Code Remote-SSH, (4) suggestions for other missing features.
- Prior scouting reports (evidence with file:line, read the ones relevant to you in full):
  ${S}/click-cursor.md, ${S}/html-preview.md, ${S}/ssh-seams.md, ${S}/upstream-and-ssh-ecosystem.md, ${S}/build-env.md, ${S}/gaps.md
- Shell: the user's zsh has lazy nvm shell functions and an eza 'ls' alias. Before any node/pnpm/cargo command run: source ${SP}/tools/env.sh  (sets node 22, corepack pnpm shim, cargo). Use /bin/ls. You are READ-ONLY in the repo: do not modify tracked files. You may run read-only commands, tests and type checks.
- User environment facts: /Applications/Terax.app 0.8.6 is their daily driver with the same bundle id app.crynta.terax (shared settings dir, keychain service 'terax-ai', ~/Library/Caches/terax/control.json). The updater plugin (src-tauri/src/lib.rs:195, tauri.conf.json:119-122) points at crynta's GitHub releases, so a fork build would be auto-replaced by upstream unless disabled. ~/.ssh/config: 'Include ~/.orbstack/ssh/config' at top, Host * has ControlMaster auto / ControlPath ~/.ssh/master-%r@%h:%p / ControlPersist 30m / ServerAliveInterval 60, hosts: an Ubuntu VM in Oracle Cloud (user ubuntu), an iMac via Tailscale and via mDNS (so a REMOTE can be macOS with BSD userland, not only GNU/Linux), a LAN box. OrbStack is installed and running but has no Linux machines yet (orb create could make a local Linux sshd target for tests, needs user approval). tmux is not installed locally; ripgrep is.
- Verified baseline (build-env.md): pnpm lint 0 errors/90 warnings, check-types clean, vitest 171 files/1209 tests, cargo clippy -D warnings clean, cargo test 373 passed. Bundle budget: total client JS 1.47 of 1.5 MB gzip (only ~30 kB headroom), eager startup 245/540 kB; new UI must be lazy. Single shared src-tauri/target (cargo lock): parallel Rust agents need git worktrees or separate CARGO_TARGET_DIR (~3 min cold, ~4 GB each). Vite dev port 1420 is strict (one dev app at a time). Dev run: pnpm tauri dev --config '{"identifier":"app.crynta.terax.dev"}'. Kill orphaned dev binary by path only (pkill -f target/debug/terax), never pkill -x terax. WebGPU fails on this macOS 27 WebKit, WebGL fallback is what runs here.
- Conventions (TERAX.md): no em-dash, no emojis, minimal comments, '@/' imports, pnpm only, functional core / thin shell, a locking test for any change to terminal spawn, workspace auth, git, fs, IPC or AI tool surface. Hot, upstream-churning files (App.tsx ~1600 lines, useGhosttyTerminalSession.ts, useTabs.ts, settings/store.ts, lib.rs, shortcuts.ts): keep edits thin, put logic in new pure modules.

MODEL RUBRIC for subagents that will EXECUTE the tasks (assign exactly one per task):
- haiku (Claude Haiku 5.5): mechanical, fully specified edits that follow an existing pattern in the same file: a new preference + settings switch, keymap table entries, config/json edits, docs updates, renames, adding test cases to an existing table-driven test, scripts. Low judgement, low blast radius.
- sonnet (Claude Sonnet 5.5): normal feature work inside 1-3 modules with a clear design: React components and hooks, pure TS/Rust functions with tests, a new Tauri command following an existing command's pattern, wiring through existing seams, UI states.
- opus (Claude Opus 5.5): security-critical, concurrency-critical or cross-cutting work and anything needing deep design judgement: SSH transport and ControlMaster lifecycle, SFTP client, fail-closed workspace refactors, the custom URI scheme handler security, IPC boundary changes, Zig/WASM changes, plus REVIEW of every security-sensitive task.
Also give each task reviewBy: none | sonnet | opus (an independent reviewer subagent after implementation). Security/IPC/fs/spawn tasks must be reviewed by opus.

TASK CARD QUALITY BAR: each task is one subagent session (roughly 30-120 minutes of agent work), produces one verifiable, independently mergeable change with tests, lists exact files (verify they exist and that cited line numbers are right by reading the code), concrete steps an agent can follow without this conversation, acceptance criteria, the exact verification commands, dependencies by task id, and whether it can run in parallel with siblings (and which hot files it touches). Prefer more smaller tasks over a few huge ones, but do not split below a meaningful testable unit. Mark manual GUI checks the user must do.`

const PLAN_SCHEMA = {
  type: 'object',
  properties: {
    epic: { type: 'string' },
    summary: { type: 'string', description: 'What the epic delivers and the chosen design in 5-10 sentences' },
    decisions: { type: 'array', items: { type: 'object', properties: {
      id: { type: 'string' }, question: { type: 'string' }, recommended: { type: 'string' },
      alternatives: { type: 'array', items: { type: 'string' } }, impact: { type: 'string' } },
      required: ['id', 'question', 'recommended', 'alternatives', 'impact'] } },
    tasks: { type: 'array', items: { type: 'object', properties: {
      id: { type: 'string', description: 'e.g. E1-T03' },
      title: { type: 'string' },
      model: { type: 'string', enum: ['haiku', 'sonnet', 'opus'] },
      modelRationale: { type: 'string' },
      size: { type: 'string', enum: ['S', 'M', 'L'] },
      dependsOn: { type: 'array', items: { type: 'string' } },
      files: { type: 'array', items: { type: 'string' }, description: 'repo-relative paths to create or modify, with line refs where useful' },
      hotFiles: { type: 'array', items: { type: 'string' } },
      steps: { type: 'string', description: 'markdown, precise and self-contained' },
      acceptance: { type: 'array', items: { type: 'string' } },
      tests: { type: 'array', items: { type: 'string' } },
      verification: { type: 'array', items: { type: 'string' } },
      reviewBy: { type: 'string', enum: ['none', 'sonnet', 'opus'] },
      parallelNotes: { type: 'string' },
      manualCheck: { type: 'string', description: 'GUI check for the user, or empty' },
      risk: { type: 'string' } },
      required: ['id', 'title', 'model', 'modelRationale', 'size', 'dependsOn', 'files', 'hotFiles', 'steps', 'acceptance', 'tests', 'verification', 'reviewBy', 'parallelNotes', 'manualCheck', 'risk'] } },
    milestones: { type: 'array', items: { type: 'string' }, description: 'user-visible checkpoints, in order' },
  },
  required: ['epic', 'summary', 'decisions', 'tasks', 'milestones'],
}

const CRITIQUE_SCHEMA = {
  type: 'object',
  properties: {
    issues: { type: 'array', items: { type: 'object', properties: {
      severity: { type: 'string', enum: ['critical', 'major', 'minor'] },
      taskId: { type: 'string' }, problem: { type: 'string' }, fix: { type: 'string' } },
      required: ['severity', 'taskId', 'problem', 'fix'] } },
    checkedClaims: { type: 'number', description: 'how many file/line/API claims you verified against the code' },
    revisedPlan: PLAN_SCHEMA,
  },
  required: ['issues', 'checkedClaims', 'revisedPlan'],
}

const EPICS = [
  {
    key: 'E0-foundation',
    reports: ['build-env.md', 'upstream-and-ssh-ecosystem.md'],
    brief: `EPIC E0 FOUNDATION (do first, makes every later agent effective and protects the daily driver). Cover at least:
- Git hygiene for a personal fork: rename origin to upstream, a fork integration branch (e.g. fork/main), one branch per epic or task, how agents work in worktrees, how to periodically rebase/merge upstream (upstream churns hot files), keeping custom code in new modules.
- Agent memory: make CLAUDE.md actually import TERAX.md (Claude Code supports '@TERAX.md' import lines) and add a short fork section (env.sh usage or an equivalent committed scripts/agent-env.sh, cargo lock rules, port 1420, kill-by-path, baseline counts, no repo-wide formatting). Keep AGENTS.md consistent.
- Separate app identity for the fork so it coexists with /Applications/Terax.app: dev config file (e.g. src-tauri/tauri.dev.conf.json passed via --config in a package.json script) and a release identity (identifier, productName, keychain service name? control.json path?). Investigate which of these are hard-coded (secrets.rs KEYRING_SERVICE, control.rs cache path) and decide what to change.
- Disable or repoint the auto-updater in the fork (lib.rs:195 plugin, tauri.conf.json updater endpoints/pubkey, createUpdaterArtifacts, the src/modules/updater UI) so upstream releases never overwrite it.
- Tauri npm/crate version mismatch: bump the 11 @tauri-apps/* npm packages (respecting pnpm-workspace minimumReleaseAge) or keep --ignore-version-mismatches; pick one.
- Toolchain helpers for the user (cargo-nextest via brew, corepack enable pnpm) as user-run steps, not agent steps.
- SSH test targets: an OrbStack Linux machine (orb create ubuntu ...) and the user's iMac for a BSD/macOS remote; document as user-approved steps.
- A release build + install script for the fork (.app with ad-hoc signing incl. entitlements) for the user's daily use.`,
  },
  {
    key: 'E1-terminal-input',
    reports: ['click-cursor.md', 'gaps.md', 'upstream-and-ssh-ecosystem.md'],
    brief: `EPIC E1 TERMINAL INPUT (ask 1: click to place the cursor, plus keyboard ergonomics). Default decisions to plan with (you may challenge with evidence):
- Quick win first: a preference "default terminal mode: Classic | Blocks" so Cmd+T can open Blocks tabs, where the CodeMirror command bar already supports click-to-caret, selection, undo and history autosuggest. Check split-pane inheritance and that block tabs cannot be split today (useTabs.ts ~1295); decide what Cmd+D does in a Blocks tab.
- Classic tabs: JS-only v1 click-to-move: PromptInputTracker fed by OSC 133 events, viewportLine(row) model accessor, pure promptClickDelta + arrow encoding via model.encodeKey, click seam in TerminalSelectionController.handlePointerUp (unmoved click), both WebGPU and WebGL surfaces, gates (input phase, primary screen, no mouse tracking, scroll offset 0, not block prompt mode, not IME). Gesture: plain click inside the active prompt input row group when shell integration is active; Option+click as a universal fallback (works over ssh/REPLs, cursor row group only). Links inside the active input require Cmd+click; elsewhere plain click still opens links. Preference toggle to turn it off.
- Optional v2 hardening tasks: move OSC 133 B to the end of PS1 for zsh/bash (update shellBlockCompatibility test, verify PromptPresentationGate), and the Zig prompt_end payload with exact input start (needs Zig 0.16, mark optional/deferred).
- Keyboard: Cmd+Z -> ^_ undo, Fn+Option+Delete -> ESC d, Fn+Cmd+Delete -> ^K, Home/End -> ^A/^E on primary screen, guarded zsh bindkey '^U' backward-kill-line; investigate the keyCode 229 guard (GhosttyInputController.ts ~229, upstream issue #1009) that may swallow Option+Arrow/Backspace in WKWebView and plan a fix with tests; Option-as-Meta preference (Left Option only by default so Right Option still types «» —), deriving base char from event.code.
- Alternate scroll DECSET 1007 (wheel in less/man/git pager on alt screen).`,
  },
  {
    key: 'E2-html-preview',
    reports: ['html-preview.md', 'gaps.md', 'upstream-and-ssh-ecosystem.md'],
    brief: `EPIC E2 HTML PREVIEW (ask 2). Default decisions to plan with (you may challenge with evidence):
- Rust custom async URI scheme 'terax-preview' (register_asynchronous_uri_scheme_protocol) serving files from a registered, authorized root with: per-root random 32-hex token as host, traversal guard (reject .., NUL, absolute, canonicalize + starts_with root, symlink escape refused), Rust port of the secret deny-list (ai/lib/security.ts patterns) as a shared pure module reusable later, own MIME table with octet-stream fallback, nosniff, Cache-Control no-store, ACAO *, a response CSP, single Range support for media, IO in spawn_blocking. Commands preview_serve / preview_release (+ optional preview_open_external via tauri_plugin_opener Rust API). CSP frame-src += terax-preview:. Windows URL form http://terax-preview.localhost/<token>/... handled by a pure URL builder.
- Frontend UX option B: HTML stays an 'editor' tab with htmlView 'code' | 'split' | 'preview' (persisted optionally in spaces serialize), toggle UI generalized from MarkdownViewToggle, lazy HtmlPreviewFrame (sandbox WITHOUT allow-same-origin and WITHOUT allow-top-navigation, no-referrer, key-nonce reload, 30 s hidden suspend like PreviewPane), Reload and Open externally buttons, live reload on save via fs:file-written + fs:changed with canonical-root matching, debounce, and a watch helper that bypasses SKIP_DIRS for explicitly previewed dirs (still authorization-gated). Preview shows saved content (v1).
- Serve root: the nearest authorized workspace/explorer root containing the file (so /root-relative links work), falling back to the file's directory.
- Include: generic isHtmlPath, eager-budget assertion, source-regex sandbox test, docs (TERAX.md, two-process-model.md, security-model.md).
- Related small wins to plan as separate tasks: fix markdown live reload (MarkdownPreviewPane reads once), fix the md raw/rendered duplicate-tab dedupe bug (planMarkdownTabOpen), rendered SVG view via the same frame or <img>, explorer context item 'Open Preview'.
- Separate optional hardening task: narrow the pre-existing assetProtocol scope ["**"] (tauri.conf.json:31-34) carefully without breaking media viewers.
- Design the root/reader abstraction so that later an SSH space can serve preview bytes from the remote provider.`,
  },
  {
    key: 'E3-ssh-remote',
    reports: ['ssh-seams.md', 'upstream-and-ssh-ecosystem.md', 'gaps.md'],
    brief: `EPIC E3 SSH REMOTE WORKSPACE (ask 3, the largest and riskiest). Default design to plan with (you may challenge with evidence, especially on transport):
- Transport: system OpenSSH. Terminal = 'ssh -tt' inside the normal local portable-pty session (keeps OutputCredit flow control, agent detection, resize, Ghostty path); host-key/password/2FA prompts appear in the terminal. Terax-owned ControlMaster socket in a short 0700 dir (macOS 104-byte limit) so the explorer and exec reuse the authenticated connection with BatchMode=yes + ConnectTimeout; check with ssh -O check, close on app exit (RunEvent::Exit). Host resolution and host list via 'ssh -G <alias>' and a pure ~/.ssh/config Host enumerator that follows Include (the user's config includes ~/.orbstack/ssh/config). FS via SFTP: russh-sftp 3.x over the stdio of 'ssh -S <ctl> -s <host> sftp' (measured +167 KB, needs tokio 'process' feature), behind a RemoteFs trait so a helper binary could replace it later. Exec ('ssh -S ctl -o BatchMode=yes -- host sh -lc <quoted>') for search/grep/list, with pure parsers and caps, GNU and BSD (macOS remote) compatible. Atomic save via temp + posix-rename@openssh.com if advertised else exec mv -f; editor conflict check tolerant of 1-second SFTP mtimes. No file watching v1: refresh on expand, focus, manual button, optional polling of expanded dirs.
- Strict security: host/alias argument-injection guard (no leading '-', allowlist chars, always '--'), POSIX single-quote escaping as pure proptested function, never StrictHostKeyChecking=no, never forward TERAX_CONTROL_TOKEN or API keys, no agent forwarding by default, a host-scoped RemoteRegistry of explicitly opened roots gating mutations, the deny-list applied to remote paths, AI tools read-only by default in SSH spaces.
- Order: Phase 0 safety refactors with NO behavior change (Rust fail-closed: fallible local_path replacing resolve_path for non-local envs, exhaustive matches in build_command/build_oneshot_command, lsp reject !Local, pty control env skip for !Local; TS exhaustive env handling; per-tab env via envForSpace(spaceId) instead of global currentWorkspaceEnv in IO paths like useDocument autosave; force LOCAL for theme files; env-keyed caches; fsFor(env) provider consolidating ~31 call sites; env tag on fs:changed/fs:file-written; parseOsc7 returns {host,path} and local tabs ignore foreign hosts). Phase 1 quick win: SSH host picker (palette + new-tab menu) that opens a terminal running ssh <alias> (optionally 'tmux new -A' if present) with a private-tab default; usable before the full remote workspace exists. Phase 2: WorkspaceEnv::Ssh{host} variant (Rust + TS), ControlMaster lifecycle, ssh_* commands, remote PTY with shell-integration bootstrap (upload Terax OSC 7/133 scripts to remote ~/.cache/terax via the mux like the WSL precedent, user setting to disable) so Blocks mode and click-to-move also work on the remote. Phase 3: SFTP RemoteFs and Rust-side dispatch in each fs_* command (async), RemoteRegistry, atomic write. Phase 4: UI (connect flow creating an SSH space with remote root, status-bar env indicator with connection state and reconnect, envCaps gating: hide reveal, no git decorations, source-control placeholder, no format-on-save, media preview via bytes/blob, LSP off), remote search via rg if present else find/grep. Phase 5 optional: port forwarding (-O forward -L) to preview remote dev servers, remote git via exec, AI tools over ssh, upload/download via drag and drop, close-tab running-process warning for ssh tabs.
- Keep all ControlMaster code behind cfg(unix) with a clear non-unix stub so Linux/Windows still compile. Isolate code in new modules (src-tauri/src/modules/remote/*, src/modules/remote/*) to limit conflicts with a future upstream SSH (maintainer reserves SSH). Use the stephenstack fork (feat/ssh-remote-sessions, Apache-2.0) only as reference for POSIX command builders and quoting tests; do not port its byte-dropping pump.
- Testing: pure units with proptests, integration tests gated on an env var pointing at a test host (OrbStack Linux machine; the iMac as BSD remote), and a manual QA script.`,
  },
  {
    key: 'E4-extras',
    reports: ['gaps.md', 'upstream-and-ssh-ecosystem.md', 'click-cursor.md'],
    brief: `EPIC E4 EXTRAS (ask 4: what else the user is missing). Plan the ranked suggestions as optional, independently pickable tasks, each with clear user value. Candidates from gaps.md (verify each claim before planning it, drop any that are wrong):
- Editor key theft fix: Cmd+D / Cmd+[ / Cmd+] (and decide Cmd+G) swallowed by global capture-phase shortcuts in editor tabs (App.tsx shortcutsDisabled ~980). High value for HTML editing, size S.
- Layout-independent shortcuts for the Russian JCUKEN layout: matchBinding e.code fallback when e.key is a non-Latin letter (keep the existing AZERTY/Option test green), recorder stores code-derived key, SourceControlPanel raw e.key checks. Include a manual verification step (dev:inspect keydown logging on Russian layouts) BEFORE coding.
- Clickable path:line:col links in terminal output opening in the editor (Cmd+click, resolve against leaf cwd; local tabs only until SSH).
- Saved commands/snippets palette mode + broadcast input to all panes of a tab (multi-server ops), respecting private tabs.
- Long-running command finished notifications (OSC 133 C->D duration threshold, reuse agents/lib/route.ts).
- Cmd+P file quick-open in the palette (fs_search), project-wide replace (separate larger task).
- Copy-on-select, terminal search regex/case toggles.
- Emmet for HTML (lazy, budget-aware), CSV table / JSON tree previews (lower priority).
- Russian UI localization: plan only as an optional large epic sketch with an i18n approach that is lazy and budget-safe; mark as needs-user-decision.
Order by value-for-this-user / size. Each task must be optional and not block the core epics.`,
  },
]

const planPrompt = (e) => `${CONTEXT}

YOUR JOB: produce the implementation plan for ONE epic as task cards for executor subagents. Read the relevant reports first: ${e.reports.map(r => `${S}/${r}`).join(', ')}. Then verify every file path, line reference and API you put into a task by reading the actual code (the reports can be wrong or stale). Where the reports leave a design decision open, record it in decisions[] with a recommendation; plan the tasks according to the recommendation.

${e.brief}

Return the epic plan via the schema. Task ids must be prefixed with the epic code (${e.key.split('-')[0]}-T01, ...).`

const critiquePrompt = (e, plan) => `${CONTEXT}

YOUR JOB: you are an adversarial reviewer of an implementation plan for epic ${e.key}. Assume it contains mistakes. Verify against the actual code:
- every file path and line reference (open the files), every claimed API/function/test name, every claimed existing behavior;
- the design is correct and secure (attack it: injection, path traversal, IPC exposure, fail-open paths, data loss on save, concurrency, process cleanup, cross-env mixups, bundle budget, eager imports, upstream merge pain);
- tasks are executable by a subagent with no other context, correctly ordered, not missing steps (registration in lib.rs, capabilities, CSP, serialize/persistence, docs, tests that lock invariants), sized sensibly, and the model tier fits the judgement required (do not under-assign security work to haiku/sonnet without opus review; do not waste opus on mechanical edits);
- parallelism claims are right (hot files, cargo lock) and acceptance criteria are testable.
Then return the issues you found AND a fully revised plan (complete, not a diff) that fixes them. Keep what was right. Reference reports for background: ${e.reports.map(r => `${S}/${r}`).join(', ')}.

PLAN UNDER REVIEW (JSON):
${JSON.stringify(plan)}`

phase('Plan')
const revised = await pipeline(
  EPICS,
  (e) => agent(planPrompt(e), { label: `plan:${e.key}`, phase: 'Plan', schema: PLAN_SCHEMA }),
  (plan, e) => plan ? agent(critiquePrompt(e, plan), { label: `critique:${e.key}`, phase: 'Critique', schema: CRITIQUE_SCHEMA, effort: 'high' })
    .then(c => c ? ({ key: e.key, draft: plan, issues: c.issues, checkedClaims: c.checkedClaims, plan: c.revisedPlan }) : ({ key: e.key, draft: plan, issues: [], checkedClaims: 0, plan })) : null,
)

const epics = revised.filter(Boolean)
const missing = EPICS.map(e => e.key).filter(k => !epics.find(r => r.key === k))
if (missing.length) log(`Epics without a plan: ${missing.join(', ')}`)

phase('Integrate')
const INTEGRATE_SCHEMA = {
  type: 'object',
  properties: {
    waves: { type: 'array', items: { type: 'object', properties: {
      name: { type: 'string' }, goal: { type: 'string' }, taskIds: { type: 'array', items: { type: 'string' } },
      parallelism: { type: 'string', description: 'which tasks run concurrently, isolation (worktree / CARGO_TARGET_DIR), merge order' },
      exitGate: { type: 'string', description: 'what must pass before the next wave' } },
      required: ['name', 'goal', 'taskIds', 'parallelism', 'exitGate'] } },
    hotFiles: { type: 'array', items: { type: 'object', properties: {
      file: { type: 'string' }, taskIds: { type: 'array', items: { type: 'string' } }, mitigation: { type: 'string' } },
      required: ['file', 'taskIds', 'mitigation'] } },
    crossEpicDependencies: { type: 'array', items: { type: 'string' } },
    dependencyFixes: { type: 'array', items: { type: 'string' }, description: 'task dependsOn edges that must be added/removed across epics' },
    userDecisions: { type: 'array', items: { type: 'object', properties: {
      question: { type: 'string' }, recommended: { type: 'string' }, why: { type: 'string' } },
      required: ['question', 'recommended', 'why'] } },
    modelBudget: { type: 'string', description: 'counts of tasks per model and reviewer, and where cheaper/stronger tiers would be better' },
    missing: { type: 'array', items: { type: 'string' }, description: 'gaps in the overall plan: unplanned work, unverified assumptions, missing QA' },
  },
  required: ['waves', 'hotFiles', 'crossEpicDependencies', 'dependencyFixes', 'userDecisions', 'modelBudget', 'missing'],
}
const compact = epics.map(e => ({
  epic: e.key, summary: e.plan.summary, decisions: e.plan.decisions,
  tasks: e.plan.tasks.map(t => ({ id: t.id, title: t.title, model: t.model, size: t.size, dependsOn: t.dependsOn, files: t.files, hotFiles: t.hotFiles, reviewBy: t.reviewBy, parallelNotes: t.parallelNotes })),
}))
const integration = await agent(`${CONTEXT}

YOUR JOB: integrate the revised per-epic plans below into one executable schedule for subagents. Verify hot-file overlaps by reading the files when unsure. Produce:
- waves (ordered; E0 first; quick wins early so the user gets value fast: Blocks-default preference, editor key-theft fix, SSH host picker; then the big tracks in parallel where file overlap allows: E1 classic click-to-move, E2 HTML preview, E3 SSH phase 0), with the exact parallelism and isolation strategy (git worktrees per task branch, per-agent CARGO_TARGET_DIR, one integrator merges to fork/main in a fixed order, one dev-app session at a time) and an exit gate per wave (pnpm lint/check-types/test, cargo clippy/test, pnpm build && pnpm size, manual checks);
- every hot file touched by more than one task with a mitigation (sequencing, a shared prerequisite task that adds extension points first, or merge order);
- cross-epic dependencies (e.g. SSH remote shell integration enables Blocks and click-to-move on remote shells; the HTML preview root abstraction and the Rust secret deny-list port reused by SSH; parseOsc7 host awareness touches terminal code that E1 also touches; settings/store.ts touched by many tasks) and concrete dependsOn edges to add;
- the decisions the USER must make before or during execution, each with a recommendation;
- model budget (task counts per model and reviewers) with any reassignment advice;
- anything missing from the overall plan.

REVISED EPIC PLANS (compact JSON):
${JSON.stringify(compact)}`, { label: 'integrate', phase: 'Integrate', schema: INTEGRATE_SCHEMA, effort: 'high' })

return { epics, integration, missing }
