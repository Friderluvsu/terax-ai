use super::exec::{run, RawOutput};
use super::target::validate_alias;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEV_IDENTIFIER: &str = "app.crynta.terax.dev";
const FORK_IDENTIFIER: &str = "dev.terax.fork";
// macOS sun_path is 104 bytes with the NUL; ssh binds "<path>.<16 chars>" first.
const MAX_SOCKET_PATH: usize = 103;
const CONTROL_HASH_LEN: usize = 40;
const MASTER_TEMP_SUFFIX_LEN: usize = 17;
const CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);
const RESOLVE_MAX_OUTPUT: usize = 256 * 1024;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Interactive,
    Batch,
}

pub fn socket_dir_name(identifier: &str) -> String {
    match identifier {
        FORK_IDENTIFIER => "cm".to_string(),
        DEV_IDENTIFIER => "cm-dev".to_string(),
        other => {
            let safe: String = other
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            format!("cm-{safe}")
        }
    }
}

fn is_safe_path_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+' | '@')
}

pub fn socket_dir(home: &Path, identifier: &str) -> Result<PathBuf, String> {
    let dir = home.join(".terax").join(socket_dir_name(identifier));
    let text = dir
        .to_str()
        .ok_or_else(|| "ssh socket directory is not valid UTF-8".to_string())?;
    if !text.starts_with('/') || !text.chars().all(is_safe_path_char) {
        return Err(format!(
            "ssh socket directory {text} contains unsupported characters"
        ));
    }
    if text.len() + 1 + CONTROL_HASH_LEN + MASTER_TEMP_SUFFIX_LEN > MAX_SOCKET_PATH {
        return Err(format!(
            "ssh socket directory {text} is too long for a control socket"
        ));
    }
    Ok(dir)
}

pub fn ensure_socket_dir(dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| format!("create ssh socket directory {}: {e}", dir.display()))?;
    let meta = std::fs::symlink_metadata(dir)
        .map_err(|e| format!("stat ssh socket directory {}: {e}", dir.display()))?;
    if !meta.file_type().is_dir() {
        return Err(format!("{} is not a directory", dir.display()));
    }
    // SAFETY: geteuid has no preconditions.
    if meta.uid() != unsafe { libc::geteuid() } {
        return Err(format!(
            "{} is not owned by the current user",
            dir.display()
        ));
    }
    if meta.permissions().mode() & 0o777 != 0o700 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("secure ssh socket directory {}: {e}", dir.display()))?;
    }
    Ok(())
}

pub fn control_path(dir: &Path) -> String {
    format!("{}/%C", dir.display())
}

pub fn base_options(mode: Mode, control_path: &str) -> Vec<String> {
    let mut opts: Vec<String> = Vec::new();
    let mut push = |o: String| {
        opts.push("-o".to_string());
        opts.push(o);
    };
    match mode {
        Mode::Interactive => {
            push("ControlMaster=auto".into());
            push(format!("ControlPath={control_path}"));
            push("ControlPersist=600".into());
            push("ServerAliveInterval=30".into());
            push("RemoteCommand=none".into());
            push("ForwardAgent=no".into());
        }
        Mode::Batch => {
            push("ControlMaster=no".into());
            push(format!("ControlPath={control_path}"));
            push("BatchMode=yes".into());
            push("ConnectTimeout=10".into());
            push("RemoteCommand=none".into());
            push("RequestTTY=no".into());
            push("ClearAllForwardings=yes".into());
            push("PermitLocalCommand=no".into());
            push("ForwardAgent=no".into());
            opts.push("-T".to_string());
        }
    }
    opts
}

pub fn build_args(
    mode: Mode,
    control_path: &str,
    pre_host: &[&str],
    host: &str,
    post_host: &[&str],
) -> Result<Vec<String>, String> {
    validate_alias(host)?;
    let mut args = base_options(mode, control_path);
    args.extend(pre_host.iter().map(|s| s.to_string()));
    args.push("--".to_string());
    args.push(host.to_string());
    args.extend(post_host.iter().map(|s| s.to_string()));
    Ok(args)
}

pub fn ssh_binary() -> Result<PathBuf, String> {
    which::which("ssh").map_err(|_| "ssh executable not found in PATH".to_string())
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshResolved {
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub proxy_jump: Option<String>,
    pub proxy_command: Option<String>,
    pub identity_agent: Option<String>,
    pub identity_files: Vec<String>,
}

fn non_none(value: &str) -> Option<String> {
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(value.to_string())
    }
}

