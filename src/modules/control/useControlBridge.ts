import {
  parseTaskClose,
  parseTaskOpen,
  parseTaskScreen,
  parseTaskSend,
  type TaskOpened,
  type TaskOpenRequest,
  TaskRequestError,
} from "@/modules/orchestra";
import { DEFAULT_SPACE_ID, type Tab } from "@/modules/tabs/lib/useTabs";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type RefObject, useEffect } from "react";
import { resolveControlContext } from "./lib/context";
import { createReadinessQueue } from "./lib/readiness";

type ControlError = {
  code: string;
  message: string;
};

type ControlRequest = {
  id: string;
  method: string;
  params: unknown;
  caller: {
    pane_id?: number;
  };
};

type FrontendResponse = {
  ok: boolean;
  result?: unknown;
  error?: ControlError;
};

type OpenRequest = {
  path: string;
  line?: number;
  focus: boolean;
};

type UseControlBridgeOptions = {
  ready: boolean;
  tabsRef: RefObject<Tab[]>;
  activeTabIdRef: RefObject<number>;
  activeSpaceIdRef: RefObject<string | null>;
  onOpen: (request: OpenRequest & { spaceId: string }) => number | null;
  onTaskOpen: (request: TaskOpenRequest) => TaskOpened;
  onTaskClose: (paneId: number) => boolean;
  onTaskSend: (paneId: number, text: string) => boolean;
  onTaskScreen: (paneId: number, lines: number) => string | null;
};

class RequestError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
  }
}

export function parseOpenRequest(params: unknown): OpenRequest {
  if (typeof params !== "object" || params === null) {
    throw new RequestError("invalid_params", "open parameters are required");
  }
  const value = params as Record<string, unknown>;
  if (typeof value.path !== "string" || value.path.length === 0) {
    throw new RequestError("invalid_params", "open path is required");
  }
  if (
    value.line !== undefined &&
    (!Number.isSafeInteger(value.line) || (value.line as number) < 1)
  ) {
    throw new RequestError("invalid_params", "line must be a positive integer");
  }
  if (value.column !== undefined) {
    throw new RequestError(
      "unsupported_parameter",
      "column targeting is not supported yet",
    );
  }
  if (value.focus !== undefined && typeof value.focus !== "boolean") {
    throw new RequestError("invalid_params", "focus must be a boolean");
  }
  return {
    path: value.path,
    line: value.line as number | undefined,
    focus: (value.focus as boolean | undefined) ?? true,
  };
}

const setFrontendReady = createReadinessQueue((ready) =>
  invoke("control_frontend_ready", { ready }),
);

async function respond(
  requestId: string,
  response: FrontendResponse,
): Promise<void> {
  const delivered = await invoke<boolean>("control_respond", {
    requestId,
    response,
  });
  if (!delivered) {
    console.warn(`[terax] control response expired: ${requestId}`);
  }
}

export function useControlBridge({
  ready,
  tabsRef,
  activeTabIdRef,
  activeSpaceIdRef,
  onOpen,
  onTaskOpen,
  onTaskClose,
  onTaskSend,
  onTaskScreen,
}: UseControlBridgeOptions): void {
  useEffect(() => {
    if (!ready) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const handleRequest = async (request: ControlRequest) => {
      try {
        const context = resolveControlContext(
          tabsRef.current ?? [],
          activeTabIdRef.current ?? 0,
          activeSpaceIdRef.current ?? DEFAULT_SPACE_ID,
          request.caller.pane_id,
        );
        if (request.method === "identify") {
          await respond(request.id, { ok: true, result: context });
          return;
        }
        if (request.method === "open") {
          const open = parseOpenRequest(request.params);
          let focused = false;
          if (open.focus) {
            const window = getCurrentWindow();
            try {
              await window.show();
              await window.setFocus();
              focused = true;
            } catch (error) {
              console.warn("[terax] could not focus control target:", error);
            }
          }
          const tabId = onOpen({ ...open, spaceId: context.space_id });
          if (tabId === null) {
            throw new RequestError(
              "open_failed",
              "Terax could not create an editor tab",
            );
          }
          await new Promise<void>((resolve) =>
            requestAnimationFrame(() => resolve()),
          );
          await respond(request.id, {
            ok: true,
            result: {
              path: open.path,
              line: open.line ?? null,
              tab_id: tabId,
              space_id: context.space_id,
              focus_requested: open.focus,
              focused,
            },
          });
          return;
        }
        if (request.method === "task.open") {
          const result = onTaskOpen(parseTaskOpen(request.params));
          await respond(request.id, { ok: true, result });
          return;
        }
        if (request.method === "task.close") {
          const closed = onTaskClose(parseTaskClose(request.params));
          await respond(request.id, { ok: true, result: { closed } });
          return;
        }
        if (request.method === "task.send") {
          const send = parseTaskSend(request.params);
          if (!onTaskSend(send.paneId, send.text)) {
            throw new RequestError("pane_gone", "the worker pane is closed");
          }
          await respond(request.id, { ok: true, result: { sent: true } });
          return;
        }
        if (request.method === "task.screen") {
          const screen = parseTaskScreen(request.params);
          const text = onTaskScreen(screen.paneId, screen.lines);
          if (text === null) {
            throw new RequestError("pane_gone", "the worker pane is closed");
          }
          await respond(request.id, { ok: true, result: { text } });
          return;
        }
        throw new RequestError(
          "unknown_method",
          `unsupported frontend method '${request.method}'`,
        );
      } catch (error) {
        const responseError =
          error instanceof RequestError || error instanceof TaskRequestError
            ? { code: error.code, message: error.message }
            : { code: "frontend_error", message: String(error) };
        await respond(request.id, { ok: false, error: responseError }).catch(
          (responseError) => {
            console.error("[terax] control response failed:", responseError);
          },
        );
      }
    };

    void listen<ControlRequest>("terax:control-request", (event) => {
      void handleRequest(event.payload);
    })
      .then((stop) => {
        if (disposed) {
          stop();
          return;
        }
        unlisten = stop;
        return setFrontendReady(true);
      })
      .catch((error) => {
        console.error("[terax] control bridge setup failed:", error);
      });

    return () => {
      disposed = true;
      unlisten?.();
      void setFrontendReady(false).catch((error) => {
        console.error("[terax] control bridge cleanup failed:", error);
      });
    };
  }, [
    ready,
    tabsRef,
    activeTabIdRef,
    activeSpaceIdRef,
    onOpen,
    onTaskOpen,
    onTaskClose,
    onTaskSend,
    onTaskScreen,
  ]);
}
