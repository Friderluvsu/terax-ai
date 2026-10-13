import { notifyDocumentSaved } from "@/modules/lsp";
import { usePreferencesStore } from "@/modules/settings/preferences";
import type { FsReadResult, WorkspaceEnv } from "@/modules/workspace";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { documentIo } from "./documentIo";
import { mtimeChanged } from "./mtime";
import { detectEol, type Eol, normalizeToLf, restoreEol } from "./eol";
import { isBlankText } from "./scratch";
import { readScratch, scheduleScratchWrite } from "./scratchStore";

type ReadResult = FsReadResult;

/// Mirrors FORCE_MAX_READ_BYTES in src-tauri fs/file.rs.
export const FORCE_READ_LIMIT = 50 * 1024 * 1024;

export type DocumentState =
  | { status: "loading" }
  | { status: "ready"; content: string; size: number }
  | { status: "binary"; size: number }
  | { status: "toolarge"; size: number; limit: number }
  | { status: "error"; message: string };

type Options = {
  path: string;
  env: WorkspaceEnv;
  onDirtyChange?: (dirty: boolean) => void;
  /** Untitled buffer: loads from and writes through app-data scratch storage. */
  scratchId?: string;
  onEmptyChange?: (empty: boolean) => void;
};

export function useDocument({
  path,
  env,
  onDirtyChange,
  scratchId,
  onEmptyChange,
}: Options) {
  const [doc, setDoc] = useState<DocumentState>({ status: "loading" });
  const [dirty, setDirty] = useState(false);

  const autoSave = usePreferencesStore((s) => s.editorAutoSave);
  const autoSaveDelay = usePreferencesStore((s) => s.editorAutoSaveDelay);

  // Track the saved buffer so we can detect changes cheaply.
  const savedRef = useRef<string>("");
  const bufferRef = useRef<string>("");
  const eolRef = useRef<Eol>("\n");
  const dirtyRef = useRef(false);
  useEffect(() => {
    dirtyRef.current = dirty;
  }, [dirty]);

  const autoSaveRef = useRef({ autoSave, autoSaveDelay });
  autoSaveRef.current = { autoSave, autoSaveDelay };

  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const clearAutoSaveTimer = useCallback(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
  }, []);

  const diskMtimeRef = useRef<number | null>(null);
  const scratchIdRef = useRef(scratchId);
  scratchIdRef.current = scratchId;
  const onEmptyChangeRef = useRef(onEmptyChange);
  onEmptyChangeRef.current = onEmptyChange;
  const emptyRef = useRef<boolean | null>(null);
  // Set by a Save As so the switch to the new path keeps the live buffer.
  const adoptedPathRef = useRef<string | null>(null);

  const reportEmpty = useCallback((text: string) => {
    const empty = isBlankText(text);
    if (empty === emptyRef.current) return;
    emptyRef.current = empty;
    onEmptyChangeRef.current?.(empty);
  }, []);

  const writeToDisk = useCallback(async () => {
    const content = bufferRef.current;
    const mtime = await documentIo(env, path).write(
      restoreEol(content, eolRef.current),
    );
    diskMtimeRef.current = mtime;
    savedRef.current = content;
    // Edits typed while the write was in flight must stay dirty.
    setDirty(bufferRef.current !== content);
    notifyDocumentSaved(path);
  }, [path, env]);

  // False when the write was withheld because the file changed on disk
  // since load; overwriting is an explicit user action from the toast.
  const saveNow = useCallback(async (): Promise<boolean> => {
    const known = diskMtimeRef.current;
    if (known !== null) {
      const stat = await documentIo(env, path)
        .stat()
        .catch(() => null);
      if (stat && mtimeChanged(env, known, stat.mtime)) {
        const name = path.split(/[\\/]/).pop() ?? path;
        toast.warning("File changed on disk", {
          id: `save-conflict:${path}`,
          description: `${name} was modified by another program while you had unsaved changes. Overwrite to keep your version.`,
          action: { label: "Overwrite", onClick: () => void writeToDisk() },
        });
        return false;
      }
    }
    await writeToDisk();
    return true;
  }, [path, env, writeToDisk]);

  // Notify parent of dirty transitions.
  const onDirtyChangeRef = useRef(onDirtyChange);
  useEffect(() => {
    onDirtyChangeRef.current = onDirtyChange;
  }, [onDirtyChange]);
  useEffect(() => {
    onDirtyChangeRef.current?.(dirty);
  }, [dirty]);

  const forceRef = useRef(false);

  // Adopts a read result as the new saved baseline. `skipIfUnchanged` avoids
  // the re-render when disk already matches the buffer (self-save / duplicate
  // watcher event); initial loads must always publish a state.
  const adoptRead = useCallback((res: ReadResult, skipIfUnchanged = false) => {
    if (res.kind === "text") {
      eolRef.current = detectEol(res.content);
      diskMtimeRef.current = res.mtime;
      const content = normalizeToLf(res.content);
      if (skipIfUnchanged && content === savedRef.current) return;
      savedRef.current = content;
      bufferRef.current = content;
      setDirty(false);
      setDoc({ status: "ready", content, size: res.size });
    } else if (res.kind === "binary") {
      setDoc({ status: "binary", size: res.size });
    } else if (res.kind === "toolarge") {
      setDoc({ status: "toolarge", size: res.size, limit: res.limit });
    }
  }, []);

  const readFromDisk = useCallback(
    (force: boolean): Promise<ReadResult> =>
      scratchId
        ? readScratch(scratchId).then((content) => ({
            kind: "text",
            content,
            size: content.length,
            mtime: 0,
          }))
        : documentIo(env, path).read(force),
    [path, env, scratchId],
  );

  // Load on path change.
  useEffect(() => {
    if (!scratchId && adoptedPathRef.current === path) {
      adoptedPathRef.current = null;
      return;
    }
    adoptedPathRef.current = null;
    let cancelled = false;
    // "Open anyway" is a per-file decision; a new path starts unforced.
    forceRef.current = false;
    setDoc({ status: "loading" });
    setDirty(false);

    readFromDisk(forceRef.current)
      .then((res) => {
        if (cancelled) return;
        adoptRead(res);
        if (scratchId && res.kind === "text") {
          diskMtimeRef.current = null;
          reportEmpty(res.content);
        }
      })
      .catch((e) => {
        if (!cancelled) setDoc({ status: "error", message: String(e) });
      });

    return () => {
      cancelled = true;
    };
  }, [readFromDisk, adoptRead, scratchId, path, reportEmpty]);

  const openAnyway = useCallback(() => {
    forceRef.current = true;
    setDoc({ status: "loading" });
    readFromDisk(true)
      .then(adoptRead)
      .catch((e) => setDoc({ status: "error", message: String(e) }));
  }, [readFromDisk, adoptRead]);

  // Skipped while dirty: never clobber unsaved edits. Re-checked when the
  // read resolves, since typing can start while it is in flight.
  const reload = useCallback((): boolean => {
    if (dirtyRef.current || scratchIdRef.current) return false;
    void readFromDisk(forceRef.current)
      .then((res) => {
        if (!dirtyRef.current) adoptRead(res, true);
      })
      // Transient failures (e.g. ENOENT mid atomic-rename) must not replace
      // a healthy buffer with an error screen.
      .catch((e) => console.warn("[editor] reload failed", path, e));
    return true;
  }, [readFromDisk, adoptRead, path]);

  const save = useCallback(async (): Promise<boolean> => {
    clearAutoSaveTimer();
    if (scratchIdRef.current) return false;
    if (bufferRef.current === savedRef.current) return true;
    return saveNow();
  }, [clearAutoSaveTimer, saveNow]);

  // Adopt externally formatted disk content as the saved baseline before the
  // matching editor dispatch lands, so the buffer never flashes dirty. The
  // formatter's own write must also become the known mtime, or the next save
  // would report it as an external conflict.
  // Returns the LF-normalized text the caller should dispatch.
  const adoptDiskText = useCallback(
    (diskText: string, mtime: number): string => {
      eolRef.current = detectEol(diskText);
      diskMtimeRef.current = mtime;
      const content = normalizeToLf(diskText);
      savedRef.current = content;
      setDirty(bufferRef.current !== content);
      return content;
    },
    [],
  );

  // The buffer was just written to `nextPath`; the tab switches to it next.
  const adoptSavedAs = useCallback(
    (nextPath: string, content: string, mtime: number) => {
      adoptedPathRef.current = nextPath;
      eolRef.current = "\n";
      diskMtimeRef.current = mtime;
      savedRef.current = content;
      setDirty(bufferRef.current !== content);
    },
    [],
  );

  const onChange = useCallback(
    (next: string) => {
      bufferRef.current = next;
      const scratch = scratchIdRef.current;
      if (scratch) {
        scheduleScratchWrite(scratch, next);
        reportEmpty(next);
        return;
      }
      const isDirty = next !== savedRef.current;
      setDirty(isDirty);

      clearAutoSaveTimer();

      const { autoSave: active, autoSaveDelay: delay } = autoSaveRef.current;
      if (active && isDirty) {
        timeoutRef.current = setTimeout(() => {
          saveNow().catch((e) => console.error("[autosave]", e));
        }, delay);
      }
    },
    [clearAutoSaveTimer, saveNow, reportEmpty],
  );

  useEffect(() => clearAutoSaveTimer, [path, clearAutoSaveTimer]);

  return {
    doc,
    dirty,
    onChange,
    save,
    reload,
    adoptDiskText,
    adoptSavedAs,
    openAnyway,
  };
}
