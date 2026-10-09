import { pathAtOrUnder } from "@/lib/paths";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { useEffect, useRef } from "react";

export const RELOAD_DEBOUNCE_MS = 200;

export function shouldReloadFor(paths: readonly string[], root: string) {
  return paths.some((p) => pathAtOrUnder(p, root));
}

export type ReloadScheduler = {
  notify: () => void;
  setVisible: (visible: boolean) => void;
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
    dispose() {
      if (timer) clearTimeout(timer);
      timer = null;
    },
  };
}

export function useHtmlPreviewReload(
  root: string | null,
  visible: boolean,
  onReload: () => void,
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
    const handle = (paths: string[]) => {
      if (shouldReloadFor(paths, root)) scheduler.notify();
    };
    const written = win.listen<{ path: string }>("fs:file-written", (e) =>
      handle([e.payload.path]),
    );
    const changed = win.listen<{ paths: string[] }>("fs:changed", (e) =>
      handle(e.payload.paths),
    );
    return () => {
      scheduler.dispose();
      schedulerRef.current = null;
      void written.then((un) => un());
      void changed.then((un) => un());
    };
  }, [root]);

  useEffect(() => {
    schedulerRef.current?.setVisible(visible);
  }, [visible]);
}
