# Source before any node/pnpm/cargo command: `source scripts/fork/agent-env.sh`
_terax_fork_dir="$(cd "$(dirname "${BASH_SOURCE[0]:-${(%):-%x}}")" && pwd)"
unset -f node npm npx nvm yarn pnpm 2>/dev/null
export PATH="$_terax_fork_dir/bin:$HOME/.nvm/versions/node/v22.22.3/bin:$HOME/.cargo/bin:$PATH"
export COREPACK_ENABLE_DOWNLOAD_PROMPT=0
export CARGO_INCREMENTAL=0
unset _terax_fork_dir
