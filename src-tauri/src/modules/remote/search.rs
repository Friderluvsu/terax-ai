use super::control;
use super::exec::exec;
use super::quote::posix_quote;
use super::state::RemoteState;
use super::target;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const LIST_TIMEOUT: Duration = Duration::from_secs(60);
pub const GREP_TIMEOUT: Duration = Duration::from_secs(60);
const LIST_MAX_OUTPUT: usize = 8 * 1024 * 1024;
const GREP_MAX_OUTPUT: usize = 4 * 1024 * 1024;
const GREP_SIZE_KIB: u64 = 5 * 1024;
const NOT_A_DIR: i32 = 3;

pub const PRUNE_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".cache",
    ".venv",
    "__pycache__",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Rg,
    Find,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseMode {
    Sensitive,
    Insensitive,
    Smart,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawHit {
    pub rel: String,
    pub line: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepOutput {
    pub tool: Tool,
    pub hits: Vec<RawHit>,
    pub searched: Option<usize>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub files: Vec<String>,
    pub truncated: bool,
}

pub fn socket_dir_for(
    identifier: &str,
    state: &RemoteState,
    host: &str,
) -> Result<PathBuf, String> {
    target::validate_alias(host)?;
    state.socket_dir(identifier)
}

pub fn join_display(root: &str, rel: &str) -> String {
    if root.ends_with('/') {
        format!("{root}{rel}")
    } else {
        format!("{root}/{rel}")
    }
}

fn check_root(root: &str) -> Result<(), String> {
    if root.starts_with('/') {
        Ok(())
    } else {
        Err(format!("not an absolute remote path: {root}"))
    }
}

fn prune_expr(show_hidden: bool) -> String {
    let mut parts: Vec<String> = PRUNE_DIRS
        .iter()
        .map(|d| format!("-name {}", posix_quote(d)))
        .collect();
    if !show_hidden {
        parts.push("\\( ! -name . -name '.*' \\)".to_string());
    }
    format!("\\( {} \\) -prune -o", parts.join(" -o "))
}

fn find_files_cmd(show_hidden: bool, max_depth: Option<usize>, extra_tests: &str) -> String {
    let depth = max_depth
        .map(|d| format!(" -maxdepth {d}"))
        .unwrap_or_default();
    format!(
        "find .{depth} {} -type f{extra_tests} -print0",
        prune_expr(show_hidden)
    )
}

fn rg_prune_globs() -> String {
    PRUNE_DIRS
        .iter()
        .map(|d| format!("-g {}", posix_quote(&format!("!{d}"))))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn list_script(
    root: &str,
    show_hidden: bool,
    max_depth: Option<usize>,
    max_output: usize,
) -> String {
    let mut rg = format!(
        "rg --no-config --no-messages --files -0 {}",
        rg_prune_globs()
    );
    if show_hidden {
        rg.push_str(" --hidden");
    }
    if let Some(d) = max_depth {
        rg.push_str(&format!(" --max-depth {d}"));
    }
    format!(
        "cd -- {root} || exit {NOT_A_DIR}\n\
         if command -v rg >/dev/null 2>&1; then\n\
         printf 'rg\\n'\n\
         {{ {rg} -- . </dev/null 2>/dev/null; }} | head -c {max_output}\n\
         else\n\
         printf 'find\\n'\n\
         {{ {find} </dev/null 2>/dev/null; }} | head -c {max_output}\n\
         fi\n\
         exit 0\n",
        root = posix_quote(root),
        find = find_files_cmd(show_hidden, max_depth, ""),
    )
}

pub fn grep_script(
    root: &str,
    pattern: &str,
    literal: bool,
    case: CaseMode,
    globs: &[String],
    max_output: usize,
) -> String {
    let mut rg = format!(
        "rg --no-config --no-messages --json -n --max-filesize {}K {}",
        GREP_SIZE_KIB,
        rg_prune_globs()
    );
    let mut grep = String::from("grep -nIH");
    if literal {
        rg.push_str(" -F");
        grep.push_str(" -F");
    } else {
        grep.push_str(" -E");
    }
    match case {
        CaseMode::Sensitive => {}
        CaseMode::Insensitive => {
            rg.push_str(" -i");
            grep.push_str(" -i");
        }
        CaseMode::Smart => {
            rg.push_str(" -S");
            if !pattern.chars().any(char::is_uppercase) {
                grep.push_str(" -i");
            }
        }
    }
    for g in globs {
        rg.push_str(&format!(" -g {}", posix_quote(g)));
    }
    let pat = posix_quote(pattern);
    format!(
        "cd -- {root} || exit {NOT_A_DIR}\n\
         if command -v rg >/dev/null 2>&1; then\n\
         printf 'rg\\n'\n\
         {{ {rg} -e {pat} -- . </dev/null 2>/dev/null; }} | head -c {max_output}\n\
         else\n\
         printf 'find\\n'\n\
         {{ {find} 2>/dev/null | xargs -0 {grep} -e {pat} /dev/null 2>/dev/null; }} | head -c {max_output}\n\
         fi\n\
         exit 0\n",
        root = posix_quote(root),
        find = find_files_cmd(false, None, &format!(" -size -{}k", GREP_SIZE_KIB + 1)),
    )
}

fn split_tool(out: &[u8]) -> Result<(Tool, &[u8]), String> {
    let nl = out
        .iter()
        .position(|b| *b == b'\n')
        .ok_or("remote search produced no output")?;
    let tool = match &out[..nl] {
        b"rg" => Tool::Rg,
        b"find" => Tool::Find,
        _ => return Err("unexpected remote search output".into()),
    };
    Ok((tool, &out[nl + 1..]))
}

fn strip_dot(p: &str) -> &str {
    p.strip_prefix("./").unwrap_or(p)
}

fn drop_partial_tail(body: &[u8], sep: u8, truncated: bool) -> &[u8] {
    if !truncated {
        return body;
    }
    match body.iter().rposition(|b| *b == sep) {
        Some(i) => &body[..=i],
        None => &[],
    }
}

pub fn parse_file_list(out: &[u8], truncated: bool) -> Result<Listing, String> {
    let (_, body) = split_tool(out)?;
    let body = drop_partial_tail(body, 0, truncated);
    let files = body
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| strip_dot(&String::from_utf8_lossy(s)).to_string())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(Listing { files, truncated })
}

pub fn derive_dirs(files: &[String]) -> Vec<String> {
    let mut dirs = std::collections::BTreeSet::new();
    for f in files {
        let mut end = f.len();
        while let Some(i) = f[..end].rfind('/') {
            if !dirs.insert(f[..i].to_string()) {
                break;
            }
            end = i;
        }
    }
    dirs.into_iter().collect()
}

fn json_text(v: &Value) -> Option<&str> {
    v.get("text").and_then(Value::as_str)
}

pub fn parse_rg_json(body: &[u8]) -> (Vec<RawHit>, Option<usize>) {
    let mut hits = Vec::new();
    let mut searched = None;
    for line in body.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        let data = &v["data"];
        match v["type"].as_str() {
            Some("match") => {
                let (Some(path), Some(n), Some(text)) = (
                    json_text(&data["path"]),
                    data["line_number"].as_u64(),
                    json_text(&data["lines"]),
                ) else {
                    continue;
                };
                hits.push(RawHit {
                    rel: strip_dot(path).to_string(),
                    line: n,
                    text: text.trim_end_matches(['\n', '\r']).to_string(),
                });
            }
            Some("summary") => {
                searched = data["stats"]["searches"].as_u64().map(|n| n as usize);
            }
            _ => {}
        }
    }
    (hits, searched)
}

fn split_grep_line(line: &str) -> Option<RawHit> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b':' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > start && j < bytes.len() && bytes[j] == b':' {
                let n = line[start..j].parse().ok()?;
                return Some(RawHit {
                    rel: strip_dot(&line[..i]).to_string(),
                    line: n,
                    text: line[j + 1..].trim_end_matches('\r').to_string(),
                });
            }
        }
        i += 1;
    }
    None
}

