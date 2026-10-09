# upstream-and-ssh-ecosystem

## existingSupport
VERIFIED in the clone (0.9.0-beta): there is no SSH feature. The only "ssh" strings in src and src-tauri are the AI secret-path deny-list (src/modules/ai/lib/security.ts:67), git SSH_ASKPASS blanking (git/process.rs:255), the git auth error text (git/errors.rs:74) and "ssh" in the block-editor command list (terminal/block/lib/shellEditor.ts:157). Typing `ssh host` in a Terax terminal works as a plain local PTY command, but the explorer, editor, search, git and AI tools keep operating on the local disk. parseOsc7 drops the hostname, so a remote shell's OSC 7 would be treated as a local path. The abstraction closest to SSH is WorkspaceEnv::{Local, Wsl{distro}} (workspace.rs:313), threaded through pty_open, fs_read_dir, fs grep/glob, shell and lsp, with WSL implemented by wrapping wsl.exe in the local ConPTY (shell_init.rs:605) and by UNC path mapping (workspace.rs:339). Upstream: ROADMAP.md:114 lists SSH as planned, and the owner states he is building it himself and rejects SSH PRs (#101, #276, #391, #392, #812 closed). Open community work: PR #700 (system ssh + ControlMaster + askpass + python helper, conflicting), PR #973, and the stephenstack fork branch feat/ssh-remote-sessions (russh 0.63 + russh-sftp 2.4, SFTP workspaces, remote search/git over POSIX exec, known_hosts, ProxyJump, -L forwards, Remotes panel; Windows prereleases; predates the Ghostty rewrite). Click-to-position cursor: only open issue #455; no code. HTML preview: only open issue #1318; prior art PR #324 closed, the owner says HTML preview is on his list separately and prefers non-iframe designs.

## report
# Upstream research and SSH ecosystem map (as of 2026-10-09)

Clone state: /Users/frider/Documents/iamcoder/terax_claude is Terax 0.9.0-beta, last upstream commit 2026-10-03 (3301de2). `git status --porcelain` is clean. I wrote nothing into the repo. All experiments ran in the scratchpad. Labels below: VERIFIED means read from code, API or a fetched page. MEASURED means I ran it. INFERRED is my reasoning.

## 1. Upstream crynta/terax-ai

