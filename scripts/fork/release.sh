#!/usr/bin/env bash
# Build the fork as "Terax.app" (identifier dev.terax.fork) with ad-hoc signing.
# `--install` copies it to /Applications, refusing to overwrite a different app
# (for example the stock Terax, identifier app.crynta.terax).
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

app_name="Terax.app"
fork_id="dev.terax.fork"
bundle="$repo/src-tauri/target/release/bundle/macos/$app_name"

pnpm fork:build

[ -d "$bundle" ] || { echo "bundle not found: $bundle" >&2; exit 1; }

codesign --force --sign - "$bundle/Contents/MacOS/terax-cli"
codesign --force --sign - --entitlements "$repo/src-tauri/entitlements.plist" "$bundle"
codesign --verify --deep --strict "$bundle"

if [ "$install" -eq 1 ]; then
  dest="/Applications/$app_name"
  if [ -d "$dest" ]; then
    current_id="$(defaults read "$dest/Contents/Info.plist" CFBundleIdentifier 2>/dev/null || true)"
    if [ "$current_id" != "$fork_id" ]; then
      echo "refusing to overwrite $dest (identifier '$current_id', expected '$fork_id'); move it away first" >&2
      exit 1
    fi
  fi
  osascript -e "tell application id \"$fork_id\" to quit" >/dev/null 2>&1 || true
  pkill -f "$dest/Contents/MacOS/" 2>/dev/null || true
  rm -rf "$dest"
  ditto "$bundle" "$dest"
  echo "installed: $dest"
fi

echo "bundle: $bundle ($(du -sh "$bundle" | cut -f1))"
