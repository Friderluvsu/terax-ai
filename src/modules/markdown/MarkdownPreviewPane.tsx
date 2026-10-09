import { MarkdownCode } from "@/components/ai-elements/markdown-code";
import { cn } from "@/lib/utils";
import { parentDir, watchAdd, watchRemove } from "@/modules/explorer/lib/watch";
import { useHtmlPreviewReload } from "@/modules/html-preview/lib/useHtmlPreviewReload";
import { currentWorkspaceEnv, fsFor } from "@/modules/workspace";
import { useCallback, useEffect, useState } from "react";
import { Streamdown } from "streamdown";
import { MarkdownLink } from "./MarkdownLink";
import { MarkdownViewToggle } from "./MarkdownViewToggle";

type Status =
  | { kind: "loading" }
  | { kind: "ready"; content: string }
  | { kind: "binary" }
  | { kind: "toolarge"; size: number; limit: number }
  | { kind: "error"; message: string };

type Props = {
  path: string;
  visible: boolean;
  onSetView: (mode: "rendered" | "raw") => void;
};

const components = { a: MarkdownLink, code: MarkdownCode };

export function MarkdownPreviewPane({ path, visible, onSetView }: Props) {
  const [status, setStatus] = useState<Status>({ kind: "loading" });

  const load = useCallback(
    (initial: boolean) => {
      let cancelled = false;
      if (initial) setStatus({ kind: "loading" });
      fsFor(currentWorkspaceEnv())
        .readFile(path)
        .then((res) => {
          if (cancelled) return;
          if (res.kind === "text") {
            setStatus({ kind: "ready", content: res.content });
          } else if (res.kind === "binary") {
            setStatus({ kind: "binary" });
          } else {
            setStatus({ kind: "toolarge", size: res.size, limit: res.limit });
          }
        })
        .catch((e) => {
          if (!cancelled) setStatus({ kind: "error", message: String(e) });
        });
      return () => {
        cancelled = true;
      };
    },
    [path],
  );

  useEffect(() => load(true), [load]);

  useEffect(() => {
    const dir = parentDir(path);
    watchAdd([dir]);
    return () => watchRemove([dir]);
  }, [path]);

  useHtmlPreviewReload(path, visible, () => {
    load(false);
  });

  return (
    <div
      className={cn(
        "relative flex h-full w-full flex-col overflow-hidden bg-background",
        !visible && "pointer-events-none",
      )}
    >
      <MarkdownViewToggle mode="rendered" onChange={onSetView} />
      <div className="flex-1 overflow-auto">
        <div className="px-8 py-6">
          {status.kind === "loading" && (
            <p className="text-[12px] text-muted-foreground">Loading…</p>
          )}
          {status.kind === "error" && (
            <p className="text-[12px] text-destructive">
              Failed to read file: {status.message}
            </p>
          )}
          {status.kind === "binary" && (
            <p className="text-[12px] text-muted-foreground">
              Binary file — cannot render as markdown.
            </p>
          )}
          {status.kind === "toolarge" && (
            <p className="text-[12px] text-muted-foreground">
              File is {status.size} bytes; limit {status.limit}.
            </p>
          )}
          {status.kind === "ready" && (
            <Streamdown
              className="select-text [&>*:first-child]:mt-0 [&>*:last-child]:mb-0"
              components={components}
              mode="static"
              parseIncompleteMarkdown={false}
            >
              {status.content}
            </Streamdown>
          )}
        </div>
      </div>
    </div>
  );
}