### 1.1 Maintainer position on SSH (VERIFIED, gh)
- ROADMAP.md:114 lists "SSH support (PTY auth and known_hosts first; SFTP and port forwarding later)" under "Coming next". There is no SSH branch upstream. `gh api repos/crynta/terax-ai/branches` returns only feat/cli-control, feat/ghostty-webgpu-terminal, feat/default-shell and similar. `git log --all --grep=ssh` finds only unrelated security commits.
- On 2026-06-01 the owner closed every SSH issue as a duplicate of #477 (#79, #351, #285, #235) with: "SSH is on the roadmap as a core feature I'm building into Terax myself". #701 was closed the same way on 2026-06-13.
- The owner closed all contributed SSH PRs and said he will not merge SSH PRs. Quote from #276: "SSH is a reserved core feature I want to design and build myself end to end, so I'm not merging SSH PRs". Same on #101 and #391/#392.
- Before that, on #297 (2026-05-19), he said "SSH itself is planned and we'll likely land it through one of your other PRs", so the stance hardened between 05-19 and 06-01.
- The community fork announcement (discussion #1274 and issue #1276, 2026-09-06/07) has had no maintainer reply.
- For a personal fork this means: expect no upstream SSH to merge against, and a future official SSH would conflict with whatever you build. Keep the SSH code in new modules to limit the conflict surface.

### 1.2 SSH issues and PRs
- #477 "SSH client support" is the canonical open issue: 15 thumbs-up, 7 comments. The most concrete request is jteddy's (2026-06-04): key auth with ~/.ssh/config, a remote folder in the explorer, and the terminal on the remote host. Others echo it. A contributor says "vscode is heavy".
- #705 "Agent Mode over SSH" (6 reactions) lays out three options: A, proxy the agent tools over SFTP and `ssh host cmd`; B, auto-deployed thin bridge binary (like Warp); C, extend the OSC 777 / `pty/agent_detect.rs` hooks over the SSH PTY.
- #851 "Integrate ssh remote develop" (4 reactions), #793 (explorer still shows the local tree after SSH, mac user), #78 (open URLs from SSH sessions locally), #111 (rich previews, ideally for `ssh://` files), discussion #344 (a commenter suggests sshfs as a workaround).
- Closed PRs, each a distinct design:
  - #101: russh + russh-sftp, `ssh://` URL scheme, 2645 lines.
  - #276: russh 0.45 + russh-sftp 2, TOFU host keys, `WorkspaceEnv::Ssh{profile_id}`, 5254 lines.
  - #812: embedded python3 snippets run over ssh for fs operations, auto-detects `ssh` commands typed in the terminal.
  - #391/#392: `ssh://` launch workspace PTY only.
  - #297: SSH LSP.
  - #133: draft SFTP workflow.
- Open PRs:
  - #700 (seannnnnn1017, +7092/-748, 99 files, mergeable=CONFLICTING). Uses the system `ssh` with ControlMaster=auto and ControlPersist=10m, SSH_ASKPASS with `SSH_ASKPASS_REQUIRE=force` for passwords, a python3 remote helper, and embeds Terax's `pty/scripts/*` shell-integration scripts to push OSC 7/133 to the remote. It also bundles unrelated Codex login work.
  - #973 (wh1teend, 229 files): `SSH: <host>` palette entries parsed from ~/.ssh/config, multi-host split tabs with keystroke broadcast.
- Community fork with working russh code: github.com/stephenstack/terax-ai, branch `feat/ssh-remote-sessions`. It is 57 commits ahead and 85 behind upstream main (so it predates the Ghostty rewrite), 149 files, Apache-2.0, with Windows preview prereleases 0.8.6-ssh.2/.3 (Sep 6/12). See section 4.

### 1.3 Most-requested open items (VERIFIED, ranked by reactions)
#121 ACP protocol (29), #477 SSH (15), #9 OpenCode Go provider (14), #419 GitHub Copilot (10), #134 persistent terminal sessions (7; ROADMAP also lists it), #114 themes/extensions (7), #307 vertical tab grouping (6), #705 agent over SSH (6), #189 YOLO/auto-approve (5), #219 split editor with terminal panes (4), #192 paste images into terminal TUIs (3), #853 terminal history (3), #503 copy-on-select (3), #378/#1002 MCP and skills (4), #955/#720 i18n (4+3), #739 multiple windows (2), #915 font ligatures (2), #1319 / PR #1320 move live terminals between tabs and splits. PR #1138 (open) adds an iTerm2-style terminal password manager. PR #825 (open) adds smart Ctrl+C/V.

### 1.4 Your other two asks upstream
- Click-to-position cursor: only #455 (open, 2 reactions, "add autocompletion and mouse cursor positioning"; a commenter asks for mouse cursor positioning). No PR exists.
- Related macOS bug #1009 / PR #956: Option+Arrow and Option+Backspace word navigation is dead in WKWebView because the `keyCode === 229` guard swallows Option-modified keys. The guard still exists in the current tree at src/modules/terminal/ghostty/input/GhosttyInputController.ts:229 (the issue named the old rendererPool.ts). Whether it still drops events in the shipped WKWebView is from the issue text, not retested. Option+click is the standard click-to-move gesture elsewhere, so this bug will interact with it.
- HTML preview: #1318 (open, 0 reactions, "Preview HTML (just like markdown)"). Prior art: PR #324 (closed) added Markdown + HTML preview through the Tauri `protocol-asset` plus an iframe, with relative asset rewriting to blob URLs and live reload. The owner declined the iframe approach for Markdown in favour of #350 (React + Streamdown) because an iframe per tab costs "~5-10MB RAM each", and said "HTML preview ... is on the list for separately". The author offered a separate PR and there was no follow-up. ROADMAP "Previews" already lists a "Sandboxed iframe" for dev-server preview. Existing tab kinds include `preview` and `markdown` (TERAX.md). Anything iframe-based must stay lazy and sandboxed.

## 2. Rust SSH ecosystem (crates.io + GitHub, 2026-10-09)

| Option | Version / activity | Notes |
|---|---|---|
| russh | 0.64.1 (2026-10-05); releases 0.62.7 (Aug 17), 0.63.0 (Aug 21), 0.63.3 (Sep 9), 0.64.0 (Oct 4); 1.9k stars, 71 open issues, ~3.35M recent downloads | Pure Rust, tokio-native, edition 2024, MSRV 1.89. Needs one crypto backend feature (`aws-lc-rs` default, or `ring`). The README lists mlkem768x25519 plus curve25519 kex, chacha20-poly1305, AES-GCM, strict-kex (Terrapin), ed25519/ecdsa/rsa-sha2, OpenSSH certs, PPK, keyboard-interactive, direct-tcpip/forward-tcpip/streamlocal, keepalive. Built-in agent client: `connect_env`, `connect_uds(path)`, Windows named pipe and Pageant. Built-in `known_hosts` check/learn with hashed hosts. Depends on pre-release RustCrypto crates (Cargo.toml pins ssh-key 0.7.0-rc.11, `rsa =0.10.0-rc.18`, `pkcs1 =0.8.0-rc.4`), so dependency resolution can churn. Adopters include Yazi's async SFTP provider and Devolutions Gateway. |
| russh-config | 0.58.0 | Parses Host patterns (with negation), Hostname, Port, User, IdentityFile, ProxyCommand, ProxyJump (string only), AddKeysToAgent, UserKnownHostsFile, StrictHostKeyChecking. Grep of its lib.rs finds no IdentityAgent, Include, Match, CertificateFile or ServerAlive*. So 1Password and Secretive `IdentityAgent` paths and `Match` blocks are not handled. |
| russh-sftp | 3.0.1 (2026-09-28), repo pushed 2026-10-07 | Transport-agnostic: it takes any AsyncRead+AsyncWrite, so it also works over a child process's stdio. SFTP v3 only. Extensions: limits, hardlink, fsync, statvfs, expand-path, but no posix-rename, so overwrite-rename is not atomic. README benchmark: 257/203 MiB/s up/down. |
| ssh2 (libssh2) | 0.9.6 (2026-06-30), 8.1M downloads | Blocking API (needs spawn_blocking). On macOS the practical build is `vendored-openssl`. libssh2 does not parse ssh_config and has no ProxyJump (INFERRED from API, general knowledge), so jump hosts need manual socket plumbing. Cert/sk-key support is partial; libssh2 1.11 added sk-ed25519 per its changelog. |
| openssh + openssh-sftp-client | 0.11.6 (2025-12-03, badge `maintenance = experimental`); sftp-client 0.15.10 (2026-10-01) | Wraps system `ssh` with ControlMaster. Unix only: it uses libc and Unix control sockets, and Zed's source notes Win32-OpenSSH lacks ControlMaster. Gets full ssh_config, Match, Include, ProxyJump and every agent for free. |
| System ssh/sftp binaries | OpenSSH_10.5p1 on this Mac | Zero binary cost, full config. The cost is prompt handling: host-key, password and 2FA prompts need a TTY or SSH_ASKPASS. |
| Others | async-ssh2-tokio 0.13.0 (a russh wrapper), thrussh 0.49.0 (russh's ancestor, ~7k recent downloads, superseded), libssh-rs 0.3.8 (wez; needs the libssh C library, 48k recent downloads), ssh2-config 0.8.1 (parser, no Match) | Not recommended. |

### Measured binary cost (MEASURED)
Synthetic programs in the scratchpad, outside the repo. aarch64-apple-darwin, rustc 1.97.1, profile identical to src-tauri/Cargo.toml:112-118 (lto fat, opt-level s, panic abort, strip). Each program exercises connect, agent auth, PTY shell, SFTP readdir and direct-tcpip, so the linker cannot drop the code.

| Build | Stripped size | Delta vs tokio-only |
|---|---|---|
| tokio-only baseline | 371 KB | n/a |
| russh-sftp 3.0.1 over a child `ssh -s sftp` stdio | 538 KB | +167 KB |
| openssh (native-mux) + openssh-sftp-client | 639 KB | +268 KB |
| russh =0.64.1 [ring, flate2, rsa] + russh-sftp + russh-config | 1,995 KB | +1.62 MB |
| same with aws-lc-rs | 2,295 KB | +1.92 MB |
| ssh2 0.9.6 + vendored-openssl (no tokio) | 3,715 KB | about +3.3 MB |

The app already links `ring` 0.17.14 (src-tauri/Cargo.lock:3618) and rustls 0.23.40 through reqwest's rustls-tls, and has no aws-lc-* crate. So russh must use `ring`, which adds no second crypto C library. The real marginal cost is below the table's russh figure. CONTRIBUTING.md:86 sets the ">5MB compiled on Rust side" justification threshold, and none of these cross it. The stephenstack fork uses russh 0.63 `ring` + `flate2` and no `rsa` feature, so RSA keys do not work in that fork.

### Criteria summary
- ~/.ssh/config: system ssh is complete. `ssh -G <host>` prints the fully resolved effective config, including Include, Match, ProxyJump and IdentityAgent. I ran it here and it works (it also showed that your config already has `ControlMaster auto`, `ControlPersist 1800`, `ServerAliveInterval 60` and `ControlPath ~/.ssh/master-%r@%h:%p` applied globally). russh needs `ssh -G` or a hand-rolled parser to match.
- Agents (1Password/Secretive): macOS GUI apps do not inherit a shell's SSH_AUTH_SOCK (1Password's own docs and Bitwarden guides). System ssh honours IdentityAgent natively. In-process russh must call `connect_uds` with the IdentityAgent path. The community fork only uses `AgentClient::connect_env()` on Unix (ssh/agent.rs), so it would miss a 1Password IdentityAgent unless SSH_AUTH_SOCK is exported to the GUI. Terax already has a login-shell env capture at src-tauri/src/modules/lsp/env.rs:47, but only for LSP.
- known_hosts: russh has `check_known_hosts`, `learn_known_hosts` and hashed hosts, so a custom TOFU dialog is straightforward (the fork's ssh/hostkey.rs is 284 lines). System ssh uses its own prompt.
- Keyboard-interactive/2FA: russh supports it at API level but you must build the UI. System ssh needs a PTY or askpass.
- Platforms: russh and russh-sftp work on macOS, Linux and Windows. System ssh with ControlMaster is Unix only.

## 3. How comparable tools do remote trees

- VS Code Remote-SSH (VERIFIED, docs): uses the local OpenSSH client. It installs a VS Code Server on the remote (downloaded by the remote first, else uploaded from local). The remote needs glibc 2.17+, 1-2 GB RAM, bash, tar and curl/wget, and Alpine is unsupported. Extensions, terminals, file watching, search and git all run remotely, and traffic goes through the SSH tunnel.
- Zed (VERIFIED, docs): shells out to local `ssh` and starts one ControlMaster per project, then multiplexes its protocol, terminals and tasks over it. It installs a version-locked headless server in `~/.zed_server`. `upload_binary_over_ssh` is available. Proxy mode keeps a daemon alive for reconnect. No SFTP. Supports -L/-R/-J and graphical askpass. Weak on very large directories (over 100k files).
- Warp (VERIFIED, docs via search): an "SSH extension" in `~/.warp/remote-server`, installed only with consent, gives the remote file tree, code editing and review. Without it the session still works but those features are off. The older tmux-based Warpify wrapper needs bash/zsh/fish. Warp's docs say ControlMaster fails if the server has MaxSessions below 2.
- Wave (VERIFIED, docs): `wsh` installed at `~/.waveterm/bin/wsh` on the remote enables preview and files widgets. Parses ssh config except `Match`. Supports ProxyJump chains and IdentityAgent (named pipe on Windows). Fails if the server sets AllowTcpForwarding no.
- Tabby: built-in SFTP transfer inside SSH tabs, plus an optional SFTP-tab plugin. Not a remote workspace. WindTerm: a "filer" SFTP panel that syncs with the terminal cwd (docs). Termius: SFTP panel (from background knowledge, not verified here). Yazi: russh-based SFTP provider.

### SFTP-only vs remote helper binary
- SFTP-only (plus exec for search and git):
  - Pros: nothing to install on the server, works on BusyBox/Alpine, no version-lock handshake, much less code.
  - Cons: no change notifications, so the tree and open buffers go stale. No remote git decorations. Walking a tree costs one round trip per directory, so global search must be delegated to `find`/`grep` or `rg` over exec. SFTP v3 mtimes are whole seconds, and overwrite-rename is not atomic.
  - The community fork's own docs (docs/ssh-remote.md) accept all of these: no watching, no git decorations, no LSP, search and git via POSIX exec.
- Helper binary (Zed, Warp, Wave, VS Code):
  - Pros: native inotify watching, fast batched list/search/grep, git status, one logical RPC per operation (lower latency), a place to host remote LSP and agent tools.
  - Cons: per-arch static binaries to build and ship (Linux x86_64/aarch64 at least), a version-match handshake, a trust and consent step, and a remote filesystem write. Terax already has a `crates/` workspace (terax-cli, terax-control-protocol) and shares `ignore`, `grep-*` and `notify`, so a `terax-remote` crate could reuse the local fs/search/grep/watch code. That is real but non-trivial work.
- INFERRED: for a single-user fork, agentless SFTP+exec is enough to start. Design a `RemoteFs` trait so a helper can be swapped in later.

## 4. What the community fork already provides (VERIFIED by reading its files)
Rust modules `ssh/{agent,config,hostkey,mod,session,target}.rs` and `remote/{conn,exec,forward,fs,mod,path}.rs`, plus a TS `src/modules/remotes/` panel (host profiles, groups, ssh_config import, transfer dialog, tunnels).
- Auth order list, Windows agent via named pipe or Pageant, key files with passphrase prompts, keyboard-interactive, TOFU known_hosts with changed-key warnings (certificates refused), ProxyJump chains, local `-L` forwards, and no remote `-R`.
- SFTP for file I/O, POSIX `find`/`grep`/`git` over exec for search and source control, mutations confined to the opened root, AI shell commands routed to the remote host.
- Design flaw in `ssh/session.rs`: its `pump` (lines 423-505) drops bytes when pending output exceeds a cap ("ssh backpressure: dropped N bytes"). It also opens a separate `ssh_open` command set and bypasses Terax's PTY ack protocol.
- Its remote write is non-atomic (remote/fs.rs:143-154).

## 5. Terax seams that decide the design (VERIFIED)
- The PTY pipeline has credit-based flow control: `OutputCredit` in src-tauri/src/modules/pty/output.rs (max 2 in-flight chunks, 2 MiB) with cumulative acks via `pty_ack_output` (pty/mod.rs:164). It also has `agent_detect.rs`, resize and foreground-process checks. Anything that is not a local PTY child bypasses all of that.
- WSL is the precedent for "wrap a transport binary inside the local PTY": `WorkspaceEnv::{Local, Wsl{distro}}` at src-tauri/src/modules/workspace.rs:313-319. `shell_init::build_command` (pty/shell_init.rs:53) dispatches per platform, and the Windows module's `build_wsl` (shell_init.rs:605) launches `wsl.exe -d <distro>` in ConPTY. One-shot AI commands use the same idea in `build_oneshot_command` (shell/mod.rs:284-300).
- Filesystem commands take `workspace: Option<WorkspaceEnv>` (fs/tree.rs:138, fs/grep.rs:179/220/271) but then call `resolve_path` (workspace.rs:339-349). That returns a local `PathBuf` and uses `std::fs`, which only works for WSL because Windows exposes UNC paths. A remote backend needs a provider abstraction, not path mapping.
- Frontend: `WorkspaceEnv` TS union at src/modules/workspace/env.ts (`local` or `wsl`), read through `currentWorkspaceEnv()` at about 59 call sites. `pty_open` is invoked from src/modules/terminal/lib/pty-bridge.ts:103 and `fs_read_dir` from src/modules/explorer/lib/useFileTree.ts:181.
- `parseOsc7` (src/modules/terminal/lib/osc-handlers.ts:5-21) discards the URL host (`file://[^/]*`). A remote shell's OSC 7 is therefore treated as a local path, which is the cause of the "explorer shows local files after ssh" reports (#235, #793). The hostname is the signal to detect "this terminal is remote".
- A remote shell does not get Terax's OSC 7/133 init scripts (they are injected locally by ZDOTDIR and rc files). Remote cwd tracking needs a bootstrap, as #700 (embedded scripts) and the fork (`shellIntegration.ts`, `echoSuppressor.ts`) both found.
- Cargo.toml:63 has tokio with only the `rt` feature, so SSH code must add `process`, `io-util`, `net`, `time` and `sync`. `portable-pty` is at Cargo.toml:42 and `which` at :66.

## 6. Recommendation (personal macOS fork that still compiles elsewhere)

Hybrid, system-ssh first:
1. Config: resolve each host with `ssh -G <alias>` (spawn with `which`), falling back to russh-config only if no ssh binary exists. This gives Include, Match, ProxyJump, IdentityAgent and your existing ControlMaster settings for free.
2. Terminal: add `WorkspaceEnv::Ssh{host,...}`, with a `build_ssh()` command built in `pty/shell_init.rs` like `build_wsl`, spawning `ssh -tt <alias>` inside the normal local PTY. This keeps ack flow control, agent detection, selection and search, and the whole Ghostty path unchanged. Auth prompts (host key, password, 2FA, touch) appear in the terminal. Use ControlMaster/ControlPath so file operations can reuse the connection (your config already enables it). Keep ControlPath short, because macOS socket paths are limited to about 104 bytes (INFERRED) and `$TMPDIR` is long.
3. Remote workspace file provider: a `RemoteFs` trait with an SFTP implementation using russh-sftp over `ssh -S <ctl> -s <host> sftp` child stdio (+167 KB measured, no crypto in the binary). Search, grep and git use `ssh -S <ctl> <host> -- find/grep/git` (reuse the fork's POSIX command builders and quoting tests). Refresh the explorer by polling or on user action. Save via write-temp then `mv -f` over exec to avoid SFTP's non-atomic rename. Remember SFTP mtimes are whole seconds when checking editor conflicts.
4. Guardrails for cross-platform builds: put everything that needs ControlMaster behind `#[cfg(unix)]`, with a non-unix stub that returns a clear error, so Linux and Windows builds still compile. Optionally add a cargo feature `ssh-native` that uses russh 0.64.x with `ring`, `flate2` and `rsa`, pinned exactly (`=0.64.1`), for Windows and for hosts without a usable ControlMaster. Port the fork's `ssh/hostkey.rs`, `ssh/agent.rs` (add `connect_uds` for IdentityAgent), `remote/exec.rs` and `remote/fs.rs` there. Do not port its `pump` backpressure drop, and bridge the SSH channel into `OutputCredit` instead.
5. Defer the helper binary. Revisit it only if polling or `find` search latency hurts on your servers.

Why not russh-only: roughly +1.6 MB, a hand-built prompt UI, a hand-built ssh_config layer with IdentityAgent, and a second terminal transport that must re-implement Terax's flow control. Why not ssh2: +3.3 MB, blocking, no ProxyJump. Zero-code stopgap: sshfs/macFUSE (suggested in discussion #344) but I did not verify it on current macOS.

Suggested work slices (for the planner): (a) Rust: `WorkspaceEnv::Ssh` variant, `ssh -G` resolver, `build_ssh`, ControlMaster lifecycle, SFTP-over-stdio provider, exec runner, with invariant tests (core subsystem rule in TERAX.md). (b) Rust: route `fs_*`, `fs_search`/`fs_grep`, git and `shell_run_command` through the provider. (c) TS: SSH workspace type, host picker, OSC 7 host detection, status bar indicator, explorer/editor wiring. (d) tests and docs. Rust transport and fs routing are the high-judgement parts. TS wiring, host-list UI and test writing are mechanical.

## keyFiles
- /Users/frider/Documents/iamcoder/terax_claude/ROADMAP.md: Line 114 states the planned SSH scope (PTY auth and known_hosts first, SFTP and port forwarding later); also the out-of-scope list (no heavyweight IDE infra) that bounds helper-binary and LSP ideas.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/Cargo.toml: Line 63 tokio has only the rt feature; release profile at lines 112-118 (lto fat, opt-level s, panic abort, strip) used for the size measurements; portable-pty at 42; reqwest rustls-tls at 57.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/Cargo.lock: Line 3618 ring 0.17.14 already linked and no aws-lc-* crate, which is why russh must use the ring feature.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/workspace.rs: Lines 313-329 define WorkspaceEnv {Local, Wsl}; lines 339-349 resolve_path maps to a local PathBuf, the model that a remote backend cannot reuse.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/shell_init.rs: Line 53 build_command dispatch; unix::build at 311 and the Windows build_wsl at 605 are the precedent for launching a transport binary (ssh) inside the local PTY.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/mod.rs: pty_open (lines 46-110) takes workspace: Option<WorkspaceEnv>; pty_ack_output at 164, pty_resize 176, pty_close 207, pty_has_foreground_process 237.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/output.rs: OutputCredit flow control (2 chunks, 2 MiB, cumulative acks) that any SSH transport must honour instead of dropping bytes.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/session.rs: spawn() at about line 281 builds the command and wires the reader, OutputQueue and killer; Session struct at 177.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/tree.rs: fs_read_dir (line 138) shows the std::fs + resolve_path pattern every fs command uses; the same applies to file.rs, mutate.rs, search.rs, grep.rs and watch.rs.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/shell/mod.rs: build_oneshot_command (lines 284-300) is where WSL wraps AI commands in wsl.exe; the ssh analogue would wrap ssh -S <ctl> <host> -- <cmd>.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/lsp/env.rs: Line 47 capture_login_env: existing login-shell env capture, a reusable source for SSH_AUTH_SOCK on a GUI-launched app.
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/lib.rs: Line 253 generate_handler! is where new ssh and remote commands must be registered.
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/workspace/env.ts: TS WorkspaceEnv union (local|wsl) and the zustand store; currentWorkspaceEnv() has about 59 call sites.
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/lib/osc-handlers.ts: parseOsc7 (lines 5-21) discards the URL host, so remote OSC 7 is misread as a local cwd; the hook for detecting a remote terminal.
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/lib/pty-bridge.ts: Line 103 frontend invoke of pty_open; where a workspace kind would be passed.
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/explorer/lib/useFileTree.ts: Line 181 invoke of fs_read_dir; the explorer data path to be pointed at a remote provider.
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/input/GhosttyInputController.ts: Line 229 keyCode 229 guard (issue #1009, Option-key events dropped on macOS) and mouse handlers at about 405-431 (mouse events only forwarded in mouse-tracking mode), relevant to click-to-move cursor.
- /Users/frider/Documents/iamcoder/terax_claude/CONTRIBUTING.md: Line 86: new Rust dependencies over about 5MB compiled need justification; all measured SSH options are under that.
- https://github.com/stephenstack/terax-ai/tree/feat/ssh-remote-sessions: Community Apache-2.0 reference implementation (ssh/*.rs, remote/*.rs, src/modules/remotes/*, docs/ssh-remote.md) worth porting selectively; predates the Ghostty rewrite and has a byte-dropping pump in ssh/session.rs:423-505.
- https://github.com/crynta/terax-ai/pull/700: Open PR showing the system-ssh ControlMaster + SSH_ASKPASS + embedded shell-integration-script approach and its python3 helper.
- /private/tmp/claude-501/-Users-frider-Documents-iamcoder-terax-claude/c2d597b7-b40d-4501-a701-a769049d2877/scratchpad/sizetest: Scratch crates (base, sftp_only, osc, russh_ring, russh_aws, ssh2_vend) used for the binary-size measurements; outside the repo.

## integrationPoints
- src-tauri/src/modules/workspace.rs:313 add WorkspaceEnv::Ssh{host, ...} and mirror it in src/modules/workspace/env.ts (union, scope key, parse); resolve_path stays Local-only and remote fs gets a separate provider.
- src-tauri/src/modules/pty/shell_init.rs:53 build_command: add an Ssh branch (unix::build at :311, modelled on windows::build_wsl at :605) that spawns `ssh -tt <alias>` with ControlMaster/ControlPath/ControlPersist inside the existing portable-pty session so pty/output.rs OutputCredit, agent_detect and resize all keep working. Do not copy the fork's separate ssh_open pump.
- src-tauri/src/modules/pty/mod.rs:46-110 pty_open already receives workspace; also gate the control_env injection (like the WSL check at :66) because a local control helper cannot run on the remote.
- New module src-tauri/src/modules/remote (or ssh): ssh -G resolver, ControlMaster lifecycle (ssh -O check/exit), RemoteFs trait with SFTP-over-child-stdio via russh-sftp, exec runner; register commands at src-tauri/src/lib.rs:253 generate_handler!; add tokio features process, io-util, net, time, sync (Cargo.toml:63).
- src-tauri/src/modules/fs/{tree,file,mutate,search,grep,watch}.rs: early-branch on WorkspaceEnv::Ssh to the provider before the std::fs path; keep authorization and secret deny-list checks (workspace_authorize registry, ai/lib/security.ts) applying to remote paths. Watch.rs needs a polling fallback for remote.
- src-tauri/src/modules/shell/mod.rs:284 build_oneshot_command and shell/session.rs:129 wrap_with_sentinel: add the ssh equivalent of the WSL branch so AI run_command, shell_session and shell_bg run on the remote host.
- src/modules/terminal/lib/osc-handlers.ts:5 parseOsc7: capture the URL host; when it is not the local hostname, mark the terminal/tab as remote and drive the explorer root from the remote cwd instead of the local one.
- src/modules/terminal/lib/pty-bridge.ts:103 and src/modules/explorer/lib/useFileTree.ts:181: pass the ssh workspace through pty_open and fs_read_dir; add tab and status-bar indicator and a host picker (ssh -G or ~/.ssh/config Host list via command palette, cf. PR #973).
- Remote shell integration bootstrap: reuse src-tauri/src/modules/pty/scripts/* (zshenv, bashrc, init.fish) as in PR #700 so the remote shell emits OSC 7/133; needs echo suppression like the fork's remotes/lib/echoSuppressor.ts.
- Optional cargo feature ssh-native: russh =0.64.1 [ring, flate2, rsa] + russh-sftp 3.0.x, importing selected modules from the stephenstack fork (hostkey.rs, agent.rs with connect_uds for IdentityAgent, remote/exec.rs POSIX find/grep builders), bridged into OutputCredit; keep all ControlMaster code under cfg(unix) with a non-unix stub so Linux and Windows builds still compile.

## risks
- Upstream conflict: the owner reserves SSH as a core feature he will build himself and rejects SSH PRs; a future official SSH will conflict with this work. Mitigation: isolate in new modules and a WorkspaceEnv variant.
- Fork code is stale: stephenstack's branch is 85 commits behind and predates the libghostty rewrite, so only Rust remote/ssh modules and the Remotes panel are portable; its terminal glue (xterm era, echoSuppressor, separate ssh_open pump) is not, and its pump drops bytes under backpressure.
- System-ssh path is Unix only: ControlMaster is unsupported on Windows OpenSSH; keep cfg(unix) gating and a stub so non-mac builds compile. Prompt handling (password, 2FA, host key) must happen in the terminal or via an askpass helper; PR #700 stores passwords for SSH_ASKPASS which is a security smell.
- macOS ControlPath socket paths are limited to about 104 bytes (inferred), and $TMPDIR is long; use a short dir such as ~/.ssh/terax-cm/%C. Servers with MaxSessions below 2 or AllowTcpForwarding no break multiplexing, jump hosts and some helpers (Warp and Wave docs).
- russh churn: a release every 1-4 weeks, pre-release RustCrypto pins (ssh-key 0.7.0-rc.11, rsa =0.10.0-rc.18), and no IdentityAgent/Include/Match in russh-config; pin exactly and treat upgrades as deliberate. Without the rsa feature (as in the fork) RSA keys fail. OpenSSH 10.5 on this Mac lists newer identity types (id_mldsa44_ed25519) that russh may not support (inferred).
- GUI-launched macOS apps do not inherit SSH_AUTH_SOCK; with in-process russh, 1Password or Secretive keys need the IdentityAgent socket path from ssh -G or the captured login env, otherwise auth silently falls through.
- SFTP v3 limits: whole-second mtimes (conflict check in the editor uses milliseconds), no atomic overwrite rename (russh-sftp lacks posix-rename), no change notifications, per-directory round trips make tree-walk search unusable without exec delegation; remote find/grep/git depend on tools being installed and on git 'dubious ownership' rules.
- Terminal fidelity: the remote shell lacks Terax's OSC 7/133 injection, so cwd tracking and command blocks degrade to a bare shell unless a bootstrap is injected; parseOsc7 currently misattributes remote cwds to the local disk.
- Existing invariants to lock with tests (TERAX.md: core subsystem changes need an invariant test): workspace authorization registry, secret-path deny-list on remote paths, PTY output credit, process cleanup on RunEvent::Exit (ControlMaster and sftp children must be killed).
- Binary budget: russh ring adds about 1.6 MB measured (less here since ring is already linked), ssh2 vendored about 3.3 MB; both under the CONTRIBUTING 5 MB threshold but russh is about 20 percent of the 7-8 MB app.

## openQuestions
- Transport choice: system ssh with ControlMaster (recommended, macOS/Linux only) versus in-process russh (all platforms, ~+1.6 MB) versus both behind a cargo feature? Does Windows support matter at all for this fork?
- Auth UX: is it acceptable that password, 2FA and host-key prompts appear in the Terax terminal running ssh, or do you want a native dialog (SSH_ASKPASS helper through the existing terax-cli / control protocol, as Zed does)? Are you key/agent-only, and do you use 1Password or Secretive?
- Should the file tree connect independently of any terminal (a 'Connect to host' action that owns a ControlMaster, like VS Code), or follow the active ssh terminal tab (WindTerm style)? Likely both; which is first?
- Is polling-based refresh acceptable (no remote file watching), and are remote servers always Linux with find/grep/git available? Is there appetite for a terax-remote helper binary later (arch matrix, version handshake)?
- How should remote shell integration reach the server: inject Terax's OSC 7/133 scripts on login (PR #700 / fork style), rely on the user's own remote prompt emitting OSC 7, or skip blocks on remote?
- Scope on remote: AI tools (run_command, read/write), source control, port forwarding (-L), jump hosts, tab restore for remote tabs, LSP? The fork excludes LSP and remote -R; ROADMAP treats LSP as opt-in.
- Do you want the SSH code kept as an isolated module/feature to ease rebasing, given a possible official upstream SSH later? And rebase policy on the 0.9.0-beta Ghostty line versus the 0.8.6-based community fork?
- For the non-SSH asks: confirm click-to-move design (Option+click like iTerm2/Ghostty versus plain click on the prompt line using OSC 133 B/cursor distance) and whether HTML preview should be an iframe-free sandboxed srcdoc/asset view (maintainer preference) versus the PR #324 asset-protocol iframe.
