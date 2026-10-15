# Build and diagnostic commands

Run commands from the repository root. The command definitions live in [package.json](../package.json); fork-specific rules live in [FORK.md](../FORK.md).

Load the local Node, pnpm and Rust environment before any Node, pnpm or Cargo command:

```bash
source scripts/fork/agent-env.sh
```

## Development and packaging

| Command | Result |
| --- | --- |
| `pnpm install --frozen-lockfile` | Install the workspace dependencies from the lockfile |
| `pnpm fork:dev` | Run the native development app with the fork's dev identity |
| `pnpm dev` | Run only Vite; native IPC requires the Tauri app |
| `pnpm fork:build` | Build the fork's macOS application bundle |
| `pnpm fork:release` | Build, ad-hoc sign and package `release/Terax-<version>-<arch>.dmg` |
| `pnpm fork:release -- --install` | Also install the release into `/Applications/Terax.app` |
| `pnpm build:cli` | Build the release CLI sidecar |
| `pnpm build:cli:dev` | Build the development CLI sidecar |

The release script is [fork/release.sh](fork/release.sh). It refuses to overwrite an application whose identifier differs from `dev.terax.fork`. Only one development app can use port 1420 at a time.

## Verification

```bash
source scripts/fork/agent-env.sh
pnpm lint
pnpm check-types
pnpm test
pnpm build
pnpm size
```

For backend changes:

```bash
source scripts/fork/agent-env.sh
cd src-tauri
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

See [FORK.md](../FORK.md) for the accepted warning baseline and [the testing guide](../docs/contributing/testing.md) for subsystem requirements. Format only touched files.

## Size and performance diagnostics

| Command | What it measures |
| --- | --- |
| `pnpm build` then `pnpm size` | Production JS and WASM gzip budgets from [.size-limit.mjs](../.size-limit.mjs) |
| `pnpm analyze:bundle` | Build with the bundle visualizer |
| `pnpm analyze:eager` | Static source dependency graphs for the main and settings windows |
| `pnpm bench:ghostty` | Terminal core microbenchmarks |
| `pnpm profile:ghostty` | Allocation and parsing workloads in isolated processes per WASM variant |
| `pnpm soak:ghostty` | Resource workloads against real WASM models without launching the app |
| `pnpm knip` | Unused dependency, file and export analysis |

[built-startup.mjs](built-startup.mjs) follows production HTML, preloads and static JS imports for the size gate. [eager-graph.mjs](eager-graph.mjs) inspects source imports. Use the built graph to establish actual startup size, since bundling can change which dependencies load eagerly.

Profiling and soak-test settings and measurement limits are documented in [resource efficiency](../docs/architecture/ghostty-resource-efficiency.md). Historical automation under `fork-plan/workflows/` is separate from these maintained package commands.
