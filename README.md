> [!NOTE]
> **This is a personal fork of [crynta/terax-ai](https://github.com/crynta/terax-ai).** It tracks upstream and adds the features listed below. Download the macOS build from [Releases](https://github.com/Friderluvsu/terax-ai/releases). The original project README follows after this section.

## What this fork adds

### SSH remote workspaces
- Open a remote host from `~/.ssh/config` as its own space: terminal, file explorer, editor and search all run against the server, similar to VS Code Remote SSH.
- Remote terminals get server-side shell integration (cwd tracking and Blocks) through small scripts in `~/.cache/terax/shell-integration` on the host. Can be turned off.
- Files go over SFTP on the system `ssh` with a shared ControlMaster connection. Saves are atomic, keep symlinks and file modes, and detect edits made on the server (a "File changed on disk" prompt instead of silent overwrite).
- Filename and content search on the remote host, connection status indicator with Reconnect, host key changes are refused, never auto-accepted.
- The AI assistant is read-only in SSH workspaces: it can read, list and search, but cannot write files or run commands there.

### Terminal
- **Classic or Blocks per tab.** Choose the default for new tabs; `Cmd+T` opens the default and `Cmd+Shift+T` the other kind.
- **Click to move the cursor** in the command line (Classic tabs). Hold Option to force it without shell integration.
- **Option as Meta** (both or left only), so `Option+B/F/D` work as word motions while the right Option still types special characters.
- Missing readline hotkeys added; the mouse wheel sends arrow keys in full-screen apps (`less`, `htop`).
- **User-installed fonts**, including Nerd Fonts from `~/Library/Fonts`, load into the terminal, so prompt icons render instead of boxes. Compact names like `JetBrainsMono NF` are accepted.
- **Notification when a long command finishes** while the window or tab is in the background (command, exit code and duration only, never output).
- Live prompt redraw while resizing, and a fix for a blank prompt after switching tabs or spaces.
- WebGPU glyph sampling fix.

### Preview
- **HTML files** open with Code / Split / Preview modes and live reload, served from a sandboxed `terax-preview://` protocol scoped to the project.
- **SVG preview** and an "Open Preview" item in the explorer context menu.
- Markdown preview reloads live and no longer opens duplicate tabs.

### Editor and files
- **Untitled tabs** (`Cmd+E`, or Editor in the new tab menu) hot-save to scratch storage and survive restarts; Save As writes them to a real file.
- `.command` and `.tool` scripts opened from Finder run in a terminal tab (Terax can be the default app for them).
- CodeMirror owns `Cmd+D`, `Cmd+[`, `Cmd+]` and `Cmd+G` inside the editor.

### Security and packaging
- The secret-path deny-list (`.env*`, `.ssh/`, credentials, keychains) is also enforced in Rust, not only in the AI tool layer.
- Separate identity `dev.terax.fork`, built-in updater disabled, so the fork never replaces itself with an upstream release.
- All fork options live in **Settings > Fork**.

### Install the fork (macOS, Apple Silicon)

1. Download `Terax-<version>-arm64.dmg` from [Releases](https://github.com/Friderluvsu/terax-ai/releases) and drag Terax to Applications.
2. The build is ad-hoc signed, not notarized. On first launch macOS blocks it: open **System Settings > Privacy & Security** and press **Open Anyway**, or run `xattr -dr com.apple.quarantine /Applications/Terax.app`.

The app is named `Terax.app`, like upstream. Rename or remove the stock Terax first if you have it installed.

### Build the fork from source

```bash
source scripts/fork/agent-env.sh
pnpm install
pnpm fork:dev        # development app
pnpm fork:release    # Terax.app + release/Terax-<version>-<arch>.dmg
```

Fork rules and gates are in [FORK.md](FORK.md); the architecture doc is [TERAX.md](TERAX.md).

---

<div align="center">
  <img src="public/logo.png" width="144" height="144" alt="Terax" />
  <h1>Terax</h1>

  <p><strong>Lightweight Terminal-first AI-native dev workspace.</strong></p>
  <p>
    <a href="https://terax.app">Website</a>
    ·
    <a href="https://terax.app/docs">Docs</a>
    ·
    <a href="https://github.com/crynta/Terax-website">Website's source code</a>
  </p>

  <p>
    <img src="https://img.shields.io/github/v/release/crynta/terax-ai?label=version&color=blue" alt="version" />
    <img src="https://img.shields.io/github/downloads/crynta/terax-ai/total?label=downloads&color=blue" alt="downloads" />
    <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey" alt="platform" />
    <a href="https://discord.gg/tyveTUyEp7"><img src="https://img.shields.io/badge/Discord-5865F2?logo=discord&logoColor=white" alt="Discord" /></a>
    <a href="https://www.youtube.com/@crynta"><img src="https://img.shields.io/badge/Youtube-FF0000?logo=youtube&logoColor=white" alt="YouTube" /></a>
  </p>
</div>

<p align="center">
  <a href="docs/readme/README.zh-CN.md">简体中文</a> |
  <a href="docs/readme/README.es.md">Español</a> |
  <a href="docs/readme/README.de.md">Deutsch</a> |
  <a href="docs/readme/README.fr.md">Français</a> |
  <a href="docs/readme/README.ja.md">日本語</a> |
  <a href="docs/readme/README.ko.md">한국어</a> |
  <a href="docs/readme/README.pt-BR.md">Português</a> |
  <a href="docs/readme/README.pl.md">Polski</a> |
  <a href="docs/readme/README.ru.md">Русский</a> |
  <a href="docs/readme/README.id.md">Bahasa Indonesia</a> |
  <a href="docs/readme/README.hi.md">हिन्दी</a>
</p>

---

Terax is a lightweight open-source terminal-first AI-native development environment (ADE) built on Tauri 2 + Rust and React 19. A native PTY backend with a WebGL renderer, an agentic AI side-panel that runs against your own keys or fully local models, plus a code editor, file explorer, source control with a git graph, and a web preview pane built in. About 7-8 MB on disk. No telemetry. No account.

## Screenshots

<table>
  <tr>
    <td align="center"><img src="docs/web-preview.png" alt="Web preview" /><br/><sub>Web preview of local dev servers</sub></td>
    <td align="center"><img src="docs/ai-workflow.png" alt="AI window" /><br/><sub>Agentic AI workflow with edit diffs in the code editor</sub></td>
  </tr>
  <tr>
    <td align="center"><img src="docs/themes.png" alt="Themes and background image" style="margin-top: 12px;"/><br/><sub>Custom themes, presets, and background images</sub></td>
    <td align="center"><img src="docs/source-control.png" alt="Source control and git graph" style="margin-top: 12px;"/><br/><sub>Source control panel with git graph in history</sub></td>
  </tr>
  <tr>
    <td colspan="2" align="center"><img src="docs/terminal.png" alt="Terminal" style="border-radius: 4px; margin-top: 12px;" /><br/><sub>Block-based WebGL terminal with editor-like input panel</sub></td>
  </tr>
</table>

## Features

### Terminal

- xterm.js with WebGL renderer, multi-tab with background streaming
- GPU-accelerated block-based terminal with editor-like command input
- Native PTY backend via `portable-pty` (zsh, bash, pwsh, fish, cmd)
- Split panels (horizontal and vertical)
- Inline search, link detection, true-color
- Drag files from the explorer or desktop into a terminal as shell-safe quoted paths
- Per-tab workspace environments on Windows (Local, or any installed WSL distro)
- Spaces restore tabs, working directories, and split layouts across launches

### Code editor

- CodeMirror 6 (supports all popular languages - TS/JS, Rust, Python, Go, C/C++, Java, HTML/CSS, JSON, Markdown, etc.)
- Inline AI autocomplete with local model support
- AI edit diffs, accept or reject hunk by hunk
- Opt-in language server support with diagnostics, navigation, completion, formatting, and custom servers
- Rendered Markdown plus image, video, audio, and PDF viewing
- Vim mode
- Built-in editor themes including Kanagawa, Catppuccin, Rosé Pine, Everforest, Dracula, Solarized, Nord, Tokyo Night, GitHub, and Xcode

### Source control

- Stage / unstage hunks, commit (Cmd+Enter / Ctrl+Enter), push with upstream awareness
- Branch display including detached HEAD state
- Git history pane with a real commit graph (lane rendering for merges and branches)
- Commit search and filter, click through to the remote commit page

### File explorer

- Catppuccin icon theme
- Fuzzy search, keyboard navigation, inline rename, context actions
- Live updates when files change on disk
- Attach files and selections directly to the AI side-panel

### Web preview

- Auto-detects local dev servers and opens them in a preview tab
- External URL preview via a native child webview

### Themes and customization

- Custom themes built in-app, switch between bundled presets and your own
- Create your own themes, share them or import from the community
- Background images with adjustable opacity and blur
- Editor theme is independent from the app theme

### AI

- **BYOK providers:** OpenAI, Anthropic, Google (Gemini), Groq, xAI (Grok), Cerebras, OpenRouter, DeepSeek, Mistral, plus any OpenAI-compatible endpoint
- **Local / offline:** LM Studio, MLX, Ollama
- **Agentic workflow:** plans, sub-agents, project memory via `TERAX.md`, file read / write / edit / multi-edit / grep / glob, bash with approval gating, background processes
- **Coding-agent orchestration:** spawn Claude Code in a terminal, inspect its output, and send follow-up work through approval-gated tools
- **Composer:** prompt snippets via `#handle`, files via `@path`, voice input, attach-to-agent from explorer or selection
- **Custom agents** with their own system prompt and tool subset
- **Plan mode** for multi-step work, generates and confirms before doing

## Install

Latest installers are on the [Releases](https://github.com/crynta/terax-ai/releases/latest) page. Terax auto-updates from there.

### Windows notes

- Default shell detection: `pwsh.exe` (PowerShell 7+) -> `powershell.exe` (Windows PowerShell 5.1) -> `cmd.exe`.
- WSL is a first-class workspace environment, not a wrapped subprocess.

### Linux notes

- **Arch / AUR:** `yay -S terax-bin` (or `paru`, etc.). Tracks the latest release.
- **NixOS / Nix**: use the official flake - `nix profile install github:crynta/terax-ai` (non-NixOS), or import the flake and add `inputs.terax.packages.${pkgs.system}.terax` to `environment.systemPackages` (NixOS). The `nixosModules.terax` output is also available for a simpler setup.
- **AppImage:** needs FUSE. Without it: `./Terax_*.AppImage --appimage-extract-and-run`. On Wayland with rendering glitches, try `WEBKIT_DISABLE_DMABUF_RENDERER=1`. Otherwise the `.deb` / `.rpm` packages link against the system GTK stack and tend to be smoother.

## Configure AI

1. Open **Settings -> AI**.
2. Pick a provider and paste your API key. For local inference, point Terax at your LM Studio / MLX / Ollama endpoint.
3. Keys are written to the OS keychain via `keyring`. They never touch disk or localStorage.

## Build from source

**Prerequisites**
- Rust (stable), https://rustup.rs
- Node 22+ and [pnpm](https://pnpm.io)
- Tauri prerequisites for your platform, https://tauri.app/start/prerequisites/

**Run**
```bash
pnpm install
pnpm tauri dev          # development
pnpm tauri build        # production bundle
```

**Checks**
```bash
pnpm lint
pnpm check-types
pnpm test
cd src-tauri && cargo clippy --all-targets --locked -- -D warnings   # Rust lint (matches CI)
cd src-tauri && cargo nextest run --locked                           # or: cargo test --locked
```

## Tech stack

Tauri 2, Rust, `portable-pty`, React 19, TypeScript, Vite, xterm.js, CodeMirror 6, Vercel AI SDK v6, Tailwind v4, shadcn/ui, Zustand.

## Contributing

Issues and PRs are welcome! Feel free to open issues, suggest features, or submit pull requests. See [CONTRIBUTING.md](CONTRIBUTING.md) and the [architecture docs](docs/README.md) for more details.

## Code signing

<a href="https://signpath.org"><img src="https://avatars.githubusercontent.com/u/34448643?s=200&v=4" width="80" alt="SignPath" align="left" /></a>

Windows builds are signed with a free code signing certificate provided by [SignPath.io](https://signpath.io), certificate by the [SignPath Foundation](https://signpath.org).

<br clear="left" />

## License

Terax is licensed under the Apache-2.0 License. For more information on our dependencies, see [Apache License 2.0](LICENSE).
