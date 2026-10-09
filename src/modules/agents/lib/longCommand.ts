export type LongCommandInput = {
  command: string;
  exitCode: number | null;
  durationMs: number;
  thresholdSeconds: number;
  isPrivate: boolean;
};

export type LongCommandMessage = {
  kind: "finished" | "error";
  title: string;
  body: string;
};

export function formatCommandDuration(ms: number): string {
  const total = Math.round(ms / 1000);
  if (total < 60) return `${total}s`;
  const m = Math.floor(total / 60);
  const s = total % 60;
  if (m < 60) return s ? `${m}m ${s}s` : `${m}m`;
  const h = Math.floor(m / 60);
  return m % 60 ? `${h}h ${m % 60}m` : `${h}h`;
}

export function buildLongCommandMessage(
  input: LongCommandInput,
): LongCommandMessage | null {
  if (input.thresholdSeconds <= 0) return null;
  if (input.durationMs < input.thresholdSeconds * 1000) return null;
  const failed = input.exitCode !== null && input.exitCode !== 0;
  const name = input.isPrivate ? "" : input.command.trim().split("\n")[0];
  const status = failed ? `failed (exit ${input.exitCode})` : "finished";
  const subject = name ? name.slice(0, 80) : "Command";
  return {
    kind: failed ? "error" : "finished",
    title: `${subject} ${status}`,
    body: `Took ${formatCommandDuration(input.durationMs)}`,
  };
}
