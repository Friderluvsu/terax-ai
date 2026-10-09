# build-env

## existingSupport
The repo is a fully working baseline on this Mac with the right shell setup. Verified green: pnpm install --frozen-lockfile (28 s), check-types (7 s, 0 errors), lint (2 s, 0 errors, 90 warnings, 1 info), vitest 171 files / 1209 tests (10 s), cargo check --all-targets --locked (3 m cold, 0 warnings), cargo clippy -D warnings (clean), cargo test --locked 373 passed / 0 failed / 1 ignored (81 s), vite build (~52 s) + size-limit (all 5 budgets pass, total JS has only ~30 kB gzip headroom), React Compiler healthcheck 436/436. `pnpm tauri dev --config '{"identifier":"app.crynta.terax.dev"}'` launches the real app (PTY opens, WebGL fallback active because WebGPU shader validation fails on macOS 27). `pnpm tauri build --ignore-version-mismatches --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'` produces an arm64 Terax.app (9.84 MiB) at src-tauri/target/release/bundle/macos/Terax.app in 175 s, ad-hoc codesign verifies. Committed ghostty WASM artifacts mean zig is not required. tauri CLI 2.12.0 comes from devDependencies. cargo-nextest, zig, fish and a global pnpm are absent (pnpm works through corepack; nextest is optional with a documented cargo test fallback). Documented release recipe exists at docs/architecture/ghostty-release-readiness.md:326-329.

## report
# Terax local dev/build baseline (macOS arm64, 2026-10-09, repo HEAD 3301de2)

Everything below was executed on this Mac. "Verified" = I ran it. "Inferred" = reasoned from code/docs, not executed. No tracked file was modified (`git status --short --ignored` shows only ignored artifacts: dist/, node_modules/, src-tauri/target/, src-tauri/binaries/, src-tauri/gen/). No dev server or app process is left running (port 1420 free, no `target/debug/terax`).

## 1. Toolchain (verified)

| Tool | Found | Requirement | Status |
|---|---|---|---|
| macOS / arch | macOS 27.2 (Darwin 27.2.0), arm64 | `minimumSystemVersion` 13.0 (src-tauri/tauri.conf.json:61) | OK |
| Xcode CLT | /Library/Developer/CommandLineTools, clang 21.0.0; full Xcode NOT installed | Tauri on macOS needs only CLT | OK |
| node | default on PATH in the Bash tool = v20.16.0 (TOO OLD). nvm has v22.22.3 (nvm default alias = 22) | `engines.node >=22` (package.json:9) | BLOCKER unless PATH fixed (see 2) |
| pnpm | NOT installed for any nvm node | `packageManager pnpm@11.9.0` (package.json:7) | Works via corepack 0.34.6 (node 22). `corepack pnpm --version` = 11.9.0 |
| rustc / cargo | 1.97.1 stable (aarch64-apple-darwin) via rustup | no rust-toolchain file anywhere; README says "Rust (stable)" | OK |
| clippy, rustfmt | installed | needed | OK |
| tauri CLI | @tauri-apps/cli 2.12.0 in devDependencies (package.json:116), run as `pnpm tauri` | - | OK, no global cargo-tauri needed |
| cargo-nextest | NOT installed (`brew info cargo-nextest` = 0.9.148 bottled; or `cargo install cargo-nextest --locked`) | CI uses nextest; documented fallback is `cargo test --locked` | I used cargo test |
| zig | NOT installed | NOT needed: packages/ghostty-core ships committed WASM (adapted/ghostty-vt.wasm 696 KB SIMD, ghostty-vt-scalar.wasm, official/ghostty-vt.wasm, ghostty-vt.wasm). Zig 0.16.0 only needed to REBUILD them (adapted/UPSTREAM.md). | OK |
| fish | NOT installed | only for CI step `fish --no-config src-tauri/src/modules/pty/scripts/init.test.fish` (not part of cargo tests) | skipped |
| git | /usr/bin/git | git_operations tests | OK |

## 2. Shell gotchas that WILL bite implementation agents (verified)

