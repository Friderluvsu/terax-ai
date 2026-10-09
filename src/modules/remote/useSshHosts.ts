import { useEffect, useState } from "react";
import { listSshHosts, type SshHost } from "./hosts";

export function useSshHosts(enabled: boolean): SshHost[] {
  const [hosts, setHosts] = useState<SshHost[]>([]);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    listSshHosts()
      .then((list) => {
        if (!cancelled) setHosts(list);
      })
      .catch(() => {
        if (!cancelled) setHosts([]);
      });
    return () => {
      cancelled = true;
    };
  }, [enabled]);

  return hosts;
}
