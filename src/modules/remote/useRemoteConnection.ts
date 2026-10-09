import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect } from "react";
import { sshCheck, useSshConnections, type SshConnState } from "./connection";

const MONITOR_INTERVAL_MS = 10_000;

export function useRemoteConnection(host: string | null): {
  state: SshConnState;
  disconnect: () => Promise<void>;
} {
  const state = useSshConnections((s) => (host ? s.status[host] : undefined));
  const apply = useSshConnections((s) => s.apply);

  useEffect(() => {
    if (!host) return;
    let alive = true;
    const probe = () => {
      sshCheck(host)
        .then((ok) => alive && apply(host, ok ? "ok" : "fail"))
        .catch(() => alive && apply(host, "fail"));
    };
    probe();
    const timer = setInterval(probe, MONITOR_INTERVAL_MS);
    window.addEventListener("focus", probe);
    return () => {
      alive = false;
      clearInterval(timer);
      window.removeEventListener("focus", probe);
    };
  }, [host, apply]);

  const disconnect = useCallback(async () => {
    if (!host) return;
    try {
      await invoke("ssh_disconnect", { host });
    } finally {
      apply(host, "disconnect");
    }
  }, [host, apply]);

  return { state: state ?? "connecting", disconnect };
}