The user's ~/.zshrc (lines 13-24) defines lazy `node/npm/npx/nvm/yarn/pnpm` shell functions that call `_load_nvm`. In the Bash tool they are inherited but `_load_nvm` is not defined, so calling `node` recurses until `maximum nested function level reached; increase FUNCNEST?`. Also the PATH-first node is v20.16.0. Fix for every command, put this at the top of each Bash call (or `source` a file containing it):

```sh
unset -f node npm npx nvm yarn pnpm 2>/dev/null
export PATH="$HOME/.nvm/versions/node/v22.22.3/bin:$HOME/.cargo/bin:$PATH"
export COREPACK_ENABLE_DOWNLOAD_PROMPT=0
```
`pnpm` must also be a real executable on PATH, not a shell function, because `tauri.conf.json` beforeDevCommand/beforeBuildCommand run `pnpm ...` via `sh -c`. I created a shim and env file (session scratchpad, reusable by other agents on this machine):
- /private/tmp/claude-501/-Users-frider-Documents-iamcoder-terax-claude/c2d597b7-b40d-4501-a701-a769049d2877/scratchpad/env.sh  (does the 3 lines above and prepends the shim dir)
- /private/tmp/claude-501/-Users-frider-Documents-iamcoder-terax-claude/c2d597b7-b40d-4501-a701-a769049d2877/scratchpad/bin/pnpm  (`#!/bin/sh; exec corepack pnpm "$@"`)

Usage: `source <scratchpad>/env.sh && cd /Users/frider/Documents/iamcoder/terax_claude && pnpm ...`.
Permanent fix for the user (NOT applied, modifies ~/.nvm): `source ~/.nvm/nvm.sh && nvm use 22 && corepack enable pnpm`. Note `ls` is an eza alias; use /bin/ls.

## 3. Baseline results (verified; use these to attribute later regressions)

