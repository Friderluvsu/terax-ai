<div align="center">
  <img src="public/logo.png" width="112" height="112" alt="Terax" />
  <h1>Terax</h1>
  <p>Terminal, editor, SSH workspaces and AI in one desktop app.</p>
</div>

Terax is a desktop development workspace with a terminal, code editor, SSH connections, Git and AI. It uses Tauri 2 and Rust for native operations, React and TypeScript for the interface, and Ghostty WASM with WebGPU and a WebGL fallback for the terminal. AI uses your own provider keys or local models.

[Русский гид](docs/fork/README.md) · [Documentation](docs/README.md) · [Source](https://github.com/Friderluvsu/terax-ai)

## Build and install (macOS)

Requires Node 22+, pnpm, stable Rust and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform. Run from the repository root:

```bash
source scripts/fork/agent-env.sh
pnpm install --frozen-lockfile
pnpm fork:release
```

The build creates `Terax.app` and `release/Terax-<version>-<arch>.dmg`. Open the DMG and drag the app to Applications, or use `pnpm fork:release -- --install` to build and install it directly.

The macOS build is ad-hoc signed and is not notarized. If macOS blocks the first launch, use **System Settings > Privacy & Security > Open Anyway**.

## Development

```bash
source scripts/fork/agent-env.sh
pnpm fork:dev
```

Command details and diagnostics are in the [scripts guide](scripts/README.md); repository workflow and checks are in [FORK.md](FORK.md).

## Features

### SSH remote workspaces

- Open a remote host from `~/.ssh/config` as its own space: terminal, file explorer, editor and search all run against the server, similar to VS Code Remote SSH.
- Remote terminals get server-side shell integration (cwd tracking and Blocks) through small scripts in `~/.cache/terax/shell-integration` on the host. Can be turned off.
- Files go over SFTP on the system `ssh` with a shared ControlMaster connection. Saves are atomic, keep symlinks and file modes, and detect edits made on the server (a "File changed on disk" prompt instead of silent overwrite).
- Filename and content search on the remote host, connection status indicator with Reconnect, host key changes are refused, never auto-accepted.
- The AI assistant is read-only in SSH workspaces: it can read, list and search, but cannot write files or run commands there.

### Terminal

- **One agent by default** in the launcher; choose up to four when needed.
- **Click output links** to open them in the system browser. Inside the editable command line, use Cmd/Ctrl+click.
- **Classic or Blocks per tab.** Choose the default for new tabs; `Cmd+T` opens the default and `Cmd+Shift+T` the other kind.
- **Click to move the cursor** in the command line (Classic tabs). Hold Option to force it without shell integration.
- **Option as Meta** (both or left only), so `Option+B/F/D` work as word motions while the right Option still types special characters.
- Readline shortcuts and mouse-wheel navigation in full-screen apps (`less`, `htop`).
- **User-installed fonts**, including Nerd Fonts from `~/Library/Fonts`, load into the terminal, so prompt icons render instead of boxes. Compact names like `JetBrainsMono NF` are accepted.
- **Notification when a long command finishes** while the window or tab is in the background (command, exit code and duration only, never output).
- Live prompt redraw while resizing and switching tabs or spaces.
- Ghostty WASM with WebGPU rendering and an on-demand WebGL fallback.

### Preview

- **HTML files** open with Code / Split / Preview modes and live reload, served from a sandboxed `terax-preview://` protocol scoped to the project.
- **SVG preview** and an "Open Preview" item in the explorer context menu.
- Markdown preview with live reload.

### Editor and files

- **Untitled tabs** (`Cmd+E`, or Editor in the new tab menu) hot-save to scratch storage and survive restarts; Save As writes them to a real file.
- `.command` and `.tool` scripts opened from Finder run in a terminal tab (Terax can be the default app for them).
- CodeMirror editing, search and multi-cursor shortcuts, including `Cmd+D`, `Cmd+[`, `Cmd+]` and `Cmd+G`.

### AI and Git

- Use your own AI provider keys or local models, with file context and approval for file changes and shell commands.
- Launch coding agents in terminal panes and receive attention and completion notifications.
- Review diffs, stage changes, commit and browse Git history inside the workspace.

### Security and packaging

- The secret-path deny-list (`.env*`, `.ssh/`, credentials, keychains) is also enforced in Rust, not only in the AI tool layer.
- Application identity `dev.terax.fork`; updates are installed manually.
- Additional preferences are grouped under **Settings > Fork**.

## Size budgets

The limits in [.size-limit.mjs](.size-limit.mjs) are build checks: **1505 KB gzip for all client JavaScript**, and **540 KB gzip for the startup JavaScript of each window**. They do not cap the installed app size or RAM usage. Lazy loading reduces startup work, but lazy chunks still count toward the total budget.

Run `pnpm build` followed by `pnpm size` after loading the environment above. See the [size and architecture guide](docs/fork/size-and-architecture.md) for all budgets, measurement details and optimization evidence.

## Repository map

| Location | Purpose |
| --- | --- |
| [src/](src/) | React interface and frontend modules |
| [src-tauri/](src-tauri/) | Rust backend, native integrations and packaging |
| [packages/ghostty-core/](packages/ghostty-core/) | Adapted Ghostty terminal core and WASM artifacts |
| [public/](public/) | Application assets |
| [scripts/](scripts/README.md) | Build, release and performance tools |
| [docs/](docs/README.md) | Architecture, user guides, validation reports and screenshots |
| [fork-plan/](fork-plan/README.md) | Original task cards and implementation research |
| [TERAX.md](TERAX.md) | Living architecture reference and quality bar |
| [FORK.md](FORK.md) | Repository workflow, environment and verification rules |

`dist/`, `node_modules/`, `src-tauri/target/` and `release/` are generated local directories. They are ignored by Git; their disk usage is separate from the shipped application's size.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md), [FORK.md](FORK.md) and the [testing guide](docs/contributing/testing.md) before changing code.

## Credits and license

This project is a fork of [crynta/terax-ai](https://github.com/crynta/terax-ai), developed in [Friderluvsu/terax-ai](https://github.com/Friderluvsu/terax-ai). Original project documentation is preserved in the [upstream archive](docs/upstream/README.md).

Licensed under [Apache-2.0](LICENSE). Original authorship and bundled dependency notices are retained.
