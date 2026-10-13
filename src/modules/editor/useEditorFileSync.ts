import {
  listenFsChanged,
  parentDir,
  watchAdd,
  watchRemove,
} from "@/modules/explorer/lib/watch";
import { envForSpace } from "@/modules/spaces";
import type { Tab } from "@/modules/tabs";
import {
  fsEventInEnv,
  parseWorkspaceScopeKey,
  workspaceScopeKey,
} from "@/modules/workspace";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { type RefObject, useEffect, useRef } from "react";
import { toast } from "sonner";
import type { EditorPaneHandle } from "./EditorPane";

type Params = {
  tabs: Tab[];
  tabsRef: RefObject<Tab[]>;
  editorRefs: RefObject<Map<number, EditorPaneHandle>>;
};

/**
 * Keeps open editor tabs in sync with on-disk changes: reloads on applied AI
 * diffs, external writes, and fs-watch events, and maintains the watch set for
 * the directories of open editor files.
 */
export function useEditorFileSync({ tabs, tabsRef, editorRefs }: Params) {
  // When an AI diff is approved (write_file applied to disk), reload any
  // open editor tabs for that path so the user sees the new content. We
  // track which approvalIds we've already handled to fire the reload only
  // once per applied diff.
  const appliedDiffsRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    for (const t of tabs) {
      if (t.kind !== "ai-diff") continue;
      if (t.status !== "approved") continue;
      if (appliedDiffsRef.current.has(t.approvalId)) continue;
      appliedDiffsRef.current.add(t.approvalId);
      for (const e of tabs) {
        if (e.kind !== "editor") continue;
        if (e.path !== t.path) continue;
        editorRefs.current.get(e.id)?.reload();
      }
    }
  }, [tabs, editorRefs]);

  useEffect(() => {
    type FileWrittenPayload = {
      path: string;
      env?: string;
      source?: string;
      warning?: string;
    };
    const unlistenPromise =
      getCurrentWebviewWindow().listen<FileWrittenPayload>(
        "fs:file-written",
        (event) => {
          if (event.payload.warning) toast.warning(event.payload.warning);
          if (event.payload.source === "editor") return;
          const normalizedPath = event.payload.path.replace(/\\/g, "/");
          const currentTabs = tabsRef.current;
          for (const t of currentTabs) {
            if (t.kind !== "editor") continue;
            if (!fsEventInEnv(event.payload.env, envForSpace(t.spaceId)))
              continue;
            if (t.path.replace(/\\/g, "/") === normalizedPath) {
              editorRefs.current.get(t.id)?.reload();
            }
          }
        },
      );
    return () => {
      void unlistenPromise.then((un) => un());
    };
  }, [tabsRef, editorRefs]);

  const editorWatchRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    const want = new Set<string>();
    for (const t of tabs) {
      if (t.kind !== "editor" || !t.path) continue;
      const scope = workspaceScopeKey(envForSpace(t.spaceId));
      want.add(`${scope}\0${parentDir(t.path)}`);
    }
    const prev = editorWatchRef.current;
    const apply = (
      keys: string[],
      fn: (
        paths: string[],
        env: ReturnType<typeof parseWorkspaceScopeKey>,
      ) => void,
    ) => {
      const byScope = new Map<string, string[]>();
      for (const k of keys) {
        const i = k.indexOf("\0");
        const scope = k.slice(0, i);
        byScope.set(scope, [...(byScope.get(scope) ?? []), k.slice(i + 1)]);
      }
      for (const [scope, dirs] of byScope)
        fn(dirs, parseWorkspaceScopeKey(scope));
    };
    apply(
      [...want].filter((d) => !prev.has(d)),
      watchAdd,
    );
    apply(
      [...prev].filter((d) => !want.has(d)),
      watchRemove,
    );
    editorWatchRef.current = want;
  }, [tabs]);

  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    void listenFsChanged((paths, eventEnv) => {
      const changed = new Set(paths.map((p) => p.replace(/\\/g, "/")));
      for (const t of tabsRef.current) {
        if (t.kind !== "editor") continue;
        if (!fsEventInEnv(eventEnv, envForSpace(t.spaceId))) continue;
        if (changed.has(t.path.replace(/\\/g, "/"))) {
          editorRefs.current.get(t.id)?.reload();
        }
      }
    }).then((un) => {
      if (alive) unlisten = un;
      else un();
    });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [tabsRef, editorRefs]);
}
