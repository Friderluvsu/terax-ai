use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const MAX_DEPTH: usize = 3;
const MAX_FILES: usize = 4000;
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_TTC_FACES: u32 = 64;

const NERD_FONT_CANDIDATES: &[&str] = &[
    "JetBrainsMono Nerd Font",
    "JetBrainsMono Nerd Font Mono",
    "JetBrainsMonoNL Nerd Font",
    "FiraCode Nerd Font",
    "FiraCode Nerd Font Mono",
    "MesloLGS NF",
    "MesloLGM Nerd Font",
    "Hack Nerd Font",
    "Hack Nerd Font Mono",
    "CaskaydiaCove Nerd Font",
    "CaskaydiaMono Nerd Font",
    "Iosevka Nerd Font",
    "Iosevka Term Nerd Font",
    "SauceCodePro Nerd Font",
    "Hasklug Nerd Font",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceMeta {
    pub family: String,
    pub weight: u16,
    pub italic: bool,
}

#[derive(Debug, Clone)]
struct IndexedFace {
    path: PathBuf,
    family: String,
    weight: u16,
    italic: bool,
}

#[derive(Debug, Default)]
pub struct FontIndex {
    faces: Vec<IndexedFace>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FaceInfo {
    pub id: u32,
    pub weight: u16,
    pub italic: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Resolved {
    pub family: String,
    pub faces: Vec<FaceInfo>,
}

#[derive(Default)]
pub struct FontsState {
    index: Mutex<Option<Arc<FontIndex>>>,
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    let b = d.get(o..o.checked_add(2)?)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
}

fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    let b = d.get(o..o.checked_add(4)?)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn find_table<'a>(d: &'a [u8], base: usize, tag: &[u8; 4]) -> Option<&'a [u8]> {
    let magic = d.get(base..base.checked_add(4)?)?;
    if magic != [0, 1, 0, 0] && magic != b"OTTO" && magic != b"true" {
        return None;
    }
    let n = u16_at(d, base + 4)? as usize;
    for i in 0..n {
        let rec = base.checked_add(12)?.checked_add(i.checked_mul(16)?)?;
        if d.get(rec..rec.checked_add(4)?)? == tag {
            let off = u32_at(d, rec + 8)? as usize;
            let len = u32_at(d, rec + 12)? as usize;
            return d.get(off..off.checked_add(len)?);
        }
    }
    None
}

fn decode_name(raw: &[u8], platform: u16) -> Option<String> {
    let s = if platform == 3 || platform == 0 {
        if !raw.len().is_multiple_of(2) {
            return None;
        }
        let units: Vec<u16> = raw
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&units).ok()?
    } else {
        raw.iter().map(|&b| b as char).collect()
    };
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn name_id(table: &[u8], wanted: u16) -> Option<String> {
    let count = u16_at(table, 2)? as usize;
    let storage = u16_at(table, 4)? as usize;
    let mut best: Option<(u8, String)> = None;
    for i in 0..count {
        let rec = 6usize.checked_add(i.checked_mul(12)?)?;
        if u16_at(table, rec + 6)? != wanted {
            continue;
        }
        let platform = u16_at(table, rec)?;
        let rank = match platform {
            3 => 3,
            0 => 2,
            1 => 1,
            _ => continue,
        };
        if best.as_ref().is_some_and(|(r, _)| *r >= rank) {
            continue;
        }
        let len = u16_at(table, rec + 8)? as usize;
        let off = storage.checked_add(u16_at(table, rec + 10)? as usize)?;
        let Some(raw) = table.get(off..off.checked_add(len)?) else {
            continue;
        };
        if let Some(s) = decode_name(raw, platform) {
            best = Some((rank, s));
        }
    }
    best.map(|(_, s)| s)
}

fn parse_face(d: &[u8], base: usize) -> Option<FaceMeta> {
    let name = find_table(d, base, b"name")?;
    let family = name_id(name, 16).or_else(|| name_id(name, 1))?;
    let style = name_id(name, 17)
        .or_else(|| name_id(name, 2))
        .unwrap_or_default();
    let style_lc = style.to_lowercase();
    let mut weight = if style_lc.contains("bold") { 700 } else { 400 };
    let mut italic = style_lc.contains("italic") || style_lc.contains("oblique");
    if let Some(os2) = find_table(d, base, b"OS/2") {
        if let Some(w) = u16_at(os2, 4).filter(|w| (1..=1000).contains(w)) {
            weight = w;
        }
        if let Some(sel) = u16_at(os2, 62) {
            italic = sel & 1 != 0;
        }
    }
    Some(FaceMeta {
        family,
        weight,
        italic,
    })
}

/// Parses every face of a ttf, otf or ttc. Never panics on malformed input.
pub fn parse_faces(d: &[u8]) -> Vec<FaceMeta> {
    if d.get(0..4) == Some(b"ttcf") {
        let Some(n) = u32_at(d, 8) else {
            return Vec::new();
        };
        return (0..n.min(MAX_TTC_FACES) as usize)
            .filter_map(|i| u32_at(d, 12 + i * 4))
            .filter_map(|off| parse_face(d, off as usize))
            .collect();
    }
    parse_face(d, 0).into_iter().collect()
}

fn collect_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        if out.len() >= MAX_FILES {
            return;
        }
        let Ok(meta) = fs::symlink_metadata(e.path()) else {
            continue;
        };
        if meta.is_dir() {
            collect_files(&e.path(), depth + 1, out);
        } else if meta.is_file() && meta.len() <= MAX_FILE_BYTES {
            let ext = e
                .path()
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.to_ascii_lowercase());
            if matches!(ext.as_deref(), Some("ttf" | "otf" | "ttc")) {
                out.push(e.path());
            }
        }
    }
}

