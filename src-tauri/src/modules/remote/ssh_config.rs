use super::target::validate_alias;
use globset::Glob;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_INCLUDE_DEPTH: usize = 16;
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_HOSTS: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SshHost {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
}

struct State<'a> {
    home: &'a Path,
    ssh_dir: &'a Path,
    hosts: Vec<SshHost>,
    current: Vec<usize>,
    visited: HashSet<PathBuf>,
}

pub fn list_user_hosts() -> Vec<SshHost> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let ssh_dir = home.join(".ssh");
    list_hosts(&ssh_dir.join("config"), &home, &ssh_dir)
}

pub fn list_hosts(config: &Path, home: &Path, ssh_dir: &Path) -> Vec<SshHost> {
    let mut st = State {
        home,
        ssh_dir,
        hosts: Vec::new(),
        current: Vec::new(),
        visited: HashSet::new(),
    };
    parse_file(config, 0, &mut st);
    st.hosts
}

fn split_line(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.trim_start();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let kw_end = line
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(line.len());
    let keyword = line[..kw_end].to_ascii_lowercase();
    let rest = line[kw_end..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest);
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut has_token = false;
    for c in rest.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quote => {
                if has_token {
                    args.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            '#' if !in_quote && !has_token => break,
            c => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        args.push(cur);
    }
    Some((keyword, args))
}

fn is_pattern(s: &str) -> bool {
    s.contains(['*', '?', '!'])
}

fn parse_file(path: &Path, depth: usize, st: &mut State) {
    if depth > MAX_INCLUDE_DEPTH {
        return;
    }
    let Some(text) = read_config(path) else {
        return;
    };
    let canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !st.visited.insert(canon) {
        return;
    }
    for line in text.lines() {
        let Some((kw, args)) = split_line(line) else {
            continue;
        };
        match kw.as_str() {
            "host" => {
                st.current.clear();
                for a in args {
                    if is_pattern(&a) || validate_alias(&a).is_err() {
                        continue;
                    }
                    if let Some(i) = st.hosts.iter().position(|h| h.alias == a) {
                        st.current.push(i);
                    } else if st.hosts.len() < MAX_HOSTS {
                        st.hosts.push(SshHost {
                            alias: a,
                            hostname: None,
                            user: None,
                            port: None,
                        });
                        st.current.push(st.hosts.len() - 1);
                    }
                }
            }
            "match" => st.current.clear(),
            "hostname" | "user" | "port" => {
                let Some(v) = args.into_iter().next() else {
                    continue;
                };
                for &i in &st.current {
                    let h = &mut st.hosts[i];
                    match kw.as_str() {
                        "hostname" if h.hostname.is_none() => h.hostname = Some(v.clone()),
                        "user" if h.user.is_none() => h.user = Some(v.clone()),
                        "port" if h.port.is_none() => h.port = v.parse().ok(),
                        _ => {}
                    }
                }
            }
            "include" => {
                let saved = st.current.clone();
                for pat in args {
                    for file in expand_include(&pat, st) {
                        parse_file(&file, depth + 1, st);
                    }
                }
                st.current = saved;
            }
            _ => {}
        }
    }
}

fn read_config(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > MAX_CONFIG_BYTES {
        return None;
    }
    let mut text = String::new();
    file.take(MAX_CONFIG_BYTES).read_to_string(&mut text).ok()?;
    Some(text)
}

fn expand_include(pattern: &str, st: &State) -> Vec<PathBuf> {
    let base = if let Some(rest) = pattern.strip_prefix("~/") {
        st.home.join(rest)
    } else if Path::new(pattern).is_absolute() {
        PathBuf::from(pattern)
    } else {
        st.ssh_dir.join(pattern)
    };
    let mut paths = vec![PathBuf::new()];
    for comp in base.components() {
        let name = comp.as_os_str().to_string_lossy().into_owned();
        let wild = name.contains(['*', '?', '[']);
        let mut next = Vec::new();
        for p in &paths {
            if !wild {
                next.push(p.join(comp.as_os_str()));
                continue;
            }
            let Ok(glob) = Glob::new(&name) else {
                continue;
            };
            let matcher = glob.compile_matcher();
            let Ok(rd) = fs::read_dir(p) else {
                continue;
            };
            let allow_dot = name.starts_with('.');
            let mut found: Vec<PathBuf> = rd
                .flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    (allow_dot || !n.starts_with('.')) && matcher.is_match(&n)
                })
                .map(|e| e.path())
                .collect();
            found.sort();
            next.extend(found);
        }
        paths = next;
    }
    paths.into_iter().filter(|p| p.is_file()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    struct Fx {
        _dir: TempDir,
        home: PathBuf,
        ssh: PathBuf,
    }

    fn fx() -> Fx {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        let ssh = home.join(".ssh");
        fs::create_dir_all(&ssh).unwrap();
        Fx {
            _dir: dir,
            home,
            ssh,
        }
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn run(f: &Fx) -> Vec<SshHost> {
        list_hosts(&f.ssh.join("config"), &f.home, &f.ssh)
    }

    fn aliases(h: &[SshHost]) -> Vec<&str> {
        h.iter().map(|h| h.alias.as_str()).collect()
    }

    #[test]
    fn basic_hosts_with_details() {
        let f = fx();
        write(
            &f.ssh.join("config"),
            "# comment\nHost oracle\n  HostName 1.2.3.4\n  User ubuntu\n  Port 2222\n  IdentityFile ~/.ssh/key\n\nHost imac-local\n  hostname imac.local\n",
        );
        let h = run(&f);
        assert_eq!(
            h,
            vec![
                SshHost {
                    alias: "oracle".into(),
                    hostname: Some("1.2.3.4".into()),
                    user: Some("ubuntu".into()),
                    port: Some(2222)
                },
                SshHost {
                    alias: "imac-local".into(),
                    hostname: Some("imac.local".into()),
                    user: None,
                    port: None
                },
            ]
        );
    }

    #[test]
    fn keywords_case_insensitive_and_equals_and_quotes() {
        let f = fx();
        write(
            &f.ssh.join("config"),
            "HOST=a\nHOSTNAME=h.example\nhOsT \"b\" c\n  uSeR   bob\nHost = d\n",
        );
        let h = run(&f);
        assert_eq!(aliases(&h), vec!["a", "b", "c", "d"]);
        assert_eq!(h[0].hostname.as_deref(), Some("h.example"));
        assert_eq!(h[1].user.as_deref(), Some("bob"));
        assert_eq!(h[2].user.as_deref(), Some("bob"));
        assert_eq!(h[3].user, None);
    }

    #[test]
    fn multiple_aliases_share_block() {
        let f = fx();
        write(
            &f.ssh.join("config"),
            "Host a b\n  HostName shared\n  Port 22\n",
        );
        let h = run(&f);
        assert_eq!(aliases(&h), vec!["a", "b"]);
        assert!(h.iter().all(|x| x.hostname.as_deref() == Some("shared")));
        assert!(h.iter().all(|x| x.port == Some(22)));
    }

    #[test]
    fn patterns_skipped() {
        let f = fx();
        write(
            &f.ssh.join("config"),
            "Host *\n  User all\nHost web-* !web-x real\n  HostName r\nHost ?x\nHost !neg\n",
        );
        let h = run(&f);
        assert_eq!(aliases(&h), vec!["real"]);
        assert_eq!(h[0].user, None);
    }

    #[test]
    fn match_ends_block() {
        let f = fx();
        write(
            &f.ssh.join("config"),
            "Host a\n  User one\nMatch host a\n  User two\n  HostName nope\nHost b\n  User three\n",
        );
        let h = run(&f);
        assert_eq!(h[0].user.as_deref(), Some("one"));
        assert_eq!(h[0].hostname, None);
        assert_eq!(h[1].user.as_deref(), Some("three"));
    }

    #[test]
    fn comments_and_trailing_comments() {
        let f = fx();
        write(
            &f.ssh.join("config"),
            "   # Host hidden\nHost a # trailing\n  HostName h # c\n",
        );
        let h = run(&f);
        assert_eq!(aliases(&h), vec!["a"]);
        assert_eq!(h[0].hostname.as_deref(), Some("h"));
    }

    #[test]
    fn include_relative_tilde_absolute_and_glob() {
        let f = fx();
        write(&f.ssh.join("conf.d/one.conf"), "Host one\n");
        write(&f.ssh.join("conf.d/two.conf"), "Host two\n");
        write(&f.ssh.join("conf.d/skip.txt"), "Host skipped\n");
        write(
            &f.home.join(".orbstack/ssh/config"),
            "Host orb\n  HostName orb.local\n",
        );
        let abs = f.home.join("abs.conf");
        write(&abs, "Host absolute\n");
        write(
            &f.ssh.join("config"),
            &format!(
                "Include ~/.orbstack/ssh/config\nInclude conf.d/*.conf\nInclude {}\nHost mine\n",
                abs.display()
            ),
        );
        let h = run(&f);
        assert_eq!(aliases(&h), vec!["orb", "one", "two", "absolute", "mine"]);
    }

    #[test]
    fn nested_include() {
        let f = fx();
        write(&f.ssh.join("a.conf"), "Host a\nInclude b.conf\n");
        write(&f.ssh.join("b.conf"), "Host b\nInclude c.conf\n");
        write(&f.ssh.join("c.conf"), "Host c\n");
        write(&f.ssh.join("config"), "Include a.conf\n");
        assert_eq!(aliases(&run(&f)), vec!["a", "b", "c"]);
    }

    #[test]
    fn include_cycle_terminates() {
        let f = fx();
        write(&f.ssh.join("a.conf"), "Host a\nInclude b.conf\n");
        write(
            &f.ssh.join("b.conf"),
            "Host b\nInclude a.conf\nInclude config\n",
        );
        write(&f.ssh.join("config"), "Host root\nInclude a.conf\n");
        assert_eq!(aliases(&run(&f)), vec!["root", "a", "b"]);
    }

    #[test]
    fn include_depth_is_bounded() {
        let f = fx();
        for i in 0..40 {
            write(
                &f.ssh.join(format!("f{i}.conf")),
                &format!("Host h{i}\nInclude f{}.conf\n", i + 1),
            );
        }
        write(&f.ssh.join("config"), "Include f0.conf\n");
        let h = run(&f);
        assert!(h.len() <= MAX_INCLUDE_DEPTH + 1);
        assert!(!h.is_empty());
    }

    #[test]
    fn missing_files_and_unsafe_aliases() {
        let f = fx();
        write(
            &f.ssh.join("config"),
            "Include nothing/*.conf\nInclude missing.conf\nHost -oProxyCommand=x ok 'q' a;b\n",
        );
        assert_eq!(aliases(&run(&f)), vec!["ok"]);
        assert!(list_hosts(&f.ssh.join("absent"), &f.home, &f.ssh).is_empty());
    }

    #[test]
    fn first_value_wins_and_duplicate_alias_merges() {
        let f = fx();
        write(
            &f.ssh.join("config"),
            "Host a\n  User first\nHost a\n  User second\n  HostName h\n",
        );
        let h = run(&f);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].user.as_deref(), Some("first"));
        assert_eq!(h[0].hostname.as_deref(), Some("h"));
    }

    #[test]
    fn include_restores_host_block() {
        let f = fx();
        write(
            &f.ssh.join("inc.conf"),
            "Host b
  HostName bh
",
        );
        write(
            &f.ssh.join("config"),
            "Host a
  Include inc.conf
  User u
",
        );
        let h = run(&f);
        assert_eq!(aliases(&h), vec!["a", "b"]);
        assert_eq!(h[0].user.as_deref(), Some("u"));
        assert_eq!(h[1].user, None);
    }

    #[test]
    fn include_glob_skips_dotfiles() {
        let f = fx();
        write(&f.ssh.join("conf.d/ok.conf"), "Host ok\n");
        write(&f.ssh.join("conf.d/.foo.conf.swp"), "Host swp\n");
        write(&f.ssh.join("config"), "Include conf.d/*\n");
        assert_eq!(aliases(&run(&f)), vec!["ok"]);
    }

    #[test]
    fn oversized_or_non_regular_config_ignored() {
        let f = fx();
        let big = format!("Host big\n{}", "#".repeat(MAX_CONFIG_BYTES as usize));
        write(&f.ssh.join("config"), &big);
        assert!(run(&f).is_empty());
        let g = fx();
        fs::create_dir_all(g.ssh.join("config")).unwrap();
        assert!(run(&g).is_empty());
    }

    #[test]
    fn file_included_twice_parsed_once() {
        let f = fx();
        write(&f.ssh.join("x.conf"), "Host x\n  User first\n");
        write(
            &f.ssh.join("config"),
            "Include x.conf\nHost y\nInclude x.conf\n  User late\n",
        );
        let h = run(&f);
        assert_eq!(aliases(&h), vec!["x", "y"]);
        assert_eq!(h[1].user.as_deref(), Some("late"));
    }
}
