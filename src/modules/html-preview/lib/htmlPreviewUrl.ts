const SCHEME = "terax-preview";
const WINDOWS_HOST = "terax-preview.localhost";

export function htmlPreviewUrl(
  token: string,
  relPath: string,
  platform: string,
): string {
  const encoded = relPath
    .replace(/\\/g, "/")
    .split("/")
    .filter((segment) => segment !== "")
    .map(encodeURIComponent)
    .join("/");
  return platform === "windows"
    ? `http://${WINDOWS_HOST}/${token}/${encoded}`
    : `${SCHEME}://${token}/${encoded}`;
}

export function relativeToRoot(path: string, root: string): string | null {
  const p = path.replace(/\\/g, "/");
  const r = root.replace(/\\/g, "/").replace(/\/+$/, "");
  if (!p.startsWith(`${r}/`)) return null;
  return p.slice(r.length + 1);
}
