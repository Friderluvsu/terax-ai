import type { GhosttyTerminalEvent } from "@terax/ghostty-core/protocol";
import { decodeBlockCommand } from "../GhosttyBlocks";

export type FinishedCommand = {
  command: string;
  exitCode: number | null;
  durationMs: number;
};

export class LongCommandTracker {
  private startedAt: number | null = null;
  private command = "";

  handle(
    event: GhosttyTerminalEvent,
    now: number = Date.now(),
  ): FinishedCommand | null {
    switch (event.type) {
      case "end-of-input":
        this.startedAt = now;
        this.command = decodeBlockCommand(event.command ?? "").slice(0, 200);
        return null;
      case "end-of-command": {
        if (this.startedAt === null) return null;
        const done = {
          command: this.command,
          exitCode: event.exitCode,
          durationMs: Math.max(0, now - this.startedAt),
        };
        this.reset();
        return done;
      }
      case "prompt-start":
      case "screen-cleared":
      case "overflow":
        this.reset();
        return null;
      default:
        return null;
    }
  }

  reset(): void {
    this.startedAt = null;
    this.command = "";
  }
}
