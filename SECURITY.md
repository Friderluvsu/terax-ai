# Security

Terax runs shells, reads/writes files, and talks to AI providers, so security bugs matter. If you find one, please tell us before posting it publicly.

## Reporting

Check [this repository's Security tab](https://github.com/Friderluvsu/terax-ai/security) for private vulnerability reporting options. If private reporting is unavailable, contact [the maintainer](https://github.com/Friderluvsu) privately to agree on a reporting channel before sharing exploit details. Include:

- What the issue is and what it lets an attacker do
- Steps to reproduce (a small PoC is great)
- Version, OS, arch

Please **don't** open a public GitHub issue for security reports.

## Supported versions

Development and fixes target the current code in this repository. See the current version in [package.json](package.json).

## What's in scope

- The Rust backend in `src-tauri/` (PTY, FS, IPC, plugins)
- The frontend in `src/` - anywhere untrusted input lands (terminal output, file content, AI tool results, credentials)
- Locally built application bundles and packaging scripts

## What's not

- Bugs in upstream dependencies (Tauri, Ghostty, CodeMirror, AI SDKs) - report those upstream as well.
- Anything that needs an already-compromised machine or a local attacker with shell access

## What we do to keep things safe

- **API keys** live in the OS keychain via `keyring` - not on disk, not in `localStorage`, not in logs.
- **No telemetry.** Network features include AI requests, SSH workspaces and web preview.
- **AI tool approval.** File writes and shell commands from the agent need your OK before they run.
- **No Node in the renderer.** The frontend only reaches the host through the allow-listed Tauri commands.
- **Manual updates.** The built-in updater is disabled for this application identity. Local macOS builds are ad-hoc signed and are not notarized.

## What we can't promise

- Terax runs whatever you (or the agent) tell it to run, with your permissions. That's kind of the point of a terminal.
- AI providers see whatever you send them. Read their retention policies.
- Local LLM endpoints (LM Studio, OpenAI-compatible) are trusted at the network level - only point Terax at servers you control.
