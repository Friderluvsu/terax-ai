const SCRATCH_ID_RE = /^[a-f0-9]{32}$/;
export const SCRATCH_DEBOUNCE_MS = 500;
/** Orphans younger than this survive startup cleanup, in case of races. */
export const SCRATCH_ORPHAN_MIN_AGE_MS = 7 * 24 * 60 * 60 * 1000;
/** Detection is a single JSON.parse; anything larger stays plain text. */
export const JSON_DETECT_MAX_CHARS = 1024 * 1024;

export type ScratchEntry = { id: string; mtime: number };

export function isBlankText(text: string): boolean {
  return !/\S/.test(text);
}

export function looksLikeJsonDocument(text: string): boolean {
  if (text.length > JSON_DETECT_MAX_CHARS) return false;
  const trimmed = text.trim();
  if (trimmed.length < 2) return false;
  const first = trimmed[0];
  const last = trimmed[trimmed.length - 1];
  if (!((first === "{" && last === "}") || (first === "[" && last === "]")))
    return false;
  try {
    const value: unknown = JSON.parse(trimmed);
    return typeof value === "object" && value !== null;
  } catch {
    return false;
  }
}

/** Language ids are primary extensions; anything else saves as text. */
export function extensionForLanguage(language: string | null | undefined) {
  return language && /^[a-z0-9][a-z0-9+_-]{0,15}$/i.test(language)
    ? language.toLowerCase()
    : "txt";
}

export function suggestedFileName(
  title: string,
  language: string | null | undefined,
): string {
  return `${title}.${extensionForLanguage(language)}`;
}

export type SaveAsTarget = { path: string } | { error: string };

/** Resolves the typed Save As path against the space root (WSL / SSH). */
export function resolveSaveAsInput(
  root: string | null,
  input: string,
): SaveAsTarget {
  const trimmed = input.trim().replace(/\\/g, "/");
  if (!trimmed) return { error: "Name is required" };
  const segments = trimmed.split("/");
  if (segments.some((s) => s === ".."))
    return { error: "Path must not contain .." };
  if (trimmed.endsWith("/")) return { error: "Path must name a file" };
  if (trimmed.startsWith("/")) return { path: trimmed };
  if (!root) return { error: "No workspace root" };
  const base = root.replace(/\/+$/, "");
  const rel = segments.filter((s) => s !== "" && s !== ".").join("/");
  return { path: `${base}/${rel}` };
}

export function orphanScratchIds(
  entries: readonly ScratchEntry[],
  referenced: ReadonlySet<string>,
  now: number,
  minAgeMs = SCRATCH_ORPHAN_MIN_AGE_MS,
): string[] {
  return entries
    .filter(
      (e) =>
        SCRATCH_ID_RE.test(e.id) &&
        !referenced.has(e.id) &&
        now - e.mtime >= minAgeMs,
    )
    .map((e) => e.id);
}

export type UntitledCloseAction = "discard" | "prompt";

/** Live text wins over the tab flag; disk is read only when neither is known. */
export async function untitledCloseAction(
  pendingText: string | undefined,
  knownEmpty: boolean | undefined,
  readText: () => Promise<string>,
): Promise<UntitledCloseAction> {
  let blank: boolean;
  if (pendingText !== undefined) blank = isBlankText(pendingText);
  else if (knownEmpty !== undefined) blank = knownEmpty;
  else blank = isBlankText(await readText());
  return blank ? "discard" : "prompt";
}
