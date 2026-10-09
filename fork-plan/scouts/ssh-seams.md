# ssh-seams

## existingSupport
Verified: there is no SSH support anywhere (no ssh crates in Cargo.lock, no ssh code paths). The only non-local environment is WSL (Windows-only):
- WorkspaceEnv is the tagged union {local | wsl{distro}} in TS (src/modules/workspace/env.ts:5-7) and Rust (workspace.rs:311-329).
- It flows as a `workspace` argument on every fs, git, shell, pty and lsp invoke, read from a single global zustand store (currentWorkspaceEnv()).
- Rust maps WSL file paths to host UNC paths in resolve_path (workspace.rs:338-349, 399-428) and runs processes via `wsl.exe -d <distro> --cd <cwd> --exec ...` (git/process.rs:307-332, shell/mod.rs:284-299).
- PTY shell integration for WSL writes the embedded OSC 7/133 scripts into the guest's ~/.cache/terax/shell-integration and launches `env ZDOTDIR=... <shell> -l` inside the guest (shell_init.rs:605-795). This is a direct template for remote shell integration.
- Spaces persist `env` per space (spaces/lib/store.ts:5-14). Editor tabs dedupe by (spaceId, path) (useTabs.ts:205-216), so a space-scoped env gives a sound tab identity.
- The explorer root already follows the terminal's OSC 7 cwd (useWorkspaceCwd.ts:22-28).
- LSP is already disabled for non-local envs on the frontend (sessionManager.ts:67).
- Gaps:
  - No frontend FsProvider exists. The closest facade is `native` (ai/lib/native.ts:139-384), used by AI and git only.
  - The WorkspaceRegistry is local-path-only (std::fs::canonicalize). It gates PTY, shell, lsp, git, fs_move, fs_delete_batch and fs_watch_add, but not read_dir, read_file, write_file, stat, create, rename, delete, copy, search or grep.
  - On macOS, resolve_path and build_command ignore the env entirely, so a new variant would fail open to local IO and local shells.

## report

# Remote (SSH) workspace: seam map for Terax

Scope: explorer, editor and terminal working against a remote host, using the WSL environment as the model. **[V]** = verified in code. **[I]** = inferred or design proposal.

---

## 1. The environment model today

### 1.1 Frontend (`src/modules/workspace/env.ts`)
- **[V]** `WorkspaceEnv = {kind:"local"} | {kind:"wsl"; distro}` (env.ts:5-7).
  - It lives in one **global** zustand store, `useWorkspaceEnvStore` (env.ts:26-46).
  - `currentWorkspaceEnv()` (env.ts:48) returns whatever env is active.
- **[V]** Scope keys come from `workspaceScopeKey` / `parseWorkspaceScopeKey` (env.ts:52-60).
  - Both are two-way ternaries. Any unknown kind maps to `"local"` / `LOCAL_WORKSPACE`, so a new `ssh` kind would silently collapse to local.
- **[V]** Other ternaries that would also misroute `ssh` to local:
  - `useWorkspaceSwitcher.ts:11-15` (`resolveEnvHome` would return the local `homeDir()`).
  - `useWorkspaceSwitcher.ts:76-82` (the "same env" check).
  - `activeSpace.ts:24-31` (`freshTabCwd`).
- **[V]** Who sets the env:
  - On a space switch, `adoptWorkspaceEnv(meta.env)` runs with no teardown (App.tsx:278-298, useWorkspaceSwitcher.ts:115-128).
  - An interactive switch (`switchWorkspace`) disposes sessions, re-authorizes home and resets tabs (useWorkspaceSwitcher.ts:74-113, App.tsx:251-258).
  - Boot calls `adoptWorkspaceEnv` for the active space (useSpacesBoot.ts:94-97).
- **[V]** The env picker UI is Windows-only (`WorkspaceEnvSelector.tsx:22-24`). On macOS there is no env UI at all.

### 1.2 Rust (`src-tauri/src/modules/workspace.rs`)
- **[V]** `enum WorkspaceEnv { Local, Wsl { distro } }` derives `Deserialize` only, with `#[serde(tag="kind", rename_all="lowercase")]` (workspace.rs:311-329).
  - Every fs, git, shell, pty and lsp command takes `workspace: Option<WorkspaceEnv>` and calls `WorkspaceEnv::from_option`.