pub fn scan_dirs(dirs: &[PathBuf]) -> FontIndex {
    let mut files = Vec::new();
    for d in dirs {
        collect_files(d, 0, &mut files);
    }
    let mut faces = Vec::new();
    for path in files {
        let Ok(bytes) = fs::read(&path) else { continue };
        for m in parse_faces(&bytes) {
            faces.push(IndexedFace {
                path: path.clone(),
                family: m.family,
                weight: m.weight,
                italic: m.italic,
            });
        }
    }
    FontIndex { faces }
}

fn compact(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

impl FontIndex {
    fn families(&self) -> BTreeMap<&str, ()> {
        self.faces.iter().map(|f| (f.family.as_str(), ())).collect()
    }

    pub fn resolve(&self, query: &str) -> Option<Resolved> {
        let q = query.trim();
        if q.is_empty() {
            return None;
        }
        let families = self.families();
        let family = families
            .keys()
            .find(|f| f.eq_ignore_ascii_case(q))
            .or_else(|| {
                let c = compact(q);
                families.keys().find(|f| compact(f) == c)
            })?
            .to_string();
        let faces = self
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.family == family)
            .map(|(i, f)| FaceInfo {
                id: i as u32,
                weight: f.weight,
                italic: f.italic,
            })
            .collect();
        Some(Resolved { family, faces })
    }

    pub fn nerd_fallback(&self) -> Option<String> {
        let families = self.families();
        let has = |name: &str| families.keys().find(|f| f.eq_ignore_ascii_case(name));
        for needle in ["nerd font mono", "nerd font"] {
            if let Some(f) = NERD_FONT_CANDIDATES
                .iter()
                .filter(|c| c.to_lowercase().contains(needle))
                .find_map(|c| has(c))
            {
                return Some(f.to_string());
            }
        }
        families
            .keys()
            .map(|f| f.to_string())
            .filter(|f| {
                let l = f.to_lowercase();
                l.contains("nerd font") && !l.contains("propo")
            })
            .min_by_key(|f| (!f.to_lowercase().contains("nerd font mono"), f.clone()))
    }

    fn path_for(&self, id: u32) -> Option<&Path> {
        self.faces.get(id as usize).map(|f| f.path.as_path())
    }
}

