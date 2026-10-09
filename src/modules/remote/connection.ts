import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export type SshConnState = "connecting" | "connected" | "lost";
export type SshConnEvent = "attempt" | "ok" | "fail" | "disconnect";

export function reduceConn(
  prev: SshConnState | undefined,
  event: SshConnEvent,
): SshConnState {
  switch (event) {
    case "attempt":
      return "connecting";
    case "ok":
      return "connected";
    case "fail":
      return prev === "connected" || prev === "lost" ? "lost" : "connecting";
    case "disconnect":
      return "lost";
  }
}

export function backoffDelay(attempt: number): number {
  return Math.min(1000 * 2 ** attempt, 8000);
}

type GateDeps = {
  check: (host: string) => Promise<boolean>;
  sleep: (ms: number) => Promise<void>;
  emit?: (host: string, event: SshConnEvent) => void;
};

export type SshLease = { release: () => void };

type HostGate = { leader: object | null; waiters: Array<() => void> };

/**
 * The first ssh terminal for a host starts immediately and owns the login
 * (password, 2FA). Every other terminal waits until ssh_check succeeds, so
 * only one ControlMaster is negotiated.
 */
export function createSshGate(deps: GateDeps) {
  const hosts = new Map<string, HostGate>();

  const safeCheck = async (host: string) => {
    try {
      return await deps.check(host);
    } catch {
      return false;
    }
  };

  const wake = (gate: HostGate) => {
    const waiters = gate.waiters;
    gate.waiters = [];
    for (const w of waiters) w();
  };

  const poll = async (host: string, gate: HostGate, token: object) => {
    for (let attempt = 0; gate.leader === token; attempt++) {
      await deps.sleep(backoffDelay(attempt));
      if (gate.leader !== token) return;
      if (await safeCheck(host)) {
        deps.emit?.(host, "ok");
        if (gate.leader === token) gate.leader = null;
        wake(gate);
        return;
      }
    }
  };

  const enter = async (host: string): Promise<SshLease> => {
    let gate = hosts.get(host);
    if (!gate) {
      gate = { leader: null, waiters: [] };
      hosts.set(host, gate);
    }
    const owned = gate;
    for (;;) {
      if (await safeCheck(host)) {
        deps.emit?.(host, "ok");
        return { release: () => {} };
      }
      if (!owned.leader) {
        const token = {};
        owned.leader = token;
        deps.emit?.(host, "attempt");
        void poll(host, owned, token);
        return {
          release: () => {
            if (owned.leader !== token) return;
            owned.leader = null;
            wake(owned);
          },
        };
      }
      await new Promise<void>((resolve) => owned.waiters.push(resolve));
    }
  };

  return { enter };
}

type ConnStore = {
  status: Record<string, SshConnState>;
  apply: (host: string, event: SshConnEvent) => void;
};

export const useSshConnections = create<ConnStore>((set) => ({
  status: {},
  apply: (host, event) =>
    set((s) => {
      const next = reduceConn(s.status[host], event);
      return s.status[host] === next
        ? s
        : { status: { ...s.status, [host]: next } };
    }),
}));

export function sshCheck(host: string): Promise<boolean> {
  return invoke<boolean>("ssh_check", { host });
}

export const sshGate = createSshGate({
  check: sshCheck,
  sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  emit: (host, event) => useSshConnections.getState().apply(host, event),
});
