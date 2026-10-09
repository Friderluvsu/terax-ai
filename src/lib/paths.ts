function normalizePath(path: string): string {
  return path.replace(/\\/g, "/");
}

export function pathAtOrUnder(path: string, root: string): boolean {
  const normalizedPath = normalizePath(path);
  const normalizedRoot = normalizePath(root);
  return (
    normalizedPath === normalizedRoot ||
    normalizedPath.startsWith(`${normalizedRoot}/`)
  );
}
