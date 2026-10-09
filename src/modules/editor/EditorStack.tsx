import { cn, isHtmlPath, isMarkdownPath, isSvgPath } from "@/lib/utils";
import { DocumentViewToggle, MarkdownViewToggle } from "@/modules/markdown";
import type { DocView, EditorTab, Tab } from "@/modules/tabs";
import { envCaps, useWorkspaceEnvStore } from "@/modules/workspace";
import { useEffect, useRef } from "react";
import { EditorPane, type EditorPaneHandle } from "./EditorPane";
import { HtmlSplitView } from "./HtmlSplitView";
import { RunScriptButton } from "./RunScriptButton";

type Props = {
  tabs: Tab[];
  activeId: number;
  onDirtyChange: (id: number, dirty: boolean) => void;
  registerHandle: (id: number, handle: EditorPaneHandle | null) => void;
  onCloseTab: (id: number) => void;
  onSetMarkdownView: (id: number, mode: "rendered" | "raw") => void;
  onRunScript: (path: string) => void;
  onSetDocView: (id: number, view: DocView) => void;
  rootHint: string | null;
};

export function EditorStack({
  tabs,
  activeId,
  onDirtyChange,
  registerHandle,
  onCloseTab,
  onSetMarkdownView,
  onRunScript,
  onSetDocView,
  rootHint,
}: Props) {
  const isLocal = useWorkspaceEnvStore((s) => s.env.kind === "local");
  const htmlPreview = useWorkspaceEnvStore((s) => envCaps(s.env).htmlPreview);
  const editors = tabs.filter(
    (t): t is EditorTab => t.kind === "editor" && !t.cold,
  );

  // Stable per-tab callbacks. Inline arrows in `ref` and `onDirtyChange`
  // change identity every render, which makes React detach+reattach the ref
  // callback and re-invoke `onDirtyChange`, triggering setState loops in
  // the parent. Memoizing per id keeps each callback's identity stable.
  const registerRef = useRef(registerHandle);
  const dirtyRef = useRef(onDirtyChange);
  const closeRef = useRef(onCloseTab);

  useEffect(() => {
    registerRef.current = registerHandle;
  }, [registerHandle]);
  useEffect(() => {
    dirtyRef.current = onDirtyChange;
  }, [onDirtyChange]);
  useEffect(() => {
    closeRef.current = onCloseTab;
  }, [onCloseTab]);

  const refCallbacks = useRef(
    new Map<number, (h: EditorPaneHandle | null) => void>(),
  );
  const dirtyCallbacks = useRef(new Map<number, (dirty: boolean) => void>());
  const closeCallbacks = useRef(new Map<number, () => void>());

  const getRefCallback = (id: number) => {
    let cb = refCallbacks.current.get(id);
    if (!cb) {
      cb = (h: EditorPaneHandle | null) => registerRef.current(id, h);
      refCallbacks.current.set(id, cb);
    }
    return cb;
  };
  const getDirtyCallback = (id: number) => {
    let cb = dirtyCallbacks.current.get(id);
    if (!cb) {
      cb = (dirty: boolean) => dirtyRef.current(id, dirty);
      dirtyCallbacks.current.set(id, cb);
    }
    return cb;
  };
  const getCloseCallback = (id: number) => {
    let cb = closeCallbacks.current.get(id);
    if (!cb) {
      cb = () => closeRef.current(id);
      closeCallbacks.current.set(id, cb);
    }
    return cb;
  };

  // Drop callback entries for closed tabs to avoid unbounded growth.
  useEffect(() => {
    const live = new Set(editors.map((t) => t.id));
    for (const id of refCallbacks.current.keys()) {
      if (!live.has(id)) refCallbacks.current.delete(id);
    }
    for (const id of dirtyCallbacks.current.keys()) {
      if (!live.has(id)) dirtyCallbacks.current.delete(id);
    }
    for (const id of closeCallbacks.current.keys()) {
      if (!live.has(id)) closeCallbacks.current.delete(id);
    }
  }, [editors]);

  if (editors.length === 0) return null;
  return (
    <div className="relative h-full w-full">
      {editors.map((t) => {
        const visible = t.id === activeId;
        const htmlTab =
          htmlPreview && (isHtmlPath(t.path) || isSvgPath(t.path));
        const pane = (
          <EditorPane
            ref={getRefCallback(t.id)}
            path={t.path}
            overrideLanguage={t.overrideLanguage}
            onDirtyChange={getDirtyCallback(t.id)}
            onClose={getCloseCallback(t.id)}
          />
        );
        return (
          <div
            key={t.id}
            className={cn(
              "absolute inset-0",
              !visible && "invisible pointer-events-none",
            )}
            aria-hidden={!visible}
          >
            <div className="relative h-full overflow-hidden bg-background">
              {isMarkdownPath(t.path) && (
                <MarkdownViewToggle
                  mode="raw"
                  onChange={(mode) => onSetMarkdownView(t.id, mode)}
                  renderedDisabled={t.dirty}
                  renderedHint="Save to preview"
                />
              )}
              {isLocal && (
                <RunScriptButton
                  path={t.path}
                  dirty={t.dirty}
                  onRun={onRunScript}
                />
              )}
              {htmlTab && (
                <DocumentViewToggle
                  mode={t.docView ?? "code"}
                  onChange={(view) => onSetDocView(t.id, view)}
                  options={[
                    { value: "code", label: "Code" },
                    { value: "split", label: "Split" },
                    { value: "preview", label: "Preview" },
                  ]}
                />
              )}
              {htmlTab ? (
                <HtmlSplitView
                  view={t.docView}
                  visible={visible}
                  path={t.path}
                  rootHint={rootHint}
                  dirty={t.dirty}
                >
                  {pane}
                </HtmlSplitView>
              ) : (
                pane
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
