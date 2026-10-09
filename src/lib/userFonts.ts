import { invoke } from "@tauri-apps/api/core";

type FaceInfo = { id: number; weight: number; italic: boolean };
type Resolved = { family: string; faces: FaceInfo[] };

export type UserFontInfo = { family: string | null; nerd: string | null };

const MAX_FACES = 4;
const NONE: UserFontInfo = { family: null, nerd: null };

const infos = new Map<string, UserFontInfo>();
const pending = new Map<string, Promise<void>>();
const loadedFaces = new Set<number>();
const listeners = new Set<() => void>();
const versions = new Map<string, number>();

const familyOf = (pref: string) => pref.trim().toLowerCase();
const keyOf = (pref: string, weight: string) => `${familyOf(pref)}|${weight}`;

export function getUserFontsVersion(pref: string): number {
  return versions.get(familyOf(pref)) ?? 0;
}

export function subscribeUserFonts(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function getUserFontInfo(pref: string): UserFontInfo {
  const prefix = `${pref.trim().toLowerCase()}|`;
  for (const [k, v] of infos) if (k.startsWith(prefix)) return v;
  return NONE;
}

export function pickFaces(faces: FaceInfo[], weights: number[]): FaceInfo[] {
  const picked = new Map<number, FaceInfo>();
  for (const italic of [false, true]) {
    const pool = faces.filter((f) => f.italic === italic);
    if (!pool.length) continue;
    for (const w of weights) {
      const best = pool.reduce((a, b) =>
        Math.abs(b.weight - w) < Math.abs(a.weight - w) ? b : a,
      );
      picked.set(best.id, best);
    }
  }
  return [...picked.values()].slice(0, MAX_FACES);
}

async function loadFamily(
  family: string,
  faces: FaceInfo[],
  weights: number[],
  loaded: { count: number },
): Promise<boolean> {
  let ok = false;
  for (const face of pickFaces(faces, weights)) {
    if (loadedFaces.has(face.id)) {
      ok = true;
      continue;
    }
    try {
      const bytes = await invoke<ArrayBuffer>("font_read", { id: face.id });
      const font = new FontFace(family, bytes, {
        weight: String(face.weight),
        style: face.italic ? "italic" : "normal",
      });
      await font.load();
      document.fonts.add(font);
      loadedFaces.add(face.id);
      loaded.count += 1;
      ok = true;
    } catch {
      /* keep the current stack */
    }
  }
  return ok;
}

async function run(
  pref: string,
  weight: string,
  loaded: { count: number },
): Promise<void> {
  const name = pref.trim().replace(/['"]/g, "");
  const wanted = [400, 700];
  const w = Number.parseInt(weight, 10);
  if (Number.isFinite(w) && !wanted.includes(w)) wanted.push(w);
  const info: UserFontInfo = { family: null, nerd: null };

  let nerd: string | null = null;
  if (name && !name.includes(",")) {
    const r = await invoke<Resolved | null>("fonts_resolve", { family: name });
    if (r && (await loadFamily(r.family, r.faces, wanted, loaded))) {
      info.family = r.family;
      if (/nerd font/i.test(r.family)) nerd = r.family;
    }
  }
  if (!nerd) {
    const fallback = await invoke<string | null>("fonts_nerd_fallback");
    if (fallback) {
      const r = await invoke<Resolved | null>("fonts_resolve", {
        family: fallback,
      });
      if (r && (await loadFamily(r.family, r.faces, [400, 700], loaded))) {
        nerd = r.family;
        if (!name) info.family = r.family;
      }
    }
  }
  info.nerd = nerd;
  infos.set(keyOf(pref, weight), info);
}

export function ensureUserFonts(pref: string, weight = "400"): Promise<void> {
  const key = keyOf(pref, weight);
  const hit = pending.get(key);
  if (hit) return hit;
  const before = getUserFontInfo(pref);
  const loaded = { count: 0 };
  const p = run(pref, weight, loaded)
    .catch(() => {
      infos.set(key, NONE);
    })
    .then(() => {
      const after = getUserFontInfo(pref);
      if (
        loaded.count === 0 &&
        after.family === before.family &&
        after.nerd === before.nerd
      )
        return;
      const family = familyOf(pref);
      versions.set(family, (versions.get(family) ?? 0) + 1);
      for (const fn of listeners) fn();
    });
  pending.set(key, p);
  return p;
}