- **[V]** How WSL paths are reached:
  - **Files:** host-visible UNC paths, resolved in `resolve_path` (workspace.rs:338-349).
    - `\\wsl.localhost\<distro>\...`, falling back to `\\wsl$\...` (`wsl_path_to_unc`, workspace.rs:399-420).
    - `/mnt/<drive>` maps back to `X:\` (`wsl_drvfs_to_windows`, workspace.rs:378-396; `wsl_path_to_host`, workspace.rs:423-428).
  - **Processes:** run through `wsl.exe -d <distro> --cd <cwd> --exec ...`:
    - git: `git/process.rs:307-332`
    - AI one-shot shell: `shell/mod.rs:284-299`
    - probes: `wsl_exec_capture`, workspace.rs:471-490
    - PTY: `shell_init.rs:605-730`
- **[V] Critical fail-open on macOS and Linux:**
  ```rust
  #[cfg(not(windows))]
  pub fn resolve_path(path: &str, _workspace: &WorkspaceEnv) -> PathBuf { PathBuf::from(path) }
  ```
  (workspace.rs:346-349). Adding an `Ssh` variant would make every fs command quietly read and write the **local** disk at the remote path string.
- **[V]** Other places that would also fail open:
  - `shell_init::build_command` unix arm ignores `workspace` entirely (`let _ = workspace;`, shell_init.rs:61-65). It would spawn a local shell.
  - `build_oneshot_command` unix arm ignores `workspace` (shell/mod.rs:308-322). AI `run_command` would execute locally.
  - `lsp_spawn` only rejects `is_wsl()` (lsp/mod.rs:73-75).
  - `pty_open` decides whether to inject the control env with `is_wsl()` (pty/mod.rs:66-70). An `Ssh` variant would forward `TERAX_CONTROL_TOKEN`.
  - `shell/mod.rs:94` and `shell/background.rs:108` use `if let (Local, Some(dir))`.
  - `shell/session.rs:130` uses `is_wsl()` for the POSIX sentinel.
  - `fs/search.rs:237` and `fs/grep.rs:331` use `is_wsl()` for display paths.
  - `git/utils.rs:41` uses `is_wsl()`.
  - Only **two** exhaustive `match`es exist (git/process.rs:47-50 and the Windows-only workspace.rs:340-343), so the compiler will not flag most of these.

### 1.3 Workspace authorization registry
- **[V]** `WorkspaceRegistry { roots: Mutex<HashSet<PathBuf>>, canonical_cache }` (workspace.rs:19-71).
  - `authorize()` calls `std::fs::canonicalize`, which is local-only.
  - `is_authorized()` checks `starts_with` against any root.
  - It is bootstrapped with the launch dir and home (workspace.rs:135-140) plus the CLI dir (lib.rs:243-250).
- **[V]** Commands:
  - `workspace_authorize(path, workspace)` (workspace.rs:142-152) adds any existing directory the webview asks for.
  - `workspace_current_dir` (workspace.rs:154-161).
- **[V]** Spawn helpers:
  - `authorize_spawn_cwd` rejects paths outside the roots (workspace.rs:75-96).
  - `authorize_user_spawn_cwd` registers the path (workspace.rs:100-116).
  - `user_spawn_cwd_or_home` falls back to home (workspace.rs:120-133).
- **[V] Gated by the registry:**
  - `pty_open` (pty/mod.rs:63)
  - `shell_run_command` / `shell_session_open` / `shell_session_run` / `shell_bg_spawn` (shell/mod.rs:55, 179, 214, 242)
  - `lsp_spawn` (lsp/mod.rs:76)
  - every git op through `authorized_repo_root` / `canonical_dir` (git/utils.rs:29-63, operations.rs:28, 90, 126...)
  - `fs_move` and `fs_delete_batch` (mutate.rs:39-56, 283, 363)
  - `fs_watch_add` (watch.rs:221)
  - control `open` (control.rs:372-404)
- **[V] Not gated:**
  - `fs_read_dir`, `list_subdirs`, `fs_read_file`, `fs_write_file`, `fs_stat`, `fs_canonicalize`
  - `fs_create_file`, `fs_create_dir`, `fs_rename`, `fs_delete`, `fs_copy`
  - `fs_search`, `fs_list_files`, `fs_grep`, `fs_grep_interactive`, `fs_glob`
  - Their only guard is the frontend AI deny-list (`ai/lib/security.ts`), and only for AI calls.
- **[V]** Frontend callers of `workspaceAuthorize` all use the **global** env:
  - every OSC 7 cwd (App.tsx:1075-1086, with an `authorizedCwds` set keyed by path only)
  - home on switch (useWorkspaceSwitcher.ts:48, 68)
  - all restored cwds of **all spaces** under the active space's env (useSpacesBoot.ts:105-107). This is a latent cross-env bug already.

---

## 2. Explorer data source

- **[V]** `useFileTree(rootPath)` (explorer/lib/useFileTree.ts):
  - Subscribes to the global env (line 104) and builds `scopeKey = workspaceKey\0root` (line 121).
  - `fetchChildren` calls `fs_read_dir {path, showHidden, gitDecorations, workspace}` (lines 176-239).
  - The root-change effect is keyed on `rootPath` only (lines 243-277). An env change with the same root does not refetch.
  - `expansionCache` is keyed by root path only (lines 57-76), so local and remote paths can collide.
  - Mutations:
    - create: lines 391-394
    - `fs_rename`: line 424
    - `fs_delete`: line 443
    - `fs_delete_batch` with root: line 462
    - `fs_move` with root and expectedConflict: line 513
- **[V]** `watch.ts:9-32`: `fs_watch_add` / `fs_watch_remove` with the global env, plus a listener for the global event `fs:changed {paths}`. There is **no env tag** in the payload.
- **[V]** `useGitStatus.ts:16-38`: decorations reuse the source-control snapshot. It calls `native.canonicalize(root)` (`fs_canonicalize`) with the global env.
- **[V]** Explorer root = the active terminal tab's cwd from OSC 7, else the last terminal cwd, else any terminal tab, else home (`tabs/lib/useWorkspaceCwd.ts:22-28`).
  - So with a remote PTY that emits OSC 7, the explorer follows the remote cwd automatically.
- **[V]** Other explorer paths:
  - Drag-drop from Finder: `fs_copy {sources (local), destDir, workspace}` (useExplorerFileDrop.ts:72). Its remote equivalent is an upload.
  - `revealItemInDir` (contextActions.ts:17-22) is local-only.

### Rust fs command signatures [V]
| Command | File:line | Signature | Sync/async |
|---|---|---|---|
| fs_read_dir | tree.rs:134-214 | (path, show_hidden, git_decorations?, workspace?) -> Vec<DirEntry{name,kind,size,mtime,gitignored}> | sync |
| list_subdirs | tree.rs:221-248 | (path, show_hidden, workspace?) -> Vec<String> | sync |
| fs_read_file | file.rs:57-65 | (path, workspace?, force?) -> ReadResult{text{content,size,mtime}/binary/toolarge} | async |
| fs_write_file | file.rs:127-158 | (path, content, workspace?, source?, app) -> u64 mtime; atomic tempfile+persist; restores perms; emits `fs:file-written {path,source}` | async |
| fs_canonicalize | file.rs:160-169 | (path, workspace?) -> String | async |
| fs_stat | file.rs:171-192 | (path, workspace?) -> {size,mtime,kind} | async |
| fs_create_file / fs_create_dir | mutate.rs:221-248 | (path, workspace?) | sync |
| fs_rename | mutate.rs:251-270 | (from, to, workspace?) | sync |
| fs_move | mutate.rs:273-285 | (from, to, root, expected_conflict?, workspace?, registry) -> Moved/Conflict{replaceable,token} | sync |
| fs_delete | mutate.rs:345-353 | (path, workspace?) | sync |
| fs_delete_batch | mutate.rs:355-370 | (paths, root, workspace?, registry) -> {deleted, failed} | sync |
| fs_copy | mutate.rs:410-437 | (sources, dest_dir, workspace?) | sync |
| fs_watch_add / fs_watch_remove | watch.rs:229-271 | (paths, workspace?) -> emits `fs:changed {paths}` debounced 150ms | sync |
| fs_search | search.rs:47-66 | (root, query, limit?, workspace?, show_hidden?) | sync |
| fs_list_files | search.rs:156-175 | (root, limit?, max_depth?, workspace?, show_hidden?) | sync |
| fs_grep | grep.rs:172-212 | (pattern, root, glob?, case_insensitive?, max_results?, workspace?) | sync |
| fs_grep_interactive | grep.rs:214-262 | (state, pattern, root, max_results?, workspace?) | sync |
| fs_glob | grep.rs:266-280 | (pattern, root, max_results?, workspace?) | sync |

**[I, per Tauri 2 docs]** Commands without `async` run on the main thread. Any remote branch inside the sync commands must be moved to `async` or `spawn_blocking`, or network latency will freeze the UI.

---

## 3. Is there a FsProvider abstraction?

**[V] No.** Calls are scattered `invoke("fs_*", {..., workspace: currentWorkspaceEnv()})`.

The closest thing is the `native` facade (`ai/lib/native.ts:139-384`), which covers:
- workspace: `workspaceAuthorize`
- fs: `readFile`, `writeFile`, `canonicalize`, `createFile`, `createDir`, `readDir`, `grep`, `glob`
- shell: 6 methods
- git: 17 methods

It always uses the global env.

### Every call site that would need routing [V]
**Explorer**
- explorer/lib/useFileTree.ts:181, 394, 424, 443, 462, 513
- explorer/lib/watch.ts:11, 19, 28
- explorer/lib/useExplorerFileDrop.ts:72
- explorer/ExplorerSearch.tsx:117 (`fs_search`)
- explorer/lib/useGitStatus.ts:31
- explorer/lib/contextActions.ts:19 (reveal, local-only)

**Editor**
- editor/lib/useDocument.ts:63 (write), 81 (stat), 132 (read)
- editor/useEditorFileSync.ts:47 (`fs:file-written`), 72-73 (watch), 80 (`fs:changed`)
- editor/lib/externalFormat.ts:129 (`shell_run_command` for format-on-save), 148 (read)
- editor/NewEditorDialog.tsx:72
- editor/EditorPane.tsx:583-584 (`convertFileSrc`, asset protocol, local-only media preview)
- markdown/MarkdownPreviewPane.tsx:36

**Statusbar, terminal, palette**
- statusbar/CwdBreadcrumb.tsx:183 (`list_subdirs`)
- terminal/block/lib/pathComplete.ts:43 (`fs_read_dir`)
- command-palette/hooks/useContentSearch.ts:31 (`fs_grep_interactive`)
- terminal/lib/pty-bridge.ts:103-113 (`pty_open`)

**AI**
- ai/hooks/useWorkspaceFiles.ts:30 (`fs_list_files`; its cache is keyed by root only)
- ai/lib/composer.tsx:188
- ai/lib/native.ts:141-383 and every `native.*` caller: ai/tools/fs.ts, edit.ts, search.ts, shell.ts, planStore.ts:54-56, AgentRunBridge.tsx:360, transport.ts:19 (TERAX.md memory, `projectMemoryCache` keyed by root only)

**Git**
- source-control/*, git-history/*, editor/lib/diffCache.ts:66, 90 (already scoped by `workspaceScopeKey`)

**LSP**
- lsp/lib/transport.ts:60 (already local-only via sessionManager.ts:67)

**Must always stay LOCAL** (they currently use the global env, which is a pre-existing bug):
- theme/themeFiles.ts:29-49
- theme/useThemeFileEditing.ts:39

### Cheap provider [I]
Add `src/modules/workspace/fs.ts` exporting `fsFor(env)` with typed methods:
- readDir, listSubdirs, readFile, writeFile, stat, canonicalize
- createFile, createDir, rename, move, delete, deleteBatch, copy
- search, listFiles, grep, grepInteractive, glob, watchAdd, watchRemove

Plus `envCaps(env)` returning `{watch, gitDecorations, reveal, mediaPreview, lsp, git, formatOnSave, aiShell}`.

The implementation can keep invoking the same commands with `workspace: env`, so the routing happens in Rust. This is mostly mechanical consolidation plus an explicit env parameter.

---

## 4. Editor IO and tab identity

- **[V]** `useDocument` (editor/lib/useDocument.ts):
  - Read: `fs_read_file` with the global env (130-138). It adopts `mtime` as `diskMtimeRef` (113-128).
  - Save: `saveNow` calls `fs_stat` and compares mtime. A mismatch shows the "File changed on disk" toast with Overwrite (78-97). Otherwise `writeToDisk` calls `fs_write_file`, which returns the new mtime (61-74).
  - Autosave: a timer calls `saveNow` (206-222).
- **[V] Hazard:** the env is read **at call time** from the global store.
  - If an autosave timer fires, or a watcher-triggered `reload()` runs, after the user switched to another space with a different env, the write or read goes to the other host with the same path string.
  - Today this is latent for WSL. For SSH it is a real data-integrity risk.
- **[V]** `useEditorFileSync` (editor/useEditorFileSync.ts):
  - Reloads editor tabs on `fs:file-written` with a non-editor source (43-63) and on `fs:changed` (77-96).
  - Watches the parent dirs of all editor tabs across all spaces (65-75).
  - Matching is by path string only.
- **[V]** Tab identity:
  - `EditorTab.path` plus `TabBase.spaceId` (tabs/lib/useTabs.ts:33-66).
  - Dedupe is per space: `tab.spaceId === spaceId && tab.path === path` (useTabs.ts:205-216).
  - So "one env per space" makes `(spaceId, path)` a sound remote-file identity, if IO resolves the env from the tab's space rather than the global store.
- **[V]** `TerminalStack` and `EditorStack` receive full `Tab` objects (TerminalStack.tsx:9-36, EditorStack.tsx:114-116), so the env can be derived per tab from `spaceId`.
- **[I]** SFTP v3 ATTRS mtime is uint32 seconds. Remote conflict detection is therefore second-granular unless mtime is fetched via exec `stat -c %Y.%N` (GNU-only).

---

## 5. Spaces (persisted shape)

- **[V]** `SpaceMeta { id, name, root: string|null, env: WorkspaceEnv, color?, createdAt, updatedAt }` (spaces/lib/store.ts:5-14).
  - Store file: `terax-spaces.json` (store.ts:21), with keys `spaces`, `activeId`, `state:<id>`.
  - `SpaceState { tabs: SerializedTab[], activeTabIndex }` (store.ts:16-19).
- **[V]** `SerializedTab` (spaces/lib/serialize.ts:14-27):
  - terminal: `tree` of leaves `{cwd?}`, `blocks?`, `customTitle?`
  - editor: `path`
  - preview: `url`
  - markdown: `path`
- **[V]** `useSpaces.create` takes `env` from the input or from the `defaultWorkspaceEnv` pref string (useSpaces.ts:53-71). `setEnv` (81-87). `handleNewSpace` copies the current global env (App.tsx:1159-1171).
- **[V]** `SpaceMeta.root` is written at creation but not consumed elsewhere. The explorer follows the terminal cwd instead.
- **[I] Where remote data goes:**
  - Put the host in `env`: `{kind:"ssh", host: "<ssh_config alias>"}` or `{kind:"ssh", hostId}`.
  - Put the remote root in `root`; it seeds the first terminal cwd.
  - Persist host profiles (label, alias, user?, port?, defaultRoot) in a separate LazyStore, e.g. `terax-ssh-hosts.json`. **Never store passwords**: use ssh-agent or keychain, or have the user type into the PTY.
  - `workspaceScopeKey` would become `ssh:<host>`.

---

## 6. Terminal spawn

- **[V]** Frontend:
  - `openPty(cols, rows, handlers, cwd, blocks, shell, paneId)` invokes `pty_open {cols, rows, cwd, workspace: currentWorkspaceEnv(), blocks, shell, paneId, onData, onExit}` (pty-bridge.ts:36-113).
  - Called from useGhosttyTerminalSession.ts:792-822 with `session.initialCwd`, the blocks flag and the `terminalShell` pref.
- **[V]** Rust `pty_open` (pty/mod.rs:44-118):
  1. `user_spawn_cwd_or_home` canonicalizes the cwd **locally** (line 63). A remote path would fail and fall back to local home.
  2. Control env is skipped only for WSL (66-70).
  3. Then `session::spawn` (session.rs:282-308) runs `native_pty_system().openpty`, then `shell_init::build_command(cwd, workspace, blocks, shell, control)`, then `spawn_command`.
- **[V]** `build_command` (shell_init.rs:53-71):
  - The `sanitize_shell_override` allowlist (74-88).
  - `apply_common` (146-207) sets `TERM=xterm-256color`, `COLORTERM`, `TERAX_TERMINAL=1`, `TERAX_BLOCKS`, AppImage env overrides, control env, a UTF-8 locale, and the **local** cwd.
  - Unix `build` (311-322) plus `apply_shell_init` (324-379):
    - zsh: `ZDOTDIR` points at `~/.cache/terax/shell-integration/zsh`, plus `-l`.
    - bash: `--rcfile` plus `-i`.
    - fish: `conf.d/terax.fish`, `fish_features=no-mark-prompt`, `-i -C`.
- **[V] The WSL precedent is the template for remote integration** (shell_init.rs:605-795):
  - Probe the login shell (`wsl_login_shell` getent script, workspace.rs:596-619).
  - Write the embedded scripts into the **guest's** `~/.cache/terax/shell-integration/<shell>` via UNC, with LF normalization (741-795).
  - Launch `env ZDOTDIR=... <shell> -l` (or `--rcfile`, or fish `-C`) inside the guest (671-730).
  - Set `TERM`, `COLORTERM`, `TERAX_TERMINAL` (663-666).
  - The getent script and `normalize_wsl_value` are `#[cfg(windows)]` today and would need to move to shared code.