| # | Command | Result | Duration |
|---|---|---|---|
| 1 | `pnpm install --frozen-lockfile` | PASS, 928 pkgs, 2 workspace projects (root + @terax/ghostty-core). Prints a "pnpm 12 available" nag, ignore. Slow-download WARNs are network only. | 28 s |
| 2 | `pnpm check-types` (`tsc --noEmit`) | PASS, 0 errors | 7 s |
| 3 | `pnpm lint` (`biome lint ./src`, biome 2.5.14) | PASS exit 0: 0 errors, **90 warnings, 1 info** (567 files). Top rules: useExhaustiveDependencies 22, a11y group 21, noNonNullAssertion 11, useTemplate 8, useOptionalChain 6, noTemplateCurlyInString 5, noAccumulatingSpread 4. Warnings do not fail CI; do not add new ones. | 2 s |
| 4 | `pnpm test` (`vitest run` v4.1.11) | PASS **171 files, 1209 tests, 0 failed** (vitest self-reports 7.6 s) | 10 s wall |
| 4b | `pnpm exec vitest run src/modules/terminal` (CI terminal-platforms job) | PASS 65 files / 449 tests | 3 s |
| 5 | `cd src-tauri && cargo check --all-targets --locked` | PASS, 0 warnings. Cold: downloads ~621 crates, includes first compile | 3 m 02 s (cold); 5 s warm no-change |
| 6 | `cd src-tauri && cargo clippy --all-targets --locked -- -D warnings` | PASS, 0 warnings (deps already checked by #5) | 5 s |
| 7 | `cd src-tauri && cargo test --locked` (nextest fallback) | PASS **373 passed, 0 failed, 1 ignored**: terax_lib 287 (+1 ignored manual PTY throughput test), tests/fs_search 25, ghostty_artifacts 2, git_operations 27, shell_background 7, terax-cli 15, terax-control-protocol 10, bin 0. | 81 s (incl. compiling test binaries) |
| 7b | `cargo test --locked --lib workspace` (filter example) | PASS 19 tests | 4 s warm |
| 8 | `pnpm build` (`tsc && vite build`) | PASS. NOTE 95% of vite time is the babel React Compiler plugin | ~52 s vite (82 s total with concurrent cargo) |
| 9 | `pnpm size` (size-limit, run after build) | PASS all 5 budgets: main startup 244.87 kB/540 kB gz, total client JS 1.47 MB/1.5 MB gz (**only ~30 kB headroom**), Ghostty primary 46.82/56 kB, WebGL fallback 14.11/15 kB, WASM variants 414.75/450 kB | 2 s |
| 10 | `pnpm exec react-compiler-healthcheck` (CI advisory) | PASS, 436/436 components compile | 7 s |
| 11 | `pnpm knip` (CI advisory, continue-on-error) | exit 1 baseline: unused exports/types, 8 unused enum members in ghostty-core official/types.ts, 12 config hints. Ignore as long as it is not worse. | 3 s |
| 12 | `pnpm format:check` / `cd src-tauri && cargo fmt --check` | both FAIL at baseline (biome: 112 format errors; rustfmt: 28 files / 73 hunks). Not a CI gate. Agents must NOT run `format --write` / `cargo fmt` repo-wide (noise); format only files they touch, or none. | 1 s each |
| 13 | `pnpm build:cli:dev` | PASS, creates gitignored src-tauri/binaries/terax-cli-aarch64-apple-darwin (1.4 MB) | 11 s |
| 14 | `cd src-tauri && cargo build --locked` (the app binary `tauri dev` runs) | PASS, target/debug/terax 80 MB | 31 s |
| 15 | `pnpm tauri dev` (with identifier override, see 5) | PASS: vite up in <1 s, app compiled (17.8 s incremental), window process started, PTY opened (`pty opened id=1`), no panic in log | ~50 s to running app |
| 16 | `pnpm tauri build --ignore-version-mismatches --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'` + ad-hoc codesign | PASS. Terax.app 9.84 MiB, arm64, `codesign --verify --deep --strict` exit 0, Signature=adhoc | 175 s total (Rust release LTO 2 m 32 s) |
| 17 | plain `pnpm tauri build` (no flags) | FAIL in 0 s, see blocker B1 | - |

Not run: `pnpm audit`, `cargo machete`, `cargo llvm-cov`, fish integration test, nextest, `pnpm soak:ghostty` / `profile:ghostty` / `bench:ghostty`.

## 4. Copy-pasteable command list

```sh
# 0. every Bash call
source /private/tmp/claude-501/-Users-frider-Documents-iamcoder-terax-claude/c2d597b7-b40d-4501-a701-a769049d2877/scratchpad/env.sh
cd /Users/frider/Documents/iamcoder/terax_claude

# 1. install (already done in this clone; re-run after any package.json/pnpm-lock change)
pnpm install --frozen-lockfile

# 2. frontend gate (CI "frontend" job; ~20 s total)
pnpm lint && pnpm check-types && pnpm test
pnpm exec vitest run path/to/file.test.ts          # single file
pnpm exec vitest run src/modules/terminal          # one module
pnpm build && pnpm size                             # bundle + budgets (needed when adding deps/features; budget headroom is tiny)

# 3. rust gate (CI "rust" job)
cd src-tauri
cargo check --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked                                 # nextest not installed; fallback documented in TERAX.md
cargo test --locked --lib <name_filter>             # fast loop; lib crate is terax_lib
# optional: brew install cargo-nextest && cargo nextest run --locked

# 4. run the app in dev (isolated profile, recommended, verified)
pnpm tauri dev --config '{"identifier":"app.crynta.terax.dev"}'
#   runs: beforeDevCommand "pnpm build:cli:dev && pnpm dev" (vite on strict port 1420) then `cargo run --no-default-features`
#   Stop: Ctrl-C in its terminal, then ALSO kill the orphaned binary BY PATH, never by name:
pkill -f 'src-tauri/target/debug/terax' ; pkill -f 'target/debug/terax'
#   (do NOT use `pkill -x terax`: it also kills the user's installed /Applications/Terax.app daily driver)
#   frontend only (no Tauri IPC, invoke() fails): pnpm dev  -> http://localhost:1420

# 5. release .app on macOS (verified)
pnpm tauri build --ignore-version-mismatches --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
codesign --force --sign - src-tauri/target/release/bundle/macos/Terax.app/Contents/MacOS/terax-cli
codesign --force --sign - --entitlements src-tauri/entitlements.plist src-tauri/target/release/bundle/macos/Terax.app
codesign --verify --deep --strict src-tauri/target/release/bundle/macos/Terax.app
open src-tauri/target/release/bundle/macos/Terax.app    # NOT run by me
```
The release recipe (minus `--ignore-version-mismatches`) is documented in docs/architecture/ghostty-release-readiness.md:326-329.

## 5. Special requirements for dev and release

- Env vars: none required for dev. `TAURI_DEV_HOST` (vite.config.ts:10) only for mobile/remote HMR. Release with updater artifacts needs `TAURI_SIGNING_PRIVATE_KEY` (+ `_PASSWORD`) because `bundle.createUpdaterArtifacts: true` (tauri.conf.json:49) and an updater pubkey is configured (line 120); the config override above disables that. Real signing/notarization env (APPLE_CERTIFICATE, APPLE_SIGNING_IDENTITY, APPLE_TEAM_ID, APPLE_API_ISSUER/KEY/KEY_PATH) is only used in .github/workflows/release.yml:66-90 via tauri-action; irrelevant for a personal fork build, ad-hoc signing is enough locally (Gatekeeper may need right-click Open or `xattr -dr com.apple.quarantine` only if the app is transferred/downloaded, not for a locally built one; inferred).
- Entitlements: src-tauri/entitlements.plist = camera + audio-input only (voice input). Must be passed to the ad-hoc `codesign --entitlements` step or mic/camera prompts will not work in the release build.
- `macOSPrivateApi: true` + transparent window (tauri.conf.json:14): not App Store eligible, irrelevant here.
- Sidecar: tauri.conf.json:39 `externalBin: binaries/terax-cli`. src-tauri/build.rs:6-31 strips externalBin for debug builds when the sidecar is missing, so `cargo check/clippy/test` work without it; a release profile build PANICS if src-tauri/binaries/terax-cli-<triple> is missing (run `pnpm build:cli`, which `tauri build` does automatically via beforeBuildCommand).
- Port 1420 is `strictPort` (vite.config.ts:146-147): a second `tauri dev` or a leftover vite fails instead of picking another port.
- Changing the `--config` override between runs forces tauri crates to recompile (saw 17.8 s) because the config is embedded at build time.

## 6. Blockers and fixes (all verified unless marked)

B1. Tauri JS/Rust version mismatch (real, from upstream HEAD drift). Cargo.lock was bumped by dependabot commit 3301de2 (tauri 2.12.0, plugins up to 2.13.1/2.10.0/...), but package.json (lines 72-82) still has older `@tauri-apps/*` (api ^2.11.1 resolved 2.11.1, plugin-store ~2.4.5 vs crate 2.5.0, plugin-updater ~2.12.0 vs 2.13.1, plugin-autostart ~2.5.1 vs 2.6.0, plugin-log ~2.9.2 vs 2.10.0, plugin-os ~2.3.2 vs 2.4.0, plugin-process ~2.3.1 vs 2.4.0, plugin-window-state ~2.4.1 vs 2.5.0, plugin-notification ^2.4.0 vs 2.5.0, plugin-opener ^2.5.5 vs 2.7.0, plugin-clipboard-manager ^2.3.3 vs 2.4.0). Effect: `pnpm tauri build` aborts immediately with "Found version mismatched Tauri packages" (exit 1). `pnpm tauri dev` prints the same text as "Error" but CONTINUES and runs fine. Fixes: (a) no-source-change: add `--ignore-version-mismatches` to `tauri build` (used above, works); (b) proper: bump the 11 npm packages in package.json + pnpm-lock.yaml to the current npm latest, which already match the crate minors (api 2.12.1, process 2.4.0, autostart 2.7.0, notification 2.5.1, log 2.10.0, store 2.5.0, updater 2.13.2, opener 2.7.0, clipboard-manager 2.4.1, os 2.4.0, window-state 2.5.0, cli 2.12.1). pnpm-workspace.yaml has `minimumReleaseAge: 4320` + Strict, so a version younger than 3 days cannot be installed. (b) is a tracked-file change and should be a deliberate first task or skipped; do not mix it into feature work.

B2. Wrong node / recursive lazy-nvm functions / no pnpm: see section 2.

B3. Shared identity with the installed app. /Applications/Terax.app is installed (v0.8.6, same bundle id app.crynta.terax). A dev or release run with the default identifier uses the same ~/Library/Application Support/app.crynta.terax (settings, sessions, spaces, window-state; mtimes 8 Oct 17:06) and may migrate it to the 0.9.0-beta schema (downgrade risk for the daily driver). Verified mitigation: `--config '{"identifier":"app.crynta.terax.dev"}'` created a separate ~/Library/Application Support/app.crynta.terax.dev and left the original untouched. Not isolated by that override: OS keychain service constant `terax-ai` (src-tauri/src/modules/secrets.rs), and ~/Library/Caches/terax/control.json (src-tauri/src/modules/control.rs:569-582, path independent of identifier, so a running dev instance overwrites the daily-driver CLI control descriptor). No single-instance plugin exists, so both can run side by side. Recommend quitting the installed Terax while testing, or backing up `~/Library/Application Support/app.crynta.terax`. The release build above is NOT isolated (identifier unchanged) per the release-readiness doc.

B4. Orphaned debug app after dev stop. SIGINT to `pnpm tauri dev` stopped vite and cargo but left `target/debug/terax` running as ppid 1 (TERAX.md "Known gotchas" already notes this). It also leaves a stale ~/Library/Caches/terax/control.json and run/<pid> dir (I removed mine). Kill by path (command in section 4).

B5. Single cargo lock. All cargo profiles share src-tauri/target (now 8.0 GB). Concurrent agents running cargo will block on "waiting for file lock on build directory". Options: serialize Rust verification in one integration step; or give each agent `CARGO_TARGET_DIR=/path/own-target` (cold build ~3 min, ~4 GB each); or git worktrees (each needs `pnpm install --frozen-lockfile` ~30 s with the warm store at ~/Library/pnpm/store plus its own cold target). Frontend checks are cheap and parallel-safe, but `pnpm build` writes dist/ (collision) and `pnpm tauri dev` binds fixed port 1420 (only one dev session at a time).

B6. Runtime warning at baseline (not a regression): on this macOS 27 WebKit the WebGPU renderer fails shader validation ("call to 'textureSample' requires uniform control flow") and Terax falls back to WebGL automatically (log: "WebGPU initialization failed; preserving the Ghostty session with WebGL"). Terminal works. Relevant to the terminal click-to-position task: test against the WebGL renderer on this machine.

B7. Not verified: no interactive GUI check was done (no screenshots, no click/typing in the window), keychain prompt behaviour for the unsigned debug binary, nor the installed app's `terax` CLI at ~/.local/bin/terax interplay.

## 7. Notes for splitting work into subagents

- Per-task verification budget: frontend gate ~20 s; rust gate warm ~15 s + test 20-80 s. Cheap enough to require before handoff.
- Rust changes touching pty/fs/workspace/shell/IPC/AI surface need a locking test per TERAX.md (docs/contributing/testing.md); a new Tauri command must be registered in src-tauri/src/lib.rs generate_handler! (around lines 236-330) and, for plugins, in src-tauri/capabilities/default.json.
- Bundle-size budget (.size-limit.json): total client JS is 1.47 MB gz of 1.5 MB. Any feature adding a library (e.g. an HTML preview sanitizer, an SSH helper UI) must be lazy-loaded or it will fail `pnpm size`; "eager-budget test" in editor also enforces lazy markdown chunk (TERAX.md editor section).
- Lint/format noise: keep the lint warning count at 90 or lower; do not repo-wide format.
- Conventions enforced by reviewers (TERAX.md:146-152): no em-dash, no emojis, `@/...` imports, pnpm only, minimal comments.


## keyFiles
- /Users/frider/Documents/iamcoder/terax_claude/package.json: scripts (dev/build/test/lint/check-types/build:cli*), engines node>=22 (line 9), packageManager pnpm@11.9.0 (line 7), @tauri-apps/* npm versions at lines 72-82 and 116 (mismatch source)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/tauri.conf.json: identifier app.crynta.terax (5), beforeDevCommand/beforeBuildCommand (7,9), macOSPrivateApi (14), externalBin sidecar (39), createUpdaterArtifacts true (49), macOS entitlements/min version (61-62), updater pubkey (120)
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/build.rs: strips externalBin for debug builds when sidecar missing; release panics if src-tauri/binaries/terax-cli-<triple> missing
- /Users/frider/Documents/iamcoder/terax_claude/scripts/build-cli.mjs: builds terax-cli and copies it to src-tauri/binaries/terax-cli-<triple>; honors TERAX_CLI_TARGET / CARGO_BUILD_TARGET
- /Users/frider/Documents/iamcoder/terax_claude/vite.config.ts: dev server port 1420 strictPort (146-147), React Compiler via babel plugin (slow build), manualChunks driving size budgets
- /Users/frider/Documents/iamcoder/terax_claude/.size-limit.json: bundle budgets; total client JS 1.5 MB gz is nearly exhausted (1.47 MB)
- /Users/frider/Documents/iamcoder/terax_claude/.github/workflows/ci.yml: canonical gate commands (lint, check-types, test, build, size, cargo check/clippy/nextest, fish script)
- /Users/frider/Documents/iamcoder/terax_claude/.github/workflows/release.yml: real signing/notarization env vars (APPLE_*, TAURI_SIGNING_*) used only in CI
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/entitlements.plist: camera + audio-input entitlements; must be supplied to ad-hoc codesign for release .app
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/Cargo.toml: workspace members (terax-cli, terax-control-protocol), release profile (fat LTO, opt-level s), tauri 2 / plugin crate versions
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/Cargo.lock: locked crate versions that cause the npm/crate mismatch (tauri 2.12.0, plugins)
- /Users/frider/Documents/iamcoder/terax_claude/pnpm-workspace.yaml: minimumReleaseAge 4320 + strict (blocks installing npm versions younger than 3 days), allowBuilds
- /Users/frider/Documents/iamcoder/terax_claude/src-tauri/src/modules/control.rs: control.json descriptor at ~/Library/Caches/terax/ (line 569-582) is shared between dev and installed app
- /Users/frider/Documents/iamcoder/terax_claude/docs/architecture/ghostty-release-readiness.md: documents the exact local release build + ad-hoc codesign recipe (lines 326-329) and says the build shares app identifier/settings
- /Users/frider/Documents/iamcoder/terax_claude/docs/contributing/testing.md: testing contract; which changes require locking tests
- /private/tmp/claude-501/-Users-frider-Documents-iamcoder-terax-claude/c2d597b7-b40d-4501-a701-a769049d2877/scratchpad/env.sh: working shell environment (unset lazy nvm functions, node 22, cargo, pnpm shim); source it before any pnpm/node/cargo command
- /private/tmp/claude-501/-Users-frider-Documents-iamcoder-terax-claude/c2d597b7-b40d-4501-a701-a769049d2877/scratchpad/bin/pnpm: corepack-backed pnpm 11.9.0 shim so child shells (sh -c 'pnpm ...') find pnpm

## integrationPoints
- Every implementation subagent should start each Bash call with: source <scratchpad>/env.sh (or the 3-line snippet: unset -f node npm npx nvm yarn pnpm; PATH with ~/.nvm/versions/node/v22.22.3/bin and ~/.cargo/bin; COREPACK_ENABLE_DOWNLOAD_PROMPT=0) plus a real pnpm executable on PATH
- Definition-of-done gate per task: frontend `pnpm lint && pnpm check-types && pnpm test` (~20 s) and, if Rust touched, `cd src-tauri && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked` (~1.5 min warm); compare against the baseline counts in section 3 (lint warnings must stay <= 90, vitest 1209 tests passing, cargo 373 passing)
- Any task adding a frontend dependency or a large lazy chunk must also run `pnpm build && pnpm size` (budget headroom ~30 kB gz total)
- Any task adding a Tauri command: register in src-tauri/src/lib.rs generate_handler!; plugins also need Cargo.toml dep, .plugin() call, and capability entry in src-tauri/capabilities/default.json; rebuild is picked up by `tauri dev` automatically
- Manual GUI verification: run `pnpm tauri dev --config '{"identifier":"app.crynta.terax.dev"}'` from one designated agent/terminal (port 1420 and the shared cargo target lock allow only one), then stop it and kill target/debug/terax by path
- Final integration step: release build with the verified `tauri build --ignore-version-mismatches --bundles app --config {createUpdaterArtifacts:false}` + ad-hoc codesign commands, output at src-tauri/target/release/bundle/macos/Terax.app
- Parallelism: frontend-only subagents can run concurrently; Rust subagents should either share one serialized verification step or each use a distinct CARGO_TARGET_DIR (or git worktree with its own pnpm install and target)
- Optional early housekeeping task (tracked-file change, decide first): bump the 11 @tauri-apps/* npm packages in package.json/pnpm-lock.yaml to match the crate minors so `tauri build` works without the ignore flag

## risks
- Default Bash PATH has node v20.16.0 and recursive lazy-nvm shell functions; commands run without the env fix fail with `maximum nested function level reached` or wrong-node errors
- pnpm is not installed anywhere; only corepack-on-node-22 can provide pnpm 11.9.0, and child `sh -c pnpm` calls (tauri beforeDevCommand/beforeBuildCommand) need a real executable, not a function
- Plain `pnpm tauri build` fails (npm/crate Tauri version mismatch from upstream HEAD) until `--ignore-version-mismatches` is used or npm deps are bumped; `tauri dev` prints the error but proceeds
- Dev/release builds share bundle id app.crynta.terax with the installed /Applications/Terax.app (0.8.6): default-identifier runs read and may migrate ~/Library/Application Support/app.crynta.terax; keychain service `terax-ai` and ~/Library/Caches/terax/control.json are shared even with the identifier override
- `pkill -x terax` would kill the user's daily-driver Terax.app; the orphaned dev binary must be killed by path (`pkill -f target/debug/terax`)
- Only one cargo build can hold src-tauri/target at a time (8 GB now); parallel Rust agents will block or need separate target dirs; vite port 1420 is strict so only one dev session at a time; concurrent `pnpm build` runs collide on dist/
- Bundle size budget is nearly exhausted (total client JS 1.47 of 1.5 MB gzip); HTML preview or SSH UI features can break `pnpm size` unless lazy-loaded
- Lint baseline 90 warnings and format baseline failures (112 biome errors, 28 rustfmt files) are pre-existing; running format --write or cargo fmt repo-wide would create huge unrelated diffs
- No GUI interaction was verified (only process start and log inspection); WebGPU renderer fails on this macOS 27 WebKit and falls back to WebGL, so rendering behavior tests should target WebGL
- Release timing numbers include warm dependency caches; a cold clone needs ~3 min extra for crate download and compile, and slow network (~50 KiB/s npm tarballs) can dominate installs
- Not covered by baseline: cargo-nextest results (not installed), fish integration test (fish not installed), pnpm audit, soak/profile/bench scripts

## openQuestions
- Should the 11 @tauri-apps/* npm packages be bumped in the fork now (clean `tauri build`, tracked package.json/pnpm-lock.yaml change) or should the fork keep using `--ignore-version-mismatches`?
- Should the user install a permanent pnpm (`nvm use 22 && corepack enable pnpm`) and/or `brew install cargo-nextest` so later runs match CI exactly, or keep the session-scoped scratchpad shim?
- Should the fork's dev profile permanently use a distinct identifier (e.g. app.crynta.terax.dev via a tauri.dev.conf.json passed with --config) to protect the installed 0.9/0.8.6 daily-driver data, and what identifier/name should a personal release build of the fork use so it can coexist with the stock app?
- For parallel Rust subagents: serialize cargo verification, use per-agent CARGO_TARGET_DIR, or use git worktrees (cost: ~4 GB and ~3 min cold build each)?
- Is GUI-level verification expected from implementation agents (computer-use against the dev window), or is the lint/type/test/cargo gate plus a final manual check by the user sufficient?
- Should the fork enable Apple Developer signing/notarization, or is ad-hoc signing for a personal local build enough (current assumption)?
- Install the freshly built src-tauri/target/release/bundle/macos/Terax.app over /Applications/Terax.app (0.8.6) or run it side by side? I did not launch or install it.