pub fn parse_ssh_g(text: &str) -> SshResolved {
    let mut out = SshResolved::default();
    for line in text.lines() {
        let line = line.trim();
        let (key, value) = match line.split_once(char::is_whitespace) {
            Some((k, v)) => (k.to_ascii_lowercase(), v.trim()),
            None => continue,
        };
        match key.as_str() {
            "hostname" if out.hostname.is_none() => out.hostname = non_none(value),
            "user" if out.user.is_none() => out.user = non_none(value),
            "port" if out.port.is_none() => out.port = value.parse().ok(),
            "proxyjump" if out.proxy_jump.is_none() => out.proxy_jump = non_none(value),
            "proxycommand" if out.proxy_command.is_none() => out.proxy_command = non_none(value),
            "identityagent" if out.identity_agent.is_none() => out.identity_agent = non_none(value),
            "identityfile" => {
                if let Some(v) = non_none(value) {
                    out.identity_files.push(v);
                }
            }
            _ => {}
        }
    }
    out
}

fn failure(out: &RawOutput) -> String {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        return stderr.to_string();
    }
    match out.status {
        Some(code) => format!("ssh exited with status {code}"),
        None => "ssh was terminated by a signal".to_string(),
    }
}

pub fn ensure_success(out: &RawOutput) -> Result<(), String> {
    if out.status == Some(0) {
        Ok(())
    } else {
        Err(failure(out))
    }
}

fn control_command(dir: &Path, op: &str, host: &str) -> Result<RawOutput, String> {
    let args = build_args(Mode::Batch, &control_path(dir), &["-O", op], host, &[])?;
    run(&ssh_binary()?, &args, CONTROL_TIMEOUT, 64 * 1024)
}

pub fn check(dir: &Path, host: &str) -> Result<bool, String> {
    Ok(control_command(dir, "check", host)?.status == Some(0))
}

pub fn exit(dir: &Path, host: &str) -> Result<bool, String> {
    Ok(control_command(dir, "exit", host)?.status == Some(0))
}

pub fn resolve(host: &str) -> Result<SshResolved, String> {
    validate_alias(host)?;
    let args: Vec<String> = ["-G", "--", host].iter().map(|s| s.to_string()).collect();
    let out = run(&ssh_binary()?, &args, RESOLVE_TIMEOUT, RESOLVE_MAX_OUTPUT)?;
    ensure_success(&out)?;
    Ok(parse_ssh_g(&String::from_utf8_lossy(&out.stdout)))
}

fn is_control_socket_name(name: &str) -> bool {
    name.len() == CONTROL_HASH_LEN && name.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn master_sockets(dir: &Path) -> Vec<PathBuf> {
    use std::os::unix::fs::FileTypeExt;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_socket()).unwrap_or(false))
        .filter(|e| e.file_name().to_str().is_some_and(is_control_socket_name))
        .map(|e| e.path())
        .collect()
}