### Remote PTY [I]
Use system OpenSSH in the PTY:
```
ssh -tt -o ControlMaster=auto -o ControlPath=~/.cache/terax/ssh/%C -o ControlPersist=10m -- <host> \
  '<bootstrap>'
```
- **Bootstrap:** `cd -- '<cwd>' 2>/dev/null || cd; exec env TERAX_TERMINAL=1 COLORTERM=truecolor [TERAX_BLOCKS=1] [ZDOTDIR=$HOME/.cache/terax/shell-integration/zsh TERAX_USER_ZDOTDIR=...] "$SHELL" -l` (or bash `--rcfile ... -i`).
- **Script delivery:** upload beforehand through the mux (`mkdir -p` plus `cat >`, a base64 payload, compare-then-write like `write_if_changed`), or pass them inline as a base64 env payload decoded by a `/bin/sh` stub (the kitty ssh-kitten approach, with no persistent remote files).
- **Environment:** TERM is forwarded by ssh from the local process env set in `apply_common`. `COLORTERM` and `TERAX_*` are not forwarded without AcceptEnv, so they go through `env` on the remote command line.
- **Interactive prompts** (password, 2FA, host key) appear in the terminal itself.
- **Control env** must be skipped, the same as WSL.
- **The PTY ssh process becomes the ControlMaster.** The explorer and other ops then reuse it with `-o BatchMode=yes`, so even password-auth servers need only one login.