pub fn system_font_dirs() -> Vec<PathBuf> {
    let home = dirs::home_dir();
    let mut v = Vec::new();
    if cfg!(target_os = "macos") {
        if let Some(h) = &home {
            v.push(h.join("Library/Fonts"));
        }
        v.push(PathBuf::from("/Library/Fonts"));
    } else if cfg!(target_os = "linux") {
        if let Some(h) = &home {
            v.push(h.join(".local/share/fonts"));
            v.push(h.join(".fonts"));
        }
        v.push(PathBuf::from("/usr/share/fonts"));
        v.push(PathBuf::from("/usr/local/share/fonts"));
    }
    v
}

fn index(state: &FontsState) -> Arc<FontIndex> {
    let mut guard = state.index.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .get_or_insert_with(|| Arc::new(scan_dirs(&system_font_dirs())))
        .clone()
}

#[tauri::command]
pub async fn fonts_resolve(
    state: tauri::State<'_, FontsState>,
    family: String,
) -> Result<Option<Resolved>, String> {
    let idx = index(&state);
    Ok(idx.resolve(&family))
}

#[tauri::command]
pub async fn fonts_nerd_fallback(
    state: tauri::State<'_, FontsState>,
) -> Result<Option<String>, String> {
    Ok(index(&state).nerd_fallback())
}

