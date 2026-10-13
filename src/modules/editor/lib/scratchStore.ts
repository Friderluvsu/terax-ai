import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import type { WorkspaceEnv } from "@/modules/workspace";
import {
  SCRATCH_DEBOUNCE_MS,
  type ScratchEntry,
  suggestedFileName,
  untitledCloseAction,
} from "./scratch";

type Pending = { text: string; timer: ReturnType<typeof setTimeout> };

const pending = new Map<string, Pending>();
// Per-id operation chain: a delete can never be overtaken by an older write.
const chains = new Map<string, Promise<void>>();
const warned = new Set<string>();

function enqueue(id: string, op: () => Promise<unknown>): Promise<void> {
  const prev = chains.get(id) ?? Promise.resolve();
  const next = prev.then(op).then(
    () => {
      warned.delete(id);
    },
    (e) => {
      console.error("[scratch]", e);
      if (warned.has(id)) return;
      warned.add(id);
      toast.error("Could not keep Untitled buffer", {
        description: String(e),
      });
    },
  );
  chains.set(id, next);
  void next.then(() => {
    if (chains.get(id) === next) chains.delete(id);
  });
  return next;
}

function takePending(id: string): string | undefined {
  const p = pending.get(id);
  if (!p) return undefined;
  clearTimeout(p.timer);
  pending.delete(id);
  return p.text;
}

export function scheduleScratchWrite(id: string, text: string): void {
  const prev = pending.get(id);
  if (prev) clearTimeout(prev.timer);
  pending.set(id, {
    text,
    timer: setTimeout(() => void flushScratch(id), SCRATCH_DEBOUNCE_MS),
  });
}

export function flushScratch(id: string): Promise<void> {
  const text = takePending(id);
  if (text === undefined) return chains.get(id) ?? Promise.resolve();
  return enqueue(id, () => invoke("scratch_write", { id, content: text }));
}

export async function flushAllScratch(): Promise<void> {
  const ids = new Set([...pending.keys(), ...chains.keys()]);
  await Promise.all([...ids].map(flushScratch));
}

/** Quit waits for the last write, but never hangs on a stuck IPC. */
export function flushScratchBeforeQuit(timeoutMs = 3000): Promise<void> {
  return Promise.race([
    flushAllScratch(),
    new Promise<void>((resolve) => setTimeout(resolve, timeoutMs)),
  ]);
}

export function discardScratch(id: string): Promise<void> {
  takePending(id);
  return enqueue(id, () => invoke("scratch_delete", { id }));
}

export async function readScratch(id: string): Promise<string> {
  const p = pending.get(id);
  if (p) return p.text;
  await chains.get(id);
  return (await invoke<string | null>("scratch_read", { id })) ?? "";
}

export function listScratch(): Promise<ScratchEntry[]> {
  return invoke<ScratchEntry[]>("scratch_list");
}

export function untitledCloseActionFor(
  scratchId: string,
  knownEmpty?: boolean,
) {
  return untitledCloseAction(pending.get(scratchId)?.text, knownEmpty, () =>
    readScratch(scratchId),
  );
}

/** Save As for a tab that may not be mounted, from its latest scratch text. */
export async function saveScratchAs(
  untitled: { scratchId: string; title: string; language?: string | null },
  env: WorkspaceEnv,
  root: string | null,
): Promise<boolean> {
  const [{ saveUntitledAs }, text] = await Promise.all([
    import("./untitledSave"),
    readScratch(untitled.scratchId),
  ]);
  const saved = await saveUntitledAs({
    env,
    root,
    name: suggestedFileName(untitled.title, untitled.language),
    text,
  });
  return saved !== null;
}
