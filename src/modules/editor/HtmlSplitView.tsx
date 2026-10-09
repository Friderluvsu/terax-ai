import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import type { DocView } from "@/modules/tabs";
import { lazy, Suspense, useEffect, type ReactNode } from "react";
import { usePanelRef } from "react-resizable-panels";
import { docViewLayout } from "./lib/docViewLayout";

const HtmlPreviewFrame = lazy(() =>
  import("@/modules/html-preview").then((m) => ({
    default: m.HtmlPreviewFrame,
  })),
);

type Props = {
  view: DocView | undefined;
  visible: boolean;
  path: string;
  rootHint: string | null;
  dirty: boolean;
  children: ReactNode;
};

export function HtmlSplitView({
  view,
  visible,
  path,
  rootHint,
  dirty,
  children,
}: Props) {
  const editorPanel = usePanelRef();
  const previewPanel = usePanelRef();
  const layout = docViewLayout(view);

  useEffect(() => {
    editorPanel.current?.resize(`${layout.editor}%`);
    previewPanel.current?.resize(`${layout.preview}%`);
  }, [layout.editor, layout.preview, editorPanel, previewPanel]);

  return (
    <ResizablePanelGroup orientation="horizontal">
      <ResizablePanel
        id="editor"
        panelRef={editorPanel}
        defaultSize={`${layout.editor}%`}
        minSize="0%"
        className={layout.editor === 0 ? "invisible" : undefined}
      >
        {children}
      </ResizablePanel>
      <ResizableHandle
        disabled={view !== "split"}
        className={view === "split" ? undefined : "hidden"}
      />
      <ResizablePanel
        id="preview"
        panelRef={previewPanel}
        defaultSize={`${layout.preview}%`}
        minSize="0%"
      >
        {layout.showPreview && (
          <div className="relative h-full w-full pt-10">
            {view === "split" && dirty && (
              <div className="absolute left-2 top-2 z-10 rounded border border-border/60 bg-card/85 px-2 py-0.5 text-[11px] text-muted-foreground">
                Save to refresh the preview
              </div>
            )}
            <Suspense fallback={null}>
              <HtmlPreviewFrame
                path={path}
                rootHint={rootHint}
                visible={visible}
              />
            </Suspense>
          </div>
        )}
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