#[tauri::command]
pub async fn font_read(
    state: tauri::State<'_, FontsState>,
    id: u32,
) -> Result<tauri::ipc::Response, String> {
    let idx = index(&state);
    let path = idx.path_for(id).ok_or("unknown font id")?;
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_FILE_BYTES {
        return Err("font file too large".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_table(records: &[(u16, &str)]) -> Vec<u8> {
        let mut strings = Vec::new();
        let mut recs = Vec::new();
        for (id, s) in records {
            let enc: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
            recs.extend([3u16, 1, 0x409, *id, enc.len() as u16, strings.len() as u16]);
            strings.extend(enc);
        }
        let mut t = vec![0, 0];
        t.extend((records.len() as u16).to_be_bytes());
        t.extend((6 + 12 * records.len() as u16).to_be_bytes());
        for v in recs {
            t.extend(v.to_be_bytes());
        }
        t.extend(strings);
        t
    }

    fn os2(weight: u16, italic: bool) -> Vec<u8> {
        let mut t = vec![0u8; 64];
        t[4..6].copy_from_slice(&weight.to_be_bytes());
        t[62..64].copy_from_slice(&(italic as u16).to_be_bytes());
        t
    }

    fn sfnt(tables: &[(&[u8; 4], Vec<u8>)], base_pad: usize) -> Vec<u8> {
        let mut out = vec![0u8; base_pad];
        out.extend([0, 1, 0, 0]);
        out.extend((tables.len() as u16).to_be_bytes());
        out.extend([0u8; 6]);
        let mut off = base_pad + 12 + 16 * tables.len();
        for (tag, data) in tables {
            out.extend(*tag);
            out.extend([0u8; 4]);
            out.extend((off as u32).to_be_bytes());
            out.extend((data.len() as u32).to_be_bytes());
            off += data.len();
        }
        for (_, data) in tables {
            out.extend(data);
        }
        out
    }

    fn font(family: &str, style: &str, weight: u16, italic: bool) -> Vec<u8> {
        sfnt(
            &[
                (b"name", name_table(&[(1, family), (2, style)])),
                (b"OS/2", os2(weight, italic)),
            ],
            0,
        )
    }

    #[test]
    fn parses_family_weight_italic() {
        let f = parse_faces(&font("Foo Mono", "Bold Italic", 700, true));
        assert_eq!(
            f,
            vec![FaceMeta {
                family: "Foo Mono".into(),
                weight: 700,
                italic: true
            }]
        );
    }

    #[test]
    fn prefers_typographic_family() {
        let d = sfnt(
            &[(
                b"name",
                name_table(&[(1, "Foo Light"), (16, "Foo"), (17, "Light")]),
            )],
            0,
        );
        let f = parse_faces(&d);
        assert_eq!(f[0].family, "Foo");
        assert_eq!(f[0].weight, 400);
    }

    #[test]
    fn parses_ttc_faces() {
        let head = 20;
        let a = sfnt(&[(b"name", name_table(&[(1, "A")]))], head);
        let b = sfnt(&[(b"name", name_table(&[(1, "B")]))], a.len());
        let mut d = b"ttcf".to_vec();
        d.extend([0, 1, 0, 0, 0, 0, 0, 2]);
        d.extend((head as u32).to_be_bytes());
        d.extend((a.len() as u32).to_be_bytes());
        d.extend(&a[head..]);
        d.extend(&b[a.len()..]);
        let faces = parse_faces(&d);
        assert_eq!(faces.len(), 2);
        assert_eq!(faces[0].family, "A");
        assert_eq!(faces[1].family, "B");
    }

    #[test]
    fn truncated_and_garbage_never_panic() {
        let full = font("Foo", "Regular", 400, false);
        for n in 0..full.len() {
            let _ = parse_faces(&full[..n]);
        }
        let _ = parse_faces(&[0xff; 64]);
        let mut bad = full.clone();
        bad[12 + 8..12 + 12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(parse_faces(&bad).is_empty());
        let mut ttc = b"ttcf".to_vec();
        ttc.extend([0, 1, 0, 0, 0xff, 0xff, 0xff, 0xff]);
        let _ = parse_faces(&ttc);
    }

    fn indexed(dir: &Path) -> FontIndex {
        fs::write(
            dir.join("a.ttf"),
            font("JetBrains Mono", "Regular", 400, false),
        )
        .unwrap();
        fs::write(
            dir.join("b.ttf"),
            font("JetBrains Mono", "Bold", 700, false),
        )
        .unwrap();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(
            dir.join("sub/c.otf"),
            font("JetBrainsMono Nerd Font", "Regular", 400, false),
        )
        .unwrap();
        fs::write(
            dir.join("sub/d.ttf"),
            font("JetBrainsMono Nerd Font Mono", "Regular", 400, false),
        )
        .unwrap();
        fs::write(dir.join("junk.ttf"), b"nope").unwrap();
        fs::write(dir.join("note.txt"), b"x").unwrap();
        scan_dirs(&[dir.to_path_buf(), dir.join("missing")])
    }

    #[test]
    fn resolves_by_name_and_compact_name() {
        let dir = tempfile::tempdir().unwrap();
        let idx = indexed(dir.path());
        let r = idx.resolve("jetbrains mono").unwrap();
        assert_eq!(r.family, "JetBrains Mono");
        assert_eq!(r.faces.len(), 2);
        assert_eq!(
            idx.resolve("JetBrainsMono").unwrap().family,
            "JetBrains Mono"
        );
        assert_eq!(
            idx.resolve("JetBrainsMonoNerdFont").unwrap().family,
            "JetBrainsMono Nerd Font"
        );
        assert!(idx.resolve("Nope").is_none());
        assert!(idx.resolve("  ").is_none());
    }

    #[test]
    fn nerd_fallback_prefers_mono_variant() {
        let dir = tempfile::tempdir().unwrap();
        let idx = indexed(dir.path());
        assert_eq!(
            idx.nerd_fallback().as_deref(),
            Some("JetBrainsMono Nerd Font Mono")
        );
        assert!(FontIndex::default().nerd_fallback().is_none());
    }

    #[test]
    fn font_read_ids_come_from_index_only() {
        let dir = tempfile::tempdir().unwrap();
        let idx = indexed(dir.path());
        let id = idx.resolve("JetBrains Mono").unwrap().faces[0].id;
        assert!(idx.path_for(id).is_some());
        assert!(idx.path_for(9999).is_none());
    }
}