### Side effects for remote tabs [V]
- `pty_has_foreground_process` uses `pgrep -P shell_pid` (pty/mod.rs:236-248, 281-288). The ssh client normally has no children, so the result is always false and the close-tab warning is lost.
- `pty_has_foreground_job` (253-278) treats ssh as the foreground pgid leader, so it also always returns false.
- Agent detection (`pty/agent_detect.rs`) is OSC-driven and should work unchanged over ssh **[I]**.

### OSC 7 [V]
- `parseOsc7` (terminal/lib/osc-handlers.ts:5-21) matches `^file:\/\/[^/]*(\/.*)$`. **The host part is dropped and never validated.** This is locked by a test: `file://host/home/me/a%20b` gives `/home/me/a b` (osc-handlers.test.ts:10).
- The router accepts `pwd` only outside commands (GhosttySemanticEventRouter.ts:33-38). `prompt-start` resets `inCommand` (47-50).
- The scripts emit `file://$HOST$PWD` (zshrc.zsh:46), `$HOSTNAME` (bashrc.bash:54) and `uname -n` (init.fish).
- **Pre-existing hazard [I]:** a local tab running a plain `ssh host`, where the remote shell emits its own OSC 133 A (fish 4 native mark-prompt, or a prior Terax `conf.d/terax.fish`), then OSC 7. The router accepts the remote path as the local tab cwd, so the explorer and `workspace_authorize` act on a local path that does not exist.
- **[I] Fix:** `parseOsc7` returns `{host, path}`. Local tabs accept only `""`, `localhost`, or the local hostname (compare case-insensitively and strip `.local`). SSH tabs accept any host, or pin the host on the first OSC 7.

---

## 7. Features that break or must be gated for remote

| Feature | Why | Code | v1 recommendation |
|---|---|---|---|
| fs watch | `notify` crate is local-only | watch.rs | Disable; refresh on expand, window focus and a manual button; optional poll of expanded dirs |
| Explorer git decorations (`gitignored`) | Uses an `ignore::WalkBuilder` local walk | tree.rs:29-60, 149-154 | Force `gitDecorations=false` |
| Source control and git history | `canonical_dir` / `authorized_repo_root` need a host-visible `local_path`; `resolve_within_repo` canonicalizes locally; `read_text_file` reads the worktree locally | git/utils.rs:29-63, 65-127; process.rs:200-222, 307-332; operations.rs:216-250 | Disable with a UI message (terminal git still works); later run remote git through the mux with string-based path validation and SFTP reads |
| LSP | Already blocked on the frontend (sessionManager.ts:67, useLspHint.ts:17); Rust only blocks WSL | lsp/mod.rs:73 | Harden to `!Local` |
| AI fs tools | `native.*` routes by global env; deny-list is string-based and works on POSIX paths; `checkReadableCanonical` / `checkWritableCanonical` take an injected `canonicalize` (security.ts:265-322), which must be **remote realpath** | ai/tools/fs.ts, search.ts, edit.ts | Gate behind a setting or allow read-only; keep the deny-list |
| AI run_command / shell_session / bg | `build_oneshot_command` unix arm runs locally | shell/mod.rs:284-327, 94; background.rs:108; session.rs:129-141 | Add an ssh arm (`ssh -S ctl -o BatchMode=yes host -- sh -lc '<quoted>'`) or disable |
| Format-on-save | Runs `shell_run_command` with cwd=dirname(path) | externalFormat.ts:129-145 | Off for remote |
| Media preview (img, pdf, video) | `convertFileSrc` asset protocol, local only | EditorPane.tsx:583-584 | Disable, or add a binary read returning a Blob URL |
| Reveal in Finder | Local opener | contextActions.ts:17 | Hide |
| Fuzzy search, content search, AI file list, glob | `ignore` / `grep-searcher` local walks | search.rs, grep.rs | Remote `rg --files` / `rg --json`, falling back to `find` / `grep -rn` via exec, with pure parsers |
| Drag-drop copy | `fs_copy` with local sources | mutate.rs:410-437 | SFTP upload |
| Theme files | Use the global env | themeFiles.ts:29-49, useThemeFileEditing.ts:39 | Force LOCAL (bug fix) |
| Block-mode history suggestions | Local history store | history/mod.rs | Accept, or disable for remote |
| Dev-server preview chip | Remote `localhost:PORT` opened locally | preview/lib/previewUrl.ts | Later: `ssh -O forward -L` port forwarding |
| Breadcrumb `~` | `home` must be the remote home | CwdBreadcrumb.tsx:183 | Works once `resolveEnvHome` handles ssh |

