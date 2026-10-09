import { pathAtOrUnder } from "@/lib/paths";
import { fsEventInEnv, type WorkspaceEnv } from "@/modules/workspace";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useCallback, useEffect, useRef } from "react";

export const RELOAD_DEBOUNCE_MS = 400;

export function shouldReloadFor(paths: readonly string[], root: string) {
  return paths.some((p) => pathAtOrUnder(p, root));
}

export type ReloadScheduler = {
  notify: () => void;
  setVisible: (visible: boolean) => void;
  discardPending: () => void;
  dispose: () => void;
};

export function createReloadScheduler(
  reload: () => void,
  delay = RELOAD_DEBOUNCE_MS,
  initialVisible = true,
): ReloadScheduler {
  let visible = initialVisible;
  let pending = false;
  let timer: ReturnType<typeof setTimeout> | null = null;
  const fire = () => {
    timer = null;
    if (!visible) return;
    pending = false;
    reload();
  };
  return {
    notify() {
      pending = true;
      if (!visible) return;
      if (timer) clearTimeout(timer);
      timer = setTimeout(fire, delay);
    },
    setVisible(next) {
      visible = next;
      if (next && pending && !timer) timer = setTimeout(fire, delay);
    },
    discardPending() {
      pending = false;
      if (timer) clearTimeout(timer);
      timer = null;
    },
    dispose() {
      if (timer) clearTimeout(timer);
      timer = null;
    },
  };
}

export type ReloadAction = "reserve" | "refresh";

export function reloadAction(error: string | null): ReloadAction {
  return error ? "reserve" : "refresh";
}

export function useHtmlPreviewReload(
  root: string | null,
  visible: boolean,
  onReload: () => void,
  env?: WorkspaceEnv,
) {
  const onReloadRef = useRef(onReload);
  onReloadRef.current = onReload;
  const schedulerRef = useRef<ReloadScheduler | null>(null);
  const visibleRef = useRef(visible);
  visibleRef.current = visible;

  useEffect(() => {
    if (!root) return;
    const scheduler = createReloadScheduler(
      () => onReloadRef.current(),
      RELOAD_DEBOUNCE_MS,
      visibleRef.current,
    );
    schedulerRef.current = scheduler;
    const win = getCurrentWebviewWindow();
    const handle = (paths: string[], eventEnv?: string) => {
      if (env && !fsEventInEnv(eventEnv, env)) return;
      if (shouldReloadFor(paths, root)) scheduler.notify();
    };
    const written = win.listen<{ path: string; env?: string }>(
      "fs:file-written",
      (e) => handle([e.payload.path], e.payload.env),
    );
    const changed = win.listen<{ paths: string[]; env?: string }>(
      "fs:changed",
      (e) => handle(e.payload.paths, e.payload.env),
    );
    return () => {
      scheduler.dispose();
      schedulerRef.current = null;
      void written.then((un) => un());
      void changed.then((un) => un());
    };
  }, [root, env]);

  useEffect(() => {
    schedulerRef.current?.setVisible(visible);
  }, [visible]);

  return useCallback(() => schedulerRef.current?.discardPending(), []);
}
