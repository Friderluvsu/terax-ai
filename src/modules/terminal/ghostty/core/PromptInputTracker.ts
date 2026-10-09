import type { GhosttyTerminalEvent } from "@terax/ghostty-core/protocol";

export type PromptPhase = "idle" | "prompt" | "input" | "running";

export class PromptInputTracker {
  private current: PromptPhase = "idle";

  phase(): PromptPhase {
    return this.current;
  }

  handle(event: Pick<GhosttyTerminalEvent, "type">): void {
    switch (event.type) {
      case "prompt-start":
        this.current = "prompt";
        break;
      case "prompt-end":
        this.current = "input";
        break;
      case "end-of-input":
        this.current = "running";
        break;
      case "end-of-command":
      case "screen-cleared":
      case "overflow":
        this.current = "idle";
        break;
    }
  }

  reset(): void {
    this.current = "idle";
  }
}