---

## 8. Security constraints for a remote provider

- **[V]** `docs/architecture/security-model.md:83-89` invariants:
  - The deny-list applies on read and write.
  - New fs-touching commands must respect the registry.
  - Secrets stay in the keychain.
  - New plugin APIs need capabilities. Custom commands do not need `default.json` entries.
- **[V]** TERAX.md:21 requires validation at every boundary. TERAX.md:30 requires a test locking the invariant for any change to a core subsystem (spawn, auth, fs, IPC, AI tools).
- **[I] Required for SSH:**
  1. **Host argument-injection guard.** A host such as `-oProxyCommand=...` is RCE. Validate against an allowlist like `^[A-Za-z0-9._@:\[\]%-]+$`, with no leading `-`, and always put `--` before the destination.
  2. **POSIX single-quote escaping** of every remote argument, because ssh joins argv with spaces. Make it a pure function with proptest coverage.
  3. **ControlPath directory** created with mode 0700. Keep the socket path under the 104-byte limit on macOS.
  4. **Non-interactive channels** use `BatchMode=yes` and a `ConnectTimeout`.
  5. **Host-key checking stays with OpenSSH** (known_hosts). Never set `StrictHostKeyChecking=no`.
  6. **A RemoteRegistry** keyed by host, holding normalized POSIX roots (collapse `.`, `..` and `//` purely, then server-side realpath). Mutating ops (write, rename, move, delete, mkdir, upload) must check against it. Roots are authorized only by explicit user action (opening a remote folder or terminal), never by the AI.
  7. **Never forward** `TERAX_CONTROL_TOKEN`, API keys or `SSH_AUTH_SOCK` agent forwarding unless the user opts in.
  8. **Atomic remote write:** a temp file in the same dir with a random suffix, then `posix-rename@openssh.com`, then setstat with the original mode. This mirrors `write_atomic` in file.rs:112-123.
  9. **Event payloads** (`fs:changed`, `fs:file-written`) need an env tag so a remote path cannot trigger a local reload or write.

---

## 9. Cargo and binary size [V]

- **Release profile** (Cargo.toml:112-117): `codegen-units=1`, `lto="fat"`, `opt-level="s"`, `panic="abort"`, `strip=true`. The product target is a ~7-8 MB bundle (TERAX.md:20).
- **No SSH crates** in Cargo.lock.
- **Crypto:** `ring 0.17.14` (Cargo.lock:3618) and `sha2` are already present via rustls/reqwest (`reqwest` with `rustls-tls`, Cargo.toml:57-60).
- **tokio:** direct dependency declares only `features=["rt"]` (Cargo.toml:63). `cargo tree --offline -e features -i tokio` shows the graph already enables `rt-multi-thread`, `net`, `io-util`, `macros`, `sync`, `time` and `fs`. **`process` is not enabled.** tokio is 1.53.1; `hyper-util`, `mio` and `tokio-util` are present.
- **Already available:** `which = "8.0.4"` (Cargo.toml:66) for resolving `ssh`, plus `tempfile`, `notify`, `shared_child`, and `libc` on unix.
- **Transport options [I]:**
  - **(A) Recommended: system OpenSSH.** Use `ssh` in the PTY plus ControlMaster mux, `ssh -S ctl host -s sftp` stdio for FS, and `ssh -S ctl host -- cmd` for exec.
    - Adds about zero dependencies. A hand-written SFTP v3 client is roughly 600-900 lines, or use `russh-sftp` over child stdio, which needs tokio `process`.
    - Inherits `~/.ssh/config`, the agent, keychain (`UseKeychain`), ProxyJump, 2FA and known_hosts for free.
  - **(B) russh plus russh-sftp.** Pure Rust with full control, but ssh_config, agent, ProxyJump and host-key UX must be reimplemented. It likely adds 1-2 MB. Pin `ring` and avoid `aws-lc-rs` defaults; verify feature flags.
  - **(C) ssh2 / libssh2.** C dependency plus OpenSSL. Not recommended.
  - **(D) SSHFS / macFUSE mount.** Zero code, since the existing local fs works, but it needs a macFUSE kext on macOS. Only as a stopgap.

---

## 10. Interfaces to introduce [I]

**TypeScript**
```ts
// src/modules/workspace/env.ts
export type WorkspaceEnv = { kind: "local" } | { kind: "wsl"; distro: string } | { kind: "ssh"; host: string };
export function envCaps(env: WorkspaceEnv): EnvCaps;          // exhaustive switch + assertNever
export function workspaceScopeKey / parseWorkspaceScopeKey      // add "ssh:<host>"
// src/modules/workspace/fs.ts
export interface FsProvider { readDir; listSubdirs; readFile; writeFile; stat; canonicalize; createFile; createDir; rename; move; delete; deleteBatch; copy; search; listFiles; grep; grepInteractive; glob; watchAdd; watchRemove; }
export function fsFor(env: WorkspaceEnv): FsProvider;          // invokes existing commands with explicit env
// src/modules/spaces/lib/envForSpace.ts
export function envForSpace(spaceId: string): WorkspaceEnv;     // tab -> space -> env (replaces global reads in IO paths)
```

**Rust**
```rust
// workspace.rs
pub enum WorkspaceEnv { Local, Wsl { distro: String }, Ssh { host: String } }
pub fn local_path(path: &str, ws: &WorkspaceEnv) -> Result<PathBuf, String>  // Err for Ssh: makes unported commands fail closed
// modules/remote/{mod,target,quote,control,exec,sftp,fs,search,registry,shell_init}.rs
pub struct RemoteState { hosts: Mutex<HashMap<String, HostConn /* ctl path, sftp channel */>> }
pub struct RemoteRegistry { roots: Mutex<HashMap<String, HashSet<String>>> }
// commands: ssh_hosts_list (parse ~/.ssh/config Host entries or `ssh -G`), ssh_connect/ssh_check/ssh_disconnect, ssh_home
```

