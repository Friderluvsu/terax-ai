import { describe, expect, it } from "vitest";

import {
  terminalDeleteSequence,
  terminalForwardDeleteSequence,
  terminalHomeEndSequence,
  terminalLineNavigationSequence,
  terminalReadlineSequence,
  terminalUndoSequence,
  terminalWordNavigationSequence,
  type TerminalKeyEvent,
} from "./keymap";

const evt = (partial: Partial<TerminalKeyEvent>): TerminalKeyEvent => ({
  altKey: false,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  key: "",
  code: "",
  ...partial,
});

describe("terminalUndoSequence", () => {
  it("maps Cmd+Z to readline undo on macOS", () => {
    expect(
      terminalUndoSequence(evt({ metaKey: true, key: "z", code: "KeyZ" }), {
        isMac: true,
      }),
    ).toBe("\x1f");
  });

  it("does not map Cmd+Shift+Z (redo)", () => {
    expect(
      terminalUndoSequence(
        evt({ metaKey: true, shiftKey: true, key: "Z", code: "KeyZ" }),
        { isMac: true },
      ),
    ).toBeNull();
  });

  it("does not map Cmd+Z off macOS", () => {
    expect(
      terminalUndoSequence(evt({ metaKey: true, key: "z", code: "KeyZ" }), {
        isMac: false,
      }),
    ).toBeNull();
  });

  it("does not map plain z", () => {
    expect(
      terminalUndoSequence(evt({ key: "z", code: "KeyZ" }), { isMac: true }),
    ).toBeNull();
  });
});

describe("terminalForwardDeleteSequence", () => {
  it("maps Option+Fn+Delete to kill-word-forward", () => {
    expect(
      terminalForwardDeleteSequence(
        evt({ altKey: true, key: "Delete", code: "Delete" }),
        { isMac: true },
      ),
    ).toBe("\x1bd");
  });

  it("maps Cmd+Fn+Delete to kill-to-line-end on macOS", () => {
    expect(
      terminalForwardDeleteSequence(
        evt({ metaKey: true, key: "Delete", code: "Delete" }),
        { isMac: true },
      ),
    ).toBe("\x0b");
  });

  it("does not map Cmd+Fn+Delete off macOS", () => {
    expect(
      terminalForwardDeleteSequence(
        evt({ metaKey: true, key: "Delete", code: "Delete" }),
        { isMac: false },
      ),
    ).toBeNull();
  });

  it("does not map Cmd+Option+Fn+Delete", () => {
    expect(
      terminalForwardDeleteSequence(
        evt({ metaKey: true, altKey: true, key: "Delete", code: "Delete" }),
        { isMac: true },
      ),
    ).toBeNull();
  });

  it("does not map plain Delete", () => {
    expect(
      terminalForwardDeleteSequence(evt({ key: "Delete", code: "Delete" }), {
        isMac: true,
      }),
    ).toBeNull();
  });

  it("does not map Backspace", () => {
    expect(
      terminalForwardDeleteSequence(
        evt({ altKey: true, key: "Backspace", code: "Backspace" }),
        { isMac: true },
      ),
    ).toBeNull();
  });
});

describe("terminalHomeEndSequence", () => {
  it("maps Home to line-start", () => {
    expect(terminalHomeEndSequence(evt({ key: "Home", code: "Home" }))).toBe(
      "\x01",
    );
  });

  it("maps End to line-end", () => {
    expect(terminalHomeEndSequence(evt({ key: "End", code: "End" }))).toBe(
      "\x05",
    );
  });

  it("does not map Shift+Home", () => {
    expect(
      terminalHomeEndSequence(
        evt({ shiftKey: true, key: "Home", code: "Home" }),
      ),
    ).toBeNull();
  });

  it("does not map Ctrl+End", () => {
    expect(
      terminalHomeEndSequence(evt({ ctrlKey: true, key: "End", code: "End" })),
    ).toBeNull();
  });
});

describe("terminalWordNavigationSequence", () => {
  it("maps Option+Left to readline word-left", () => {
    expect(
      terminalWordNavigationSequence(
        evt({ altKey: true, key: "ArrowLeft", code: "ArrowLeft" }),
      ),
    ).toBe("\x1bb");
  });

  it("maps Option+Right to readline word-right", () => {
    expect(
      terminalWordNavigationSequence(
        evt({ altKey: true, key: "ArrowRight", code: "ArrowRight" }),
      ),
    ).toBe("\x1bf");
  });

  it("does not remap plain arrows", () => {
    expect(
      terminalWordNavigationSequence(
        evt({ key: "ArrowLeft", code: "ArrowLeft" }),
      ),
    ).toBeNull();
  });
});

