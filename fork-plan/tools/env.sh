# source fork-plan/tools/env.sh before any node/pnpm/cargo command
_TERAX_TOOLS="$(cd "$(dirname "${BASH_SOURCE[0]:-${(%):-%x}}")" && pwd)"
unset -f node npm npx nvm yarn pnpm 2>/dev/null
export PATH="$_TERAX_TOOLS/bin:$HOME/.nvm/versions/node/v22.22.3/bin:$HOME/.cargo/bin:$PATH"
export COREPACK_ENABLE_DOWNLOAD_PROMPT=0
