#!/usr/bin/env bash
# Build "Terax Fork.app" with ad-hoc signing. `--install` copies it to /Applications.
# Never touches /Applications/Terax.app (the stock app).
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
source "$repo/scripts/fork/agent-env.sh"
cd "$repo"

install=0
for arg in "$@"; do
  case "$arg" in
    --install) install=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

app_name="Terax Fork.app"
bundle="$repo/src-tauri/target/release/bundle/macos/$app_name"

pnpm fork:build

[ -d "$bundle" ] || { echo "bundle not found: $bundle" >&2; exit 1; }

codesign --force --sign - "$bundle/Contents/MacOS/terax-cli"
codesign --force --sign - --entitlements "$repo/src-tauri/entitlements.plist" "$bundle"
codesign --verify --deep --strict "$bundle"

if [ "$install" -eq 1 ]; then
  dest="/Applications/$app_name"
  pkill -f "$dest/Contents/MacOS/" 2>/dev/null || true
  rm -rf "$dest"
  ditto "$bundle" "$dest"
  echo "installed: $dest"
fi

echo "bundle: $bundle ($(du -sh "$bundle" | cut -f1))"
