import { displayAgent, useAgentStore } from "@/modules/agents";
import type { Tab } from "@/modules/tabs/lib/useTabs";
import {
  getLeafBlockMode,
  leafScreen,
  pasteIntoSession,
  submitToLeaf,
  writeToSession,
} from "@/modules/terminal/lib/terminalSessionApi";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type RefObject, useEffect } from "react";
import {
  describeTerminals,
  keySequence,
  MobileRequestError,
  parseScreen,
  parseSend,
  requireLeaf,
  screenHash,
} from "./lib/protocol";

type MobileRequest = { id: number; method: string; params: unknown };

type Options = {
  tabsRef: RefObject<Tab[]>;
  activeTabIdRef: RefObject<number>;
};

function sendText(leaf: number, text: string, enter: boolean): boolean {
  if (enter && getLeafBlockMode(leaf) === "prompt") {
    submitToLeaf(leaf, text);
    return true;
  }
  if (text.includes("\n")) {
    if (!pasteIntoSession(leaf, text)) return false;
    return enter ? writeToSession(leaf, "\r") : true;
  }
  return writeToSession(leaf, enter ? `${text}\r` : text);
}

function handle(request: MobileRequest, options: Options): unknown {
  const terminals = describeTerminals(
    options.tabsRef.current ?? [],
    options.activeTabIdRef.current ?? 0,
    useAgentStore.getState().sessions,
    displayAgent,
  );
  switch (request.method) {
    case "list":
      return { terminals };
    case "screen": {
      const leaf = requireLeaf(request.params, terminals);
      const { lines, since } = parseScreen(request.params);
      const screen = leafScreen(leaf, lines);
      if (!screen) throw new MobileRequestError("terminal is starting");
      const hash = screenHash(screen.text, screen.cols);
      if (hash === since) return { unchanged: true, hash };
      return { text: screen.text, cols: screen.cols, hash };
    }
    case "send": {
      const leaf = requireLeaf(request.params, terminals);
      const { text, enter } = parseSend(request.params);
      if (!sendText(leaf, text, enter)) {
        throw new MobileRequestError("terminal is not accepting input");
      }
      return { ok: true };
    }
    case "key": {
      const leaf = requireLeaf(request.params, terminals);
      if (!writeToSession(leaf, keySequence(request.params))) {
        throw new MobileRequestError("terminal is not accepting input");
      }
      return { ok: true };
    }
    default:
      throw new MobileRequestError("unknown method");
  }
}

export function useMobileBridge({ tabsRef, activeTabIdRef }: Options): void {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<MobileRequest>("terax:mobile-request", ({ payload }) => {
      let reply: { ok: boolean; result?: unknown; error?: string };
      try {
        reply = {
          ok: true,
          result: handle(payload, { tabsRef, activeTabIdRef }),
        };
      } catch (error) {
        reply = {
          ok: false,
          error:
            error instanceof MobileRequestError
              ? error.message
              : "request failed",
        };
        if (!(error instanceof MobileRequestError)) {
          console.error("[terax] phone request failed:", error);
        }
      }
      void invoke("mobile_respond", { requestId: payload.id, reply });
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [tabsRef, activeTabIdRef]);
}
