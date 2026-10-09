import { currentWorkspaceEnv } from "@/modules/workspace";
import { Globe02Icon, LinkSquare02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import { reloadAction, useHtmlPreviewReload } from "./lib/useHtmlPreviewReload";

type Props = {
  path: string;
  rootHint?: string | null;
  visible: boolean;
};

type Served = { token: string; url: string; root: string };

const SUSPEND_AFTER_MS = 30_000;

export function HtmlPreviewFrame({ path, rootHint, visible }: Props) {
  const [served, setServed] = useState<Served | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [nonce, setNonce] = useState(0);
  const [loaded, setLoaded] = useState(visible);
  const [serveNonce, setServeNonce] = useState(0);
  const loadedRef = useRef(loaded);
  loadedRef.current = loaded;

  // biome-ignore lint/correctness/useExhaustiveDependencies: serveNonce re-runs preview_serve
  useEffect(() => {
    let cancelled = false;
    let token: string | null = null;
    setServed(null);
    setError(null);
    invoke<Served>("preview_serve", {
      path,
      rootHint: rootHint ?? null,
      workspace: currentWorkspaceEnv(),
    })
      .then((result) => {
        if (cancelled) {
          void invoke("preview_release", { token: result.token }).catch(
            () => {},
          );
          return;
        }
        token = result.token;
        setServed(result);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
      if (token) void invoke("preview_release", { token }).catch(() => {});
    };
  }, [path, rootHint, serveNonce]);

  const reload = useCallback(() => {
    setLoaded(true);
    setNonce((n) => n + 1);
  }, []);

  const discardPendingReload = useHtmlPreviewReload(
    served?.root ?? null,
    visible,
    reload,
  );

  useEffect(() => {
    if (visible) {
      if (!loadedRef.current) discardPendingReload();
      setLoaded(true);
      return;
    }
    const t = setTimeout(() => setLoaded(false), SUSPEND_AFTER_MS);
    return () => clearTimeout(t);
  }, [visible, discardPendingReload]);

  const onReloadClick = () => {
    if (reloadAction(error) === "reserve") setServeNonce((n) => n + 1);
    else reload();
  };

  // The backend builds this URL from the canonical entry path, so a
  // non-canonical `path` prop (symlink, /var vs /private/var) still resolves.
  const src = served?.url ?? null;

  const openExternal = () => {
    if (!served) return;
    void invoke("preview_open_external", { token: served.token }).catch(
      console.error,
    );
  };

  return (
    <div
      className="flex h-full w-full flex-col overflow-hidden bg-background"
      style={{
        visibility: visible ? "visible" : "hidden",
        pointerEvents: visible ? "auto" : "none",
      }}
    >
      <div className="flex h-8 shrink-0 items-center gap-2 border-b border-border/60 px-3 text-[11px]">
        <span className="min-w-0 flex-1 truncate text-muted-foreground">
          {path}
        </span>
        <button
          type="button"
          onClick={onReloadClick}
          className="rounded px-1.5 py-0.5 hover:bg-accent/50"
        >
          Reload
        </button>
        <button
          type="button"
          onClick={openExternal}
          disabled={!served}
          className="flex items-center gap-1 rounded px-1.5 py-0.5 hover:bg-accent/50 disabled:opacity-50"
        >
          <HugeiconsIcon icon={LinkSquare02Icon} size={10} strokeWidth={1.75} />
          Open in browser
        </button>
      </div>
      <div className="relative min-h-0 flex-1 bg-white">
        {error ? (
          <Message text={error} />
        ) : src && loaded ? (
          <iframe
            key={`${src}#${nonce}`}
            src={src}
            title="HTML preview"
            className="h-full w-full border-0"
            sandbox="allow-scripts allow-forms allow-modals allow-popups allow-popups-to-escape-sandbox"
            referrerPolicy="no-referrer"
          />
        ) : src ? (
          <Message text="Preview suspended" />
        ) : null}
      </div>
    </div>
  );
}

function Message({ text }: { text: string }) {
  return (
    <div className="flex h-full w-full flex-col items-center justify-center gap-2 bg-background px-6 text-center text-[11px] text-muted-foreground">
      <HugeiconsIcon icon={Globe02Icon} size={18} strokeWidth={1.5} />
      <p>{text}</p>
    </div>
  );
}
