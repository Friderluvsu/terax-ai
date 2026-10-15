# Terminal links: plain click opens the system browser

## Problem

The selection controller cancels pointerdown to own text selection. Both terminal renderers previously activated links through mousedown and mouseup. Canceling pointerdown suppresses those compatibility mouse events, so an ordinary click could fail to open a URL. See the [Pointer Events compatibility mapping](https://www.w3.org/TR/pointerevents3/#compatibility-mapping-with-mouse-events).

## Change

WebGPU and WebGL now dispatch a verified selection-controller click through the same terminal-cell handler. It looks up the current cell's hyperlink and calls the existing native external opener. The previous parallel mouse activation handlers are removed.

The controller distinguishes clicks from drags, canceled gestures and repeated-click selection. Dragging URL text keeps selecting it. Prompt-input cursor placement retains its existing policy: Cmd/Ctrl+click opens links inside the editable command, while plain clicks there position the cursor. Output links open with an ordinary click.

URL detection, OSC 8 precedence, external scheme restrictions and application mouse reporting remain enforced by their existing boundaries.

## Verification, 2026-10-15

- Full frontend suite: 1622 tests passed.
- TypeScript: passed.
- Lint: no errors, 90 existing warnings.
- Production build and size gate: passed, total client JavaScript 1,489,117 bytes gzip against a 1,505,000 byte budget.
- Pointer interaction regressions cover activation without compatibility mouse events, dragging, cancellation, repeated-click selection and prompt cursor placement.
- The signed release was installed in /Applications/Terax.app. Ordinary clicks on both a printed HTTPS URL and an OSC 8 label opened the expected addresses in the default Google Chrome browser, without keyboard modifiers.
- The previous app is retained at /Users/frider/Library/Caches/terax-fork-backups/2026-10-15-112046/Terax.app. The release DMG is release/Terax-0.9.0-beta-arm64.dmg.