describe("terminalLineNavigationSequence", () => {
  it("maps Cmd+Left to readline line-start on macOS", () => {
    expect(
      terminalLineNavigationSequence(
        evt({ metaKey: true, key: "ArrowLeft", code: "ArrowLeft" }),
        { isMac: true },
      ),
    ).toBe("\x01");
  });

  it("maps Cmd+Right to readline line-end on macOS", () => {
    expect(
      terminalLineNavigationSequence(
        evt({ metaKey: true, key: "ArrowRight", code: "ArrowRight" }),
        { isMac: true },
      ),
    ).toBe("\x05");
  });

  it("does not remap Cmd+Arrow off macOS", () => {
    expect(
      terminalLineNavigationSequence(
        evt({ metaKey: true, key: "ArrowLeft", code: "ArrowLeft" }),
        { isMac: false },
      ),
    ).toBeNull();
  });

  it("does not remap Cmd+Option+Arrow (selection-style combos pass through)", () => {
    expect(
      terminalLineNavigationSequence(
        evt({
          metaKey: true,
          altKey: true,
          key: "ArrowLeft",
          code: "ArrowLeft",
        }),
        { isMac: true },
      ),
    ).toBeNull();
  });
});

describe("terminalDeleteSequence", () => {
  it("maps Cmd+Backspace to kill-to-line-start on macOS", () => {
    expect(
      terminalDeleteSequence(
        evt({ metaKey: true, key: "Backspace", code: "Backspace" }),
        { isMac: true },
      ),
    ).toBe("\x15");
  });

  it("maps Option+Backspace to kill-word-backward on macOS", () => {
    expect(
      terminalDeleteSequence(
        evt({ altKey: true, key: "Backspace", code: "Backspace" }),
        { isMac: true },
      ),
    ).toBe("\x17");
  });

  it("maps Ctrl+Backspace to kill-word-backward off macOS", () => {
    expect(
      terminalDeleteSequence(
        evt({ ctrlKey: true, key: "Backspace", code: "Backspace" }),
        { isMac: false },
      ),
    ).toBe("\x17");
  });

  it("does not remap Ctrl+Backspace on macOS (reserved for native readline binding)", () => {
    expect(
      terminalDeleteSequence(
        evt({ ctrlKey: true, key: "Backspace", code: "Backspace" }),
        { isMac: true },
      ),
    ).toBeNull();
  });

  it("does not remap Cmd+Backspace off macOS", () => {
    expect(
      terminalDeleteSequence(
        evt({ metaKey: true, key: "Backspace", code: "Backspace" }),
        { isMac: false },
      ),
    ).toBeNull();
  });

  it("does not remap plain Backspace", () => {
    expect(
      terminalDeleteSequence(evt({ key: "Backspace", code: "Backspace" }), {
        isMac: true,
      }),
    ).toBeNull();
  });
});

describe("terminalReadlineSequence", () => {
  const remaps = [
    [
      "line navigation",
      evt({ metaKey: true, key: "ArrowLeft", code: "ArrowLeft" }),
      "\x01",
    ],
    [
      "word navigation",
      evt({ altKey: true, key: "ArrowRight", code: "ArrowRight" }),
      "\x1bf",
    ],
    [
      "deletion",
      evt({ metaKey: true, key: "Backspace", code: "Backspace" }),
      "\x15",
    ],
    ["undo", evt({ metaKey: true, key: "z", code: "KeyZ" }), "\x1f"],
    [
      "option forward deletion",
      evt({ altKey: true, key: "Delete", code: "Delete" }),
      "\x1bd",
    ],
    [
      "cmd forward deletion",
      evt({ metaKey: true, key: "Delete", code: "Delete" }),
      "\x0b",
    ],
    ["home", evt({ key: "Home", code: "Home" }), "\x01"],
    ["end", evt({ key: "End", code: "End" }), "\x05"],
  ] as const;

  it.each(remaps)(
    "applies %s on the normal screen",
    (_name, event, sequence) => {
      expect(
        terminalReadlineSequence(event, {
          isMac: true,
          isAlternateScreen: false,
        }),
      ).toBe(sequence);
    },
  );

  it.each(remaps)("suppresses %s on the alternate screen", (_name, event) => {
    expect(
      terminalReadlineSequence(event, {
        isMac: true,
        isAlternateScreen: true,
      }),
    ).toBeNull();
  });
});
