export type LongCommandEvent = {
  leafId: number;
  command: string;
  exitCode: number | null;
  durationMs: number;
};

type Listener = (event: LongCommandEvent) => void;

const listeners = new Set<Listener>();

export function onLongCommand(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function emitLongCommand(event: LongCommandEvent): void {
  for (const listener of listeners) listener(event);
}