pub fn parse_grep_lines(body: &[u8]) -> Vec<RawHit> {
    String::from_utf8_lossy(body)
        .split('\n')
        .filter_map(split_grep_line)
        .collect()
}

pub fn parse_grep_output(out: &[u8], truncated: bool) -> Result<GrepOutput, String> {
    let (tool, body) = split_tool(out)?;
    let body = drop_partial_tail(body, b'\n', truncated);
    let (hits, searched) = match tool {
        Tool::Rg => parse_rg_json(body),
        Tool::Find => (parse_grep_lines(body), None),
    };
    Ok(GrepOutput {
        tool,
        hits,
        searched,
        truncated,
    })
}

fn run_script(
    dir: &Path,
    host: &str,
    root: &str,
    script: &str,
    timeout: Duration,
    max_output: usize,
) -> Result<super::exec::RawOutput, String> {
    let out = exec(dir, host, script, timeout, max_output)?;
    if out.status == Some(NOT_A_DIR) {
        return Err(format!("not a directory: {root}"));
    }
    control::ensure_success(&out)?;
    Ok(out)
}

pub fn list_files(
    dir: &Path,
    host: &str,
    root: &str,
    show_hidden: bool,
    max_depth: Option<usize>,
) -> Result<Listing, String> {
    check_root(root)?;
    let script = list_script(root, show_hidden, max_depth, LIST_MAX_OUTPUT);
    let out = run_script(dir, host, root, &script, LIST_TIMEOUT, LIST_MAX_OUTPUT)?;
    parse_file_list(&out.stdout, out.truncated)
}