**Dispatch:** at the top of each fs command, `if let WorkspaceEnv::Ssh{host} = &ws { return remote::fs::xxx(...).await }`. This keeps every frontend call shape unchanged.

**Alternative:** separate `ssh_fs_*` commands with TS-side routing in `fsFor(env)`. This leaves the core local commands untouched, at the cost of duplicate command surfaces.

---

## 11. Ordered refactor steps [I]

**Phase 0: safety refactors** (no behavior change; each needs tests)
1. **Rust fail-closed:**
   - Replace `resolve_path` with a fallible `local_path` (or keep `resolve_path` and add a guard), with all call sites updated: fs ×23, git/utils.rs:34, shell ×3, workspace.rs.
   - Turn `build_command` (shell_init.rs:53-71) and `build_oneshot_command` (shell/mod.rs:284) into exhaustive matches.
   - lsp/mod.rs:73 becomes `!Local`.
   - pty/mod.rs:66 skips control env for any non-Local env.
   - Replace `is_wsl()` uses with explicit matches or an `is_posix_guest()` helper.
2. **TS exhaustive env handling:**
   - env.ts:52-60, useWorkspaceSwitcher.ts:11-15 and 76-82, activeSpace.ts:24-31.
3. **Per-tab env:** add `envForSpace(spaceId)` and thread the env into:
   - pty-bridge `openPty` (an extra param from TerminalPane / useGhosttyTerminalSession)
   - `useDocument` (EditorPane prop) and MarkdownPreviewPane
   - useEditorFileSync, `handleTerminalCwd` (App.tsx:1076), useSpacesBoot.ts:106
4. **Force LOCAL for theme files** (themeFiles.ts, useThemeFileEditing.ts).
5. **Scope caches by env key:**
   - useFileTree.ts:58 `expansionCache`, useWorkspaceFiles cache, transport.ts `projectMemoryCache`, App.tsx:1075 `authorizedCwds`.
   - Include `workspaceKey` in the root-change effect deps in useFileTree.ts:243-277.
6. **Introduce the `fsFor(env)` provider** and migrate the ~31 call sites listed in section 3, plus `native` fs methods.
7. **Add an env tag** to the `fs:changed` / `fs:file-written` payloads and filter listeners by env.
8. **OSC 7 host awareness:** parseOsc7 returns `{host,path}` and the router validates the host. Update the tests.

**Phase 1: remote core in Rust**
1. `remote/target.rs` (host validation, argv builder) and `remote/quote.rs`. Pure, with proptests.
2. `remote/control.rs` (ControlPath dir 0700, `-O check` / `-O exit`, ConnectTimeout, BatchMode), `ssh_home`, `ssh_hosts_list`.
3. `remote/registry.rs` plus a `workspace_authorize` ssh branch (pure POSIX normalization, server realpath).
4. PTY:
   - `WorkspaceEnv::Ssh` arm in `build_command`, built in `remote/shell_init.rs`.
   - Move `wsl_login_shell` SCRIPT and `normalize_wsl_value` out of `cfg(windows)`.
   - Script upload, `ssh -tt` with ControlMaster.
   - `pty_open` skips local cwd canonicalization for Ssh.

**Phase 2: remote FS**
1. SFTP client over `ssh -S ctl host -s sftp`: one channel per host in RemoteState, request-id multiplexing, reconnect, timeouts.
2. Port read_dir, list_subdirs, read_file (stat then limit, plus binary sniff), write_file (atomic `posix-rename`, setstat mode, return mtime), stat, canonicalize, create_file, create_dir, rename, move (conflict token from attrs), delete, delete_batch (recursive), copy (upload). Every ported command runs async.
3. Search via exec with pure output parsers and the same caps (MAX_SCANNED, HARD_MAX_RESULTS).
4. Watch: `fs_watch_*` becomes a no-op for Ssh, with optional polling in RemoteState; the frontend refreshes on focus.

**Phase 3: UI**
1. Host profiles store and a "Connect to SSH host" flow (space switcher, command palette, NewTabMenu). It creates a space with `env: ssh` and a remote root.
2. A cross-platform env indicator in StatusBar (WorkspaceEnvSelector is Windows-only today) with connection status.
3. Gate UI by `envCaps`: hide reveal, disable git decorations, show a source-control placeholder, turn off format-on-save, add a media-preview fallback.

**Phase 4: optional**
- AI shell over ssh, remote git, port forwarding for preview, and remote `rg` detection.

**Docs to update:** TERAX.md (module list and security), security-model.md, two-process-model.md, pty-shell-integration.md.

## 12. Suggested work-package granularity (for the planner) [I]
- **Mechanical, low risk:** Phase 0 steps 2, 4, 5, 6; Phase 3 step 3 UI gating.
- **Moderate:** Phase 0 steps 3, 7, 8; Phase 2 steps 3 and 4; Phase 3 steps 1 and 2.
- **Security- and correctness-critical, needs the strongest reviewer:** Phase 0 step 1; Phase 1 steps 1, 3, 4; Phase 2 steps 1 and 2 (SFTP client, atomic write, registry, quoting, host-key and argv injection).