pub fn shutdown_args(socket: &Path) -> Vec<String> {
    [
        "-F",
        "/dev/null",
        "-o",
        &format!("ControlPath={}", socket.display()),
        "-O",
        "exit",
        "--",
        "terax",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn exit_all(dir: &Path) {
    let Ok(ssh) = ssh_binary() else {
        return;
    };
    let handles: Vec<_> = master_sockets(dir)
        .into_iter()
        .map(|socket| {
            let ssh = ssh.clone();
            std::thread::spawn(move || {
                let _ = run(&ssh, &shutdown_args(&socket), SHUTDOWN_TIMEOUT, 4096);
            })
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CP: &str = "/Users/u/.terax/cm/%C";

    fn opt_values(args: &[String]) -> Vec<&str> {
        args.windows(2)
            .filter(|w| w[0] == "-o")
            .map(|w| w[1].as_str())
            .collect()
    }

    #[test]
    fn socket_dir_depends_on_identifier() {
        assert_eq!(socket_dir_name("dev.terax.fork"), "cm");
        assert_eq!(socket_dir_name("app.crynta.terax.dev"), "cm-dev");
        assert_eq!(socket_dir_name("app.crynta.terax"), "cm-app.crynta.terax");
        assert_eq!(socket_dir_name("a/b c"), "cm-a_b_c");
        let home = Path::new("/Users/frider");
        assert_eq!(
            socket_dir(home, "dev.terax.fork").unwrap(),
            PathBuf::from("/Users/frider/.terax/cm")
        );
        assert_eq!(
            socket_dir(home, "app.crynta.terax.dev").unwrap(),
            PathBuf::from("/Users/frider/.terax/cm-dev")
        );
        assert_ne!(
            socket_dir(home, "dev.terax.fork").unwrap(),
            socket_dir(home, "app.crynta.terax.dev").unwrap()
        );
    }

    #[test]
    fn socket_path_fits_sun_path() {
        let home = PathBuf::from("/Users/frider");
        let dir = socket_dir(&home, "app.crynta.terax.dev").unwrap();
        let worst = format!("{}/{}.{}", dir.display(), "a".repeat(40), "b".repeat(16));
        assert!(worst.len() <= MAX_SOCKET_PATH);

        let long_home = PathBuf::from(format!("/home/{}", "x".repeat(40)));
        assert!(socket_dir(&long_home, "dev.terax.fork").is_err());
    }

    #[test]
    fn socket_dir_rejects_unsafe_home() {
        for home in [
            "/Users/a b",
            "/Users/a%h",
            "/Users/a\"b",
            "relative/home",
            "/Users/a'b",
        ] {
            assert!(
                socket_dir(Path::new(home), "dev.terax.fork").is_err(),
                "{home}"
            );
        }
    }

    #[test]
    fn ensure_socket_dir_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".terax").join("cm");
        ensure_socket_dir(&dir).unwrap();
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        ensure_socket_dir(&dir).unwrap();
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[test]
    fn ensure_socket_dir_rejects_symlink_and_file() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("elsewhere");
        std::fs::create_dir(&target).unwrap();
        let link = tmp.path().join("cm");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(ensure_socket_dir(&link).is_err());
        let file = tmp.path().join("file");
        std::fs::write(&file, "x").unwrap();
        assert!(ensure_socket_dir(&file).is_err());
    }

    #[test]
    fn interactive_args() {
        let args = build_args(Mode::Interactive, CP, &["-tt"], "oracle", &[]).unwrap();
        assert_eq!(
            args,
            vec![
                "-o",
                "ControlMaster=auto",
                "-o",
                "ControlPath=/Users/u/.terax/cm/%C",
                "-o",
                "ControlPersist=600",
                "-o",
                "ServerAliveInterval=30",
                "-o",
                "RemoteCommand=none",
                "-o",
                "ForwardAgent=no",
                "-tt",
                "--",
                "oracle",
            ]
        );
    }

    #[test]
    fn batch_args() {
        let args = build_args(Mode::Batch, CP, &["-O", "check"], "oracle", &[]).unwrap();
        assert_eq!(
            args,
            vec![
                "-o",
                "ControlMaster=no",
                "-o",
                "ControlPath=/Users/u/.terax/cm/%C",
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=10",
                "-o",
                "RemoteCommand=none",
                "-o",
                "RequestTTY=no",
                "-o",
                "ClearAllForwardings=yes",
                "-o",
                "PermitLocalCommand=no",
                "-o",
                "ForwardAgent=no",
                "-T",
                "-O",
                "check",
                "--",
                "oracle",
            ]
        );
        let exec = build_args(Mode::Batch, CP, &[], "oracle", &["echo hi"]).unwrap();
        let n = exec.len();
        assert_eq!(&exec[n - 3..], &["--", "oracle", "echo hi"]);
    }

    #[test]
    fn args_never_weaken_host_keys_or_forward_agent() {
        for mode in [Mode::Interactive, Mode::Batch] {
            let args = build_args(mode, CP, &[], "oracle", &[]).unwrap();
            let opts = opt_values(&args);
            assert!(!args.iter().any(|a| a.contains("StrictHostKeyChecking")));
            assert!(!args.iter().any(|a| a == "-A"));
            assert!(opts.contains(&"ForwardAgent=no"));
            assert!(opts.contains(&"RemoteCommand=none"));
            assert!(opts.contains(&"ControlPath=/Users/u/.terax/cm/%C"));
            let dd = args.iter().position(|a| a == "--").unwrap();
            assert_eq!(args[dd + 1], "oracle");
        }
        let batch = build_args(Mode::Batch, CP, &[], "oracle", &[]).unwrap();
        assert!(opt_values(&batch).contains(&"ControlMaster=no"));
        assert!(!batch.iter().any(|a| a == "ControlMaster=auto"));
    }

    #[test]
    fn build_args_rejects_bad_alias() {
        for host in ["-oProxyCommand=x", "", "a b", "a;b"] {
            assert!(build_args(Mode::Batch, CP, &[], host, &[]).is_err());
            assert!(build_args(Mode::Interactive, CP, &[], host, &[]).is_err());
        }
    }

    #[test]
    fn parses_ssh_g_output() {
        let text = "host oracle\n\
            user ubuntu\n\
            hostname 10.0.0.5\n\
            port 2222\n\
            proxyjump bastion\n\
            identityagent none\n\
            identityfile ~/.ssh/id_ed25519\n\
            identityfile ~/.ssh/id_rsa\n\
            sendenv LANG\n\
            controlpath /Users/u/.ssh/master-ubuntu@10.0.0.5:2222\n\
            \n\
            garbage\n";
        let r = parse_ssh_g(text);
        assert_eq!(r.hostname.as_deref(), Some("10.0.0.5"));
        assert_eq!(r.user.as_deref(), Some("ubuntu"));
        assert_eq!(r.port, Some(2222));
        assert_eq!(r.proxy_jump.as_deref(), Some("bastion"));
        assert_eq!(r.proxy_command, None);
        assert_eq!(r.identity_agent, None);
        assert_eq!(r.identity_files, vec!["~/.ssh/id_ed25519", "~/.ssh/id_rsa"]);
    }

    #[test]
    fn parses_ssh_g_values_with_spaces() {
        let r = parse_ssh_g("proxycommand ssh -W %h:%p jump\nport nope\n");
        assert_eq!(r.proxy_command.as_deref(), Some("ssh -W %h:%p jump"));
        assert_eq!(r.port, None);
        assert_eq!(parse_ssh_g(""), SshResolved::default());
    }

    #[test]
    fn our_options_override_user_host_star() {
        let Ok(ssh) = ssh_binary() else {
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config");
        std::fs::write(
            &cfg,
            "Host *\n  ControlMaster auto\n  ControlPath ~/.ssh/master-%r@%h:%p\n  \
             ControlPersist 30m\n  RequestTTY force\n  RemoteCommand tmux\n  \
             ForwardAgent yes\n  LocalForward 8080 localhost:80\n  PermitLocalCommand yes\n",
        )
        .unwrap();
        let cfg = cfg.to_str().unwrap();
        let dump = |mode: Mode| -> String {
            let mut args = vec!["-F".to_string(), cfg.to_string(), "-G".to_string()];
            args.extend(build_args(mode, "/tmp/terax-cm/%C", &[], "example.invalid", &[]).unwrap());
            let out = run(&ssh, &args, Duration::from_secs(10), 1 << 20).unwrap();
            assert_eq!(
                out.status,
                Some(0),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).unwrap()
        };
        let has = |text: &str, line: &str| text.lines().any(|l| l == line);

        let batch = dump(Mode::Batch);
        assert!(has(&batch, "controlmaster false") || has(&batch, "controlmaster no"));
        assert!(batch
            .lines()
            .any(|l| l.starts_with("controlpath /tmp/terax-cm/")));
        assert!(has(&batch, "batchmode yes"));
        assert!(has(&batch, "requesttty false") || has(&batch, "requesttty no"));
        assert!(has(&batch, "clearallforwardings yes"));
        assert!(has(&batch, "permitlocalcommand no"));
        assert!(has(&batch, "forwardagent no"));
        assert!(!batch.lines().any(|l| l.starts_with("remotecommand")));

        let pty = dump(Mode::Interactive);
        assert!(has(&pty, "controlmaster auto") || has(&pty, "controlmaster true"));
        assert!(pty
            .lines()
            .any(|l| l.starts_with("controlpath /tmp/terax-cm/")));
        assert!(has(&pty, "controlpersist 600"));
        assert!(has(&pty, "serveraliveinterval 30"));
        assert!(has(&pty, "forwardagent no"));
        assert!(!pty.lines().any(|l| l.starts_with("remotecommand")));
    }

    #[test]
    fn master_sockets_only_lists_control_sockets() {
        let tmp = tempfile::Builder::new().tempdir_in("/tmp").unwrap();
        let dir = tmp.path();
        let name = "0123456789abcdef0123456789abcdef01234567";
        let _sock = std::os::unix::net::UnixListener::bind(dir.join(name)).unwrap();
        let _other = std::os::unix::net::UnixListener::bind(dir.join("short")).unwrap();
        std::fs::write(dir.join("fedcba9876543210fedcba9876543210fedcba98"), "x").unwrap();
        assert_eq!(master_sockets(dir), vec![dir.join(name)]);
        assert!(master_sockets(&dir.join("missing")).is_empty());
    }

    #[test]
    fn shutdown_args_ignore_user_config() {
        let args = shutdown_args(Path::new("/Users/u/.terax/cm/abc"));
        assert_eq!(
            args,
            vec![
                "-F",
                "/dev/null",
                "-o",
                "ControlPath=/Users/u/.terax/cm/abc",
                "-O",
                "exit",
                "--",
                "terax"
            ]
        );
    }
}
