# FORK.md

Personal fork of crynta/terax-ai. TERAX.md is the architecture doc and quality bar; this file adds fork rules. The original task cards are in `fork-plan/PLAN.md`; read [the plan index](fork-plan/README.md) for their historical scope. Current fork guides and validation reports are indexed in [docs/fork/README.md](docs/fork/README.md), and maintained commands in [scripts/README.md](scripts/README.md).

## Environment

- Run `source scripts/fork/agent-env.sh` before any node, pnpm or cargo command. The user's shell defines lazy nvm functions that recurse in tool shells, and the default node is too old.
- `ls` is aliased to eza with icon glyphs; use `/bin/ls`, `find` or `rg`.
- Dev app: `pnpm fork:dev`. Port 1420 is strict: one dev app at a time.
- Release build: `pnpm fork:release` (add `-- --install` to copy into /Applications). The DMG lands in `release/Terax-<version>-<arch>.dmg` (gitignored).
- Stop an orphaned dev binary by path only: `pkill -f target/debug/terax`. Never `pkill -x terax`: it kills the user's installed Terax.

## Gates and baseline

- FE: `pnpm lint && pnpm check-types && pnpm test` (baseline: 0 lint errors, 90 warnings max, 1209 tests).
- RS: `cd src-tauri && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked` (baseline 373 passed).
- SIZE: `pnpm build && pnpm size` when frontend code is added. Total client JS is capped at 1505 kB gzip; measure current headroom instead of relying on older estimates. New UI goes behind `lazy()`; this reduces startup loading but all lazy chunks still count toward the total.
- Format only the files you touched (`pnpm exec biome format --write <files>`, `rustfmt <files>`). The repo does not pass a repo-wide format check; never reformat everything.

## Parallel work

- Each task lives on branch `task/<ID>` cut from `fork/main`; parallel tasks use separate git worktrees (`pnpm install --frozen-lockfile --offline` first).
- One shared `src-tauri/target` holds a cargo lock. A Rust task in a worktree sets `CARGO_TARGET_DIR=$HOME/Library/Caches/terax-fork/target-A` or `target-B` as assigned; the main checkout target belongs to the integrator.

## Fork conventions

- Fork code goes into new modules (`src-tauri/src/modules/remote/*`, `src/modules/remote/*`, `src/modules/html-preview/*`, ...); edits to hot upstream files (App.tsx, useTabs.ts, settings/store.ts, lib.rs, useGhosttyTerminalSession.ts) stay minimal.
- Fork preferences live in `src/modules/settings/store.ts` and are shown in the Fork settings section.
- Upstream sync: `git fetch upstream && git merge upstream/main` into `fork/main`, then run all gates.