## keyFiles
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/workspace.rs: WorkspaceEnv enum (311-329); WorkspaceRegistry (19-71); spawn-cwd auth helpers (75-133); workspace_authorize (142-152); resolve_path, a no-op on non-Windows and a fail-open hazard (338-349); WSL UNC mapping (399-428); wsl_exec_capture (471-490); wsl_login_shell getent script, reusable for remote shell detection (596-619)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/workspace/env.ts: TS WorkspaceEnv union, global store, scope-key encode/decode that collapses unknown kinds to local (52-60); where kind 'ssh' must be added
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/shell_init.rs: build_command (53-71, unix arm ignores env); apply_common env and local cwd (146-207); unix shell init (311-379); WSL launch spec and script upload into the guest (605-795), the template for an ssh -tt arm
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/mod.rs: pty_open: local cwd canonicalization (63) and control-token env skipped only for WSL (66-70); foreground-process checks that become meaningless for an ssh child (236-288)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/pty/session.rs: session::spawn (282-308): openpty + build_command + spawn_command; any CommandBuilder, including ssh, works here
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/tree.rs: fs_read_dir (134-214, sync, local git-ignore walk) and list_subdirs (221-248); explorer data source to dispatch for ssh
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/file.rs: fs_read_file limits and binary sniff (57-103); atomic write_atomic + fs_write_file returning mtime (112-158); fs_canonicalize/fs_stat (160-192); semantics a remote SFTP backend must mirror
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/mutate.rs: create/rename/delete are ungated (221-270, 345-353); fs_move and fs_delete_batch are root-gated via resolve_authorized_root and authorize_mutation_entry (39-72, 273-392); fs_copy, the local-to-dest upload analog (410-437)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/watch.rs: notify-based watch with registry gating and an env-less `fs:changed` event (109-271); unsupported remotely
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/search.rs: fs_search/fs_list_files (47-228) are local ignore walks; display_path is_wsl branch (231-250)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/fs/grep.rs: fs_grep/fs_grep_interactive/fs_glob (172-320) are local grep-searcher walks; is_wsl display branch (322-345)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/git/utils.rs: canonical_dir/authorized_repo_root need a host-visible local_path (29-63); resolve_within_repo canonicalizes locally (65-127); the blocker for remote git
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/git/process.rs: build_git_command WSL arm (307-332), where an ssh exec arm would go; read_text_file reads the worktree locally (200-222)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/shell/mod.rs: build_oneshot_command (284-327; unix arm ignores env, so AI commands would run locally); shell_* commands gated by authorize_spawn_cwd (41-56, 171-247)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/shell/session.rs: POSIX sentinel wrapping chosen by is_wsl (129-141); cwd tracking via resolve_path (66-120)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/lsp/mod.rs: lsp_spawn rejects only WSL (72-75); must reject every non-local env
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/lib.rs: command registration (253-344) and registry bootstrap (243-250); new remote commands and state go here
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/Cargo.toml: tokio declared with rt only (63; graph already enables net/io-util/macros/sync/time but not process); reqwest rustls (57-60; ring 0.17 present); which (66); release profile opt-level s / lto fat / strip / panic abort (112-117); no ssh crates
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/explorer/lib/useFileTree.ts: explorer data source: fs_read_dir and mutations with the global env (181, 394, 424, 443, 462, 513); expansionCache keyed by root only (57-76); root effect keyed only on rootPath (243-277); fs:changed listener (279-299)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/explorer/lib/watch.ts: fs_watch_add/remove and the env-less fs:changed listener (9-32)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/explorer/lib/useGitStatus.ts: git decoration overlay built from the SC snapshot plus native.canonicalize (16-84)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/editor/lib/useDocument.ts: editor open/save with mtime conflict check (59-138); reads the global env at call time, so autosave or reload after a space switch can hit the wrong host
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/editor/useEditorFileSync.ts: reload on fs:file-written / fs:changed, matched by path only; watches editor parent dirs across all spaces (23-97)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/ai/lib/native.ts: de facto provider facade for AI and git (139-384); every method uses currentWorkspaceEnv()
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/ai/lib/security.ts: string-based deny-list, works on POSIX remote paths; checkReadableCanonical/checkWritableCanonical take an injected canonicalize (265-322), which must be remote realpath for ssh
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/lib/pty-bridge.ts: openPty passes workspace: currentWorkspaceEnv() to pty_open (36-113); needs an explicit per-tab env
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/lib/osc-handlers.ts: parseOsc7 drops the URI host without validation (5-21); locked by osc-handlers.test.ts:10
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/terminal/ghostty/core/GhosttySemanticEventRouter.ts: OSC 7 accepted only between commands (33-38, 47-63); where host validation would be enforced
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/spaces/lib/store.ts: persisted SpaceMeta {id,name,root,env,color,...} in terax-spaces.json (5-27); where the ssh env and remote root are stored
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/spaces/lib/serialize.ts: SerializedTab shape (14-27) and hydrate (139-229)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/spaces/lib/useSpacesBoot.ts: boot adopts the active space env (94-97), then authorizes cwds of all spaces under that env (105-107); cross-env bug
- /Users/frider/Documents/iamcoder/terax_claude/src/app/hooks/useWorkspaceSwitcher.ts: resolveEnvHome ternary (11-15), switch equality (76-82), adoptWorkspaceEnv (115-128); needs ssh branches
- /Users/frider/Documents/iamcoder/terax_claude/src/app/App.tsx: handleWorkspaceChange (251-258); space switch adopts env without teardown (278-298); OSC 7 cwd authorization with the global env (1075-1086); handleNewSpace copies the global env (1159-1171)
- /Users/frider/Documents/iamcoder/terax_claude/src/modules/theme/themeFiles.ts: theme files in the local app config dir use the global env (29-49); must be forced LOCAL
- /Users/frider/Documents/iamcoder/terax_claude/docs/architecture/security-model.md: invariants a remote provider must respect (31-40, 83-89)

## integrationPoints
- src-tauri/src/modules/workspace.rs:311-329: add `Ssh { host: String }` to WorkspaceEnv; replace resolve_path (338-349) with a fallible local_path that errors for Ssh so unported commands fail closed
- src-tauri/src/modules/pty/shell_init.rs:53-71: build_command needs an exhaustive match with a WorkspaceEnv::Ssh arm, a new remote/shell_init.rs modeled on build_wsl / build_wsl_launch_spec / prepare_wsl_integration_dir (605-795); move wsl_login_shell SCRIPT (workspace.rs:596-619) out of cfg(windows)
- src-tauri/src/modules/pty/mod.rs:63-70: for Ssh, skip local cwd canonicalization (pass the remote cwd into the remote bootstrap) and skip control env injection
- src-tauri/src/modules/fs/{tree.rs:134,221; file.rs:57,127,160,171; mutate.rs:221,237,251,273,345,355,410; watch.rs:229,250; search.rs:47,156; grep.rs:172,214,266}: per-command dispatch `if let WorkspaceEnv::Ssh{host}` into modules::remote::fs; ported paths must be async or spawn_blocking because most of these commands are sync
- src-tauri/src/modules/shell/mod.rs:284-327 (build_oneshot_command), mod.rs:94 and background.rs:108 (Local-only current_dir), session.rs:129-141 (posix sentinel): ssh exec arm `ssh -S <ctl> -o BatchMode=yes -- <host> sh -lc '<quoted>'` or an explicit rejection
- src-tauri/src/modules/git/process.rs:307-332 and git/utils.rs:29-63: reject Ssh in v1; later an ssh exec arm plus string-based path validation instead of local_path canonicalize
- src-tauri/src/modules/lsp/mod.rs:73: change `is_wsl()` to `!matches!(workspace, WorkspaceEnv::Local)`
- src-tauri/src/lib.rs:234-252, 253-344: manage RemoteState and RemoteRegistry; register ssh_* commands
- src-tauri/src/modules/workspace.rs:142-152 workspace_authorize: Ssh branch goes to the RemoteRegistry (host-scoped, normalized POSIX roots)
- src/modules/workspace/env.ts:5-7, 52-60: add the {kind:'ssh', host} kind, exhaustive scope keys `ssh:<host>`, envCaps(env); new src/modules/workspace/fs.ts with fsFor(env)
- src/modules/spaces: new envForSpace(spaceId) helper so IO resolves env from tab.spaceId instead of the global store
- src/modules/terminal/lib/pty-bridge.ts:36-113: add an explicit env param fed from TerminalPane / useGhosttyTerminalSession.ts:792
- src/modules/editor/lib/useDocument.ts:61-138 and useEditorFileSync.ts:43-96: take env as a prop/arg; filter fs events by env tag
- src/modules/explorer/lib/useFileTree.ts:181-519, watch.ts:9-32, ExplorerSearch.tsx:117, useExplorerFileDrop.ts:72, CwdBreadcrumb.tsx:183, pathComplete.ts:43, useContentSearch.ts:31, MarkdownPreviewPane.tsx:36, NewEditorDialog.tsx:72, ai/hooks/useWorkspaceFiles.ts:30, ai/lib/composer.tsx:188: migrate to fsFor(env)
- src/modules/terminal/lib/osc-handlers.ts:5-21 + GhosttySemanticEventRouter.ts:33-38: return {host,path}; validate host against the tab's expected host
- src/app/hooks/useWorkspaceSwitcher.ts:11-15, 76-82 and src/modules/spaces/lib/activeSpace.ts:24-31: ssh branches (remote home via ssh_home)
- src/app/App.tsx:1075-1086 and src/modules/spaces/lib/useSpacesBoot.ts:105-107: authorize cwds with each tab's space env, not the global env
- src/modules/statusbar/WorkspaceEnvSelector.tsx:22-24 (Windows-only today), src/modules/spaces/SpaceSwitcher.tsx, src/modules/tabs/NewTabMenu.tsx:29-36: SSH connect / host picker UI
- src/modules/theme/themeFiles.ts:29-49 and useThemeFileEditing.ts:39: force LOCAL env
- Feature gates via envCaps: explorer/lib/contextActions.ts:17 (reveal), editor/EditorPane.tsx:583 (convertFileSrc media), editor/lib/externalFormat.ts:129 (format-on-save), useFileTree gitDecorations, source-control panel, lsp/lib/sessionManager.ts:67 (already local-only)