pub fn grep(
    dir: &Path,
    host: &str,
    root: &str,
    pattern: &str,
    literal: bool,
    case: CaseMode,
    globs: &[String],
) -> Result<GrepOutput, String> {
    check_root(root)?;
    let script = grep_script(root, pattern, literal, case, globs, GREP_MAX_OUTPUT);
    let out = run_script(dir, host, root, &script, GREP_TIMEOUT, GREP_MAX_OUTPUT)?;
    parse_grep_output(&out.stdout, out.truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GNU_FIND: &[u8] = b"find\n./src/main.rs\0./README.md\0./a b/c d.txt\0./odd\nname.txt\0";
    const BSD_FIND: &[u8] = b"find\n./src/lib.rs\0./docs/guide.md\0";
    const RG_LIST: &[u8] = b"rg\n./src/main.rs\0./README.md\0";

    #[test]
    fn parses_gnu_find_listing() {
        let l = parse_file_list(GNU_FIND, false).unwrap();
        assert_eq!(
            l.files,
            vec!["src/main.rs", "README.md", "a b/c d.txt", "odd\nname.txt"]
        );
        assert!(!l.truncated);
    }

    #[test]
    fn parses_bsd_find_and_rg_listing() {
        assert_eq!(
            parse_file_list(BSD_FIND, false).unwrap().files,
            vec!["src/lib.rs", "docs/guide.md"]
        );
        assert_eq!(
            parse_file_list(RG_LIST, false).unwrap().files,
            vec!["src/main.rs", "README.md"]
        );
    }

    #[test]
    fn truncated_listing_drops_partial_tail() {
        let l = parse_file_list(b"find\n./a\0./b\0./par", true).unwrap();
        assert_eq!(l.files, vec!["a", "b"]);
        assert!(l.truncated);
    }

    #[test]
    fn rejects_output_without_tool_marker() {
        assert!(parse_file_list(b"", false).is_err());
        assert!(parse_file_list(b"garbage\n./a\0", false).is_err());
    }

    #[test]
    fn derives_parent_dirs_once() {
        let files = vec!["a/b/c.rs".to_string(), "a/d.rs".to_string(), "e.rs".into()];
        assert_eq!(derive_dirs(&files), vec!["a", "a/b"]);
    }

    const RG_JSON: &[u8] = br#"rg
{"type":"begin","data":{"path":{"text":"./src/main.rs"}}}
{"type":"match","data":{"path":{"text":"./src/main.rs"},"lines":{"text":"fn main() { let x = 'a'; }\n"},"line_number":3,"absolute_offset":10,"submatches":[]}}
{"type":"end","data":{"path":{"text":"./src/main.rs"},"binary_offset":null,"stats":{}}}
{"type":"match","data":{"path":{"bytes":"Li9iYWQ="},"lines":{"text":"x\n"},"line_number":1,"absolute_offset":0,"submatches":[]}}
{"type":"match","data":{"path":{"text":"./a: 1:b.txt"},"lines":{"text":"tab\there\r\n"},"line_number":12,"absolute_offset":0,"submatches":[]}}
{"type":"summary","data":{"elapsed_total":{"secs":0,"nanos":1,"human":"0s"},"stats":{"searches":7,"searches_with_match":2}}}
"#;

    #[test]
    fn parses_rg_json() {
        let g = parse_grep_output(RG_JSON, false).unwrap();
        assert_eq!(g.tool, Tool::Rg);
        assert_eq!(g.searched, Some(7));
        assert_eq!(g.hits.len(), 2);
        assert_eq!(g.hits[0].rel, "src/main.rs");
        assert_eq!(g.hits[0].line, 3);
        assert_eq!(g.hits[0].text, "fn main() { let x = 'a'; }");
        assert_eq!(g.hits[1].rel, "a: 1:b.txt");
        assert_eq!(g.hits[1].text, "tab\there");
    }

    #[test]
    fn rg_json_truncated_drops_partial_line() {
        let cut = &RG_JSON[..RG_JSON.len() - 40];
        let g = parse_grep_output(cut, true).unwrap();
        assert_eq!(g.hits.len(), 2);
        assert_eq!(g.searched, None);
    }

    #[test]
    fn parses_gnu_grep_lines() {
        let out = b"find\n./src/main.rs:3:fn main() {}\n./a b/c.txt:10:key: value: 5:x\n";
        let g = parse_grep_output(out, false).unwrap();
        assert_eq!(g.tool, Tool::Find);
        assert_eq!(g.hits.len(), 2);
        assert_eq!(g.hits[0].rel, "src/main.rs");
        assert_eq!(g.hits[0].line, 3);
        assert_eq!(g.hits[1].rel, "a b/c.txt");
        assert_eq!(g.hits[1].line, 10);
        assert_eq!(g.hits[1].text, "key: value: 5:x");
    }

    #[test]
    fn parses_bsd_grep_lines() {
        let out =
            b"find\n./lib/x.c:42:\tint y;\r\n./lib/x.c:43:\n/dev/null\nBinary file ./b matches\n";
        let g = parse_grep_output(out, false).unwrap();
        assert_eq!(g.hits.len(), 2);
        assert_eq!(g.hits[0].text, "\tint y;");
        assert_eq!(g.hits[1].text, "");
    }

    #[test]
    fn truncated_grep_drops_partial_line() {
        let out = b"find\n./a:1:one\n./b:2:tw";
        let g = parse_grep_output(out, true).unwrap();
        assert_eq!(g.hits.len(), 1);
        assert!(g.truncated);
    }

    #[test]
    fn grep_script_quotes_hostile_pattern() {
        let pat = "it's $HOME `id` a b";
        let s = grep_script("/srv/my dir", pat, true, CaseMode::Sensitive, &[], 1 << 20);
        assert!(s.contains(&format!("-e {}", posix_quote(pat))));
        assert!(s.contains("cd -- '/srv/my dir'"));
        assert!(s.contains("'it'\\''s $HOME `id` a b'"));
        assert!(!s.contains("-e it's"));
    }

    #[test]
    fn grep_script_flags() {
        let globs = vec!["*.rs".to_string(), "it's".to_string()];
        let s = grep_script("/r", "Abc", false, CaseMode::Smart, &globs, 1 << 20);
        assert!(s.contains("-S"));
        assert!(s.contains("grep -nIH -E -e"));
        assert!(!s.contains("grep -nIH -E -i"));
        assert!(s.contains("-g '*.rs'"));
        assert!(s.contains("-g 'it'\\''s'"));
        let s = grep_script("/r", "abc", true, CaseMode::Smart, &[], 1 << 20);
        assert!(s.contains("grep -nIH -F -i"));
        let s = grep_script("/r", "abc", true, CaseMode::Insensitive, &[], 1 << 20);
        assert!(s.contains("rg --no-config --no-messages --json -n") && s.contains(" -F -i"));
    }

    #[test]
    fn list_script_options() {
        let s = list_script("/r", true, Some(4), 1 << 20);
        assert!(s.contains("--hidden"));
        assert!(s.contains("--max-depth 4"));
        assert!(s.contains("find . -maxdepth 4"));
        assert!(!s.contains("-name '.*'"));
        let s = list_script("/r", false, None, 1 << 20);
        assert!(!s.contains("--hidden"));
        assert!(s.contains("-name '.*'"));
        assert!(s.contains("-g '!node_modules'"));
        assert!(!s.contains("-printf"));
        assert!(!s.contains("--null-data"));
    }

    #[test]
    fn root_must_be_absolute() {
        assert!(check_root("/home/u").is_ok());
        assert!(check_root("~/x").is_err());
        assert!(check_root("").is_err());
    }

    #[test]
    fn join_display_handles_trailing_slash() {
        assert_eq!(join_display("/a", "b/c"), "/a/b/c");
        assert_eq!(join_display("/a/", "b"), "/a/b");
    }

    #[cfg(unix)]
    fn run_local(script: &str) -> Vec<u8> {
        let out = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }

    #[cfg(unix)]
    #[test]
    fn scripts_run_against_a_local_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        let sub = root.join("it's $x");
        std::fs::create_dir_all(sub.join("node_modules")).unwrap();
        std::fs::write(sub.join("a b.txt"), "hello 'World' $HOME\nother\n").unwrap();
        std::fs::write(sub.join("node_modules/skip.txt"), "hello\n").unwrap();
        std::fs::write(sub.join(".hid.txt"), "hello\n").unwrap();
        let r = sub.to_str().unwrap();

        let l = parse_file_list(&run_local(&list_script(r, false, None, 1 << 20)), false).unwrap();
        assert_eq!(l.files, vec!["a b.txt"]);
        let l = parse_file_list(&run_local(&list_script(r, true, None, 1 << 20)), false).unwrap();
        let mut f = l.files;
        f.sort();
        assert_eq!(f, vec![".hid.txt", "a b.txt"]);

        let s = grep_script(r, "'World' $HOME", true, CaseMode::Sensitive, &[], 1 << 20);
        let g = parse_grep_output(&run_local(&s), false).unwrap();
        assert_eq!(g.hits.len(), 1);
        assert_eq!(g.hits[0].rel, "a b.txt");
        assert_eq!(g.hits[0].line, 1);
    }

    #[test]
    fn scripts_bound_output_with_head() {
        assert!(list_script("/r", false, None, 4096).contains("| head -c 4096"));
        assert!(
            grep_script("/r", "x", true, CaseMode::Sensitive, &[], 4096)
                .matches("| head -c 4096")
                .count()
                == 2
        );
    }

    #[cfg(unix)]
    #[test]
    fn oversized_output_is_cut_and_marked_truncated() {
        let tmp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        for i in 0..3000 {
            std::fs::write(root.join(format!("file_number_{i}.txt")), "hit\n").unwrap();
        }
        let r = root.to_str().unwrap();
        let cap = 2048;
        let started = std::time::Instant::now();
        let out = run_local(&list_script(r, false, None, cap));
        assert!(started.elapsed() < Duration::from_secs(20));
        // Tool marker line plus at most `cap` body bytes; exec flags this as truncated.
        assert!(out.len() > cap && out.len() <= cap + 8);
        let l = parse_file_list(&out, true).unwrap();
        assert!(l.truncated);
        assert!(!l.files.is_empty() && l.files.len() < 3000);
        assert!(l.files.iter().all(|f| f.starts_with("file_number_")));

        let s = grep_script(r, "hit", true, CaseMode::Sensitive, &[], cap);
        let out = run_local(&s);
        assert!(out.len() > cap && out.len() <= cap + 8);
        let g = parse_grep_output(&out, true).unwrap();
        assert!(g.truncated);
    }
}
