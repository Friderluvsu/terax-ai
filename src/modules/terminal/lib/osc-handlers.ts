import { IS_WINDOWS } from "@/lib/platform";

const MAX_OSC52_CLIPBOARD_BYTES = 1024 * 1024;

export type Osc7 = { host: string; path: string };

export function parseOsc7(data: string): Osc7 | null {
  const m = data.match(/^file:\/\/([^/]*)(\/.*)$/);
  if (!m) return null;
  let host = m[1];
  try {
    host = decodeURIComponent(host);
  } catch {}
  let path = m[2];
  try {
    path = decodeURIComponent(path);
  } catch {}
  // /C:/Users/foo -> C:/Users/foo so it's a valid Windows path.
  if (/^\/[A-Za-z]:/.test(path)) {
    path = path.slice(1);
  } else if (IS_WINDOWS) {
    // git-bash (MSYS) reports cwd as /c/Users/foo; map it to C:/Users/foo.
    const drive = path.match(/^\/([A-Za-z])(\/.*)?$/);
    if (drive) path = `${drive[1].toUpperCase()}:${drive[2] ?? "/"}`;
  }
  return { host, path };
}

function normalizeHost(host: string): string {
  return host.toLowerCase().replace(/\.local$/, "");
}

export function isLocalOsc7Host(
  host: string,
  localHostname: string | null,
): boolean {
  if (host === "") return true;
  const h = normalizeHost(host);
  if (h === "localhost") return true;
  if (!localHostname) return true;
  const own = normalizeHost(localHostname);
  return h === own || h === own.split(".")[0];
}

export function createOsc7HostFilter(opts: {
  ssh: boolean;
  getLocalHostname: () => string | null;
}): (host: string) => boolean {
  let pinned: string | null = null;
  return (host) => {
    if (!opts.ssh) return isLocalOsc7Host(host, opts.getLocalHostname());
    if (host === "") return false;
    const h = normalizeHost(host);
    if (pinned === null) pinned = h;
    return pinned === h;
  };
}

export function parseOsc52Clipboard(data: string): string | null {
  const parts = data.split(";");
  if (parts.length < 2) return null;
  const selection = parts[0] || "c";
  if (!selection.includes("c")) return null;
  const encoded = parts.slice(1).join(";");
  if (!encoded || encoded === "?") return null;
  if (encoded.length > Math.ceil((MAX_OSC52_CLIPBOARD_BYTES * 4) / 3) + 4) {
    return null;
  }
  const compact = encoded.replace(/\s/g, "");
  if (!/^[A-Za-z0-9+/]*={0,2}$/.test(compact)) return null;

  try {
    const bytes = Uint8Array.from(atob(compact), (c) => c.charCodeAt(0));
    if (bytes.byteLength > MAX_OSC52_CLIPBOARD_BYTES) return null;
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return null;
  }
}