## risks
- Fail-open on macOS when the Ssh variant is added: resolve_path ignores env on non-Windows (workspace.rs:346-349), build_command unix ignores env (shell_init.rs:61-65), build_oneshot_command unix ignores env (shell/mod.rs:308-322). Without a fallible resolver, remote paths would be read and written on the LOCAL disk and AI/terminal commands would run locally. Only two exhaustive matches exist, so the compiler will not catch most sites.
- Global env vs per-tab env: useDocument reads currentWorkspaceEnv() at call time (useDocument.ts:66, 83, 134). An autosave timer or watcher reload firing after a space switch can write a local buffer to the remote host (or the reverse) at the same path string. Same class: useSpacesBoot.ts:106 and App.tsx:1081 authorize cwds under the active env.
- Argument injection: an ssh destination starting with '-' (e.g. -oProxyCommand=...) is local code execution. Remote commands are re-parsed by the remote shell, so every argument needs strict POSIX quoting. Both must be pure, proptested functions.
- OSC 7 host is dropped (osc-handlers.ts:6). A local tab running plain `ssh` to a host that emits OSC 133 A (fish 4 native marks, or leftover Terax conf.d) can re-root the explorer and workspace_authorize onto a nonexistent local path. Host validation may falsely reject when $HOST differs from the OS hostname (.local suffix, case).
- Sync Tauri commands (fs_read_dir, mutate, search, grep, watch) run on the main thread per Tauri 2 docs. Remote branches must be async or spawn_blocking with timeouts, or network latency freezes the UI.
- SFTP v3 mtime has 1-second resolution, which weakens the editor's mtime-based overwrite-conflict check (useDocument.ts:79-94). Only rename-without-overwrite is standard; atomic replace needs the posix-rename@openssh.com extension.
- ControlMaster: the socket path limit is 104 bytes on macOS, the socket dir must be 0700, and ControlPersist lifetime and stale sockets need handling. Non-interactive channels must use BatchMode=yes and ConnectTimeout so a password or host-key prompt cannot hang the explorer.
- fs:changed and fs:file-written events carry only a path (watch.ts:104-107, file.rs:105-110). Remote events must be env-tagged or local and remote listeners will cross-trigger reloads.
- Path-only caches collide across envs: useFileTree expansionCache (57-76), useWorkspaceFiles cache, transport.ts projectMemoryCache, App.tsx authorizedCwds.
- Remote shell integration writes files on the server (~/.cache/terax/shell-integration, and ~/.config/fish/conf.d/terax.fish for fish), following the WSL precedent. It is persistent and may surprise users on shared or production servers.
- pty_has_foreground_process / pty_has_foreground_job (pty/mod.rs:236-288) always return false for an ssh child, so the close-tab running-process warning is lost for remote terminals.
- Git cannot be ported cheaply: operations depend on a host-visible local_path, local canonicalize, and local worktree reads (git/utils.rs:29-127, process.rs:200-222).
- Binary size: a pure-Rust SSH stack (russh) may add 1-2 MB and must be pinned to ring rather than aws-lc-rs (inference, needs verification). The system-OpenSSH approach adds roughly nothing but depends on /usr/bin/ssh and the user's ssh config.
- TERAX.md quality bar requires a test locking the invariant for any change to spawn, workspace auth, fs, IPC or the AI tool surface, plus cross-platform cfg discipline; a macOS-only fork may still want to keep Linux and Windows compiling.

## openQuestions
- Transport: system OpenSSH (ssh -tt in the PTY, ControlMaster mux, `ssh -s sftp` stdio for FS, exec for search; reuses ~/.ssh/config, agent, keychain, ProxyJump, 2FA) vs pure-Rust russh + russh-sftp (more control, more size and security surface) vs SSHFS/macFUSE (zero code, needs a kext)?
- Dispatch location: Rust-side `match WorkspaceEnv::Ssh` inside existing fs commands (no frontend signature change) vs separate ssh_fs_* commands with TS-side routing in fsFor(env) (core local commands untouched)?
- Env identity: put the raw ssh_config alias / user@host[:port] in WorkspaceEnv, or a hostId referencing a stored profile (label, alias, default root)?
- Auth UX: first terminal tab acts as ControlMaster (password/2FA typed in the terminal; explorer waits for `ssh -O check`) vs a dedicated Connect dialog that opens a PTY just for authentication?
- Remote file watching: none (refresh on expand, focus and a manual button), polling of expanded dirs at an interval, or remote inotifywait when available?
- v1 scope: which features are disabled vs ported for SSH spaces (source control/git history, AI read tools, AI run_command, fuzzy/content search, format-on-save, media preview, drag-drop upload)?
- Remote search dependency: require ripgrep on the server, or fall back to find/grep -rn with lower caps?
- Shell integration delivery: persist scripts in remote ~/.cache/terax (as WSL does) or deliver them ephemerally via an inline base64 bootstrap with no server-side files?
- Should one space be allowed to mix local and remote tabs? Recommendation: no, one env per space, which makes (spaceId, path) the file identity.
- Must the fork stay cross-platform (Linux/Windows compile and tests) per TERAX.md, or may SSH be macOS-first?
- Should AI tools in SSH spaces be read-only by default given remote production secrets, beyond the existing deny-list?
