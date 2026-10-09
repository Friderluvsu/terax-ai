const SCHEME = "terax-preview";
const WINDOWS_HOST = "terax-preview.localhost";

export function htmlPreviewUrl(
  token: string,
  relPath: string,
  platform: string,
): string {
  const encoded = (platform === "windows" ? toSlashes(relPath) : relPath)
    .split("/")
    .filter((segment) => segment !== "")
    .map(encodeURIComponent)
    .join("/");
  return platform === "windows"
    ? `http://${WINDOWS_HOST}/${token}/${encoded}`
    : `${SCHEME}://${token}/${encoded}`;
}

function toSlashes(value: string): string {
  return value.replace(/\\/g, "/");
}

export function relativeToRoot(
  path: string,
  root: string,
  platform: string,
): string | null {
  const windows = platform === "windows";
  const p = windows ? toSlashes(path) : path;
  const r = (windows ? toSlashes(root) : root).replace(/\/+$/, "");
  if (!p.startsWith(`${r}/`)) return null;
  return p.slice(r.length + 1);
}
