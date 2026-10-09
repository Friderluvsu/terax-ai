use super::control::{build_args, control_path, ssh_binary, Mode};
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const MAX_STDERR: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawOutput {
    pub status: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub truncated: bool,
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub fn wrap_script(script: &str) -> String {
    let b64 = base64_encode(script.as_bytes());
    format!(
        "sh -c 'eval \"$(printf %s {b64} | base64 -d 2>/dev/null || printf %s {b64} | base64 -D)\"'"
    )
}

fn read_capped(mut r: impl Read, cap: usize) -> (Vec<u8>, bool) {
    let mut kept = Vec::new();
    let mut truncated = false;
    let mut buf = [0u8; 8192];
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let room = cap.saturating_sub(kept.len());
                if n > room {
                    truncated = true;
                }
                kept.extend_from_slice(&buf[..n.min(room)]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    (kept, truncated)
}

fn kill_group(pid: u32) {
    if let Ok(pid) = i32::try_from(pid) {
        // SAFETY: killpg on the group this function's caller created.
        unsafe {
            libc::killpg(pid, libc::SIGKILL);
        }
    }
}

pub fn run(
    program: &Path,
    args: &[String],
    timeout: Duration,
    max_output: usize,
) -> Result<RawOutput, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", program.display()))?;
    let pid = child.id();
    let deadline = Instant::now() + timeout;
    let (tx, rx) = mpsc::channel();
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send((true, read_capped(out, max_output)));
        });
    }
    if let Some(err) = child.stderr.take() {
        std::thread::spawn(move || {
            let _ = tx.send((false, read_capped(err, MAX_STDERR)));
        });
    } else {
        drop(tx);
    }
    let timed_out = |child: &mut std::process::Child| {
        kill_group(pid);
        let _ = child.wait();
        Err("ssh command timed out".to_string())
    };

    let mut stdout = None;
    let mut stderr = None;
    while stdout.is_none() || stderr.is_none() {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok((true, v)) => stdout = Some(v),
            Ok((false, v)) => stderr = Some(v),
            Err(mpsc::RecvTimeoutError::Timeout) => return timed_out(&mut child),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => return timed_out(&mut child),
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                kill_group(pid);
                let _ = child.wait();
                return Err(format!("wait for ssh: {e}"));
            }
        }
    };
    let (stdout, out_trunc) = stdout.unwrap_or_default();
    let (stderr, _) = stderr.unwrap_or_default();
    Ok(RawOutput {
        status: status.code(),
        stdout,
        stderr,
        truncated: out_trunc,
    })
}

pub fn exec(
    socket_dir: &Path,
    host: &str,
    script: &str,
    timeout: Duration,
    max_output: usize,
) -> Result<RawOutput, String> {
    let wrapped = wrap_script(script);
    let args = build_args(
        Mode::Batch,
        &control_path(socket_dir),
        &[],
        host,
        &[wrapped.as_str()],
    )?;
    run(&ssh_binary()?, &args, timeout, max_output)
}

#[cfg(test)]
mod tests {
    use super::super::quote::posix_quote;
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn base64_vectors() {
        for (input, want) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(input.as_bytes()), want);
        }
        assert_eq!(base64_encode(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn wrapper_uses_only_safe_characters() {
        let w = wrap_script("printf '%s' \"$HOME\"; echo `id` \\ \n'\"");
        let b64 = base64_encode("printf '%s' \"$HOME\"; echo `id` \\ \n'\"".as_bytes());
        assert_eq!(
            w,
            format!(
                "sh -c 'eval \"$(printf %s {b64} | base64 -d 2>/dev/null || printf %s {b64} | base64 -D)\"'"
            )
        );
        assert!(b64
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')));
        assert!(!w.contains('\\'));
        assert!(!w.contains('!'));
        assert_eq!(w.matches('\'').count(), 2);
    }

    fn shells() -> Vec<PathBuf> {
        ["sh", "bash", "zsh", "dash", "fish", "tcsh"]
            .iter()
            .filter_map(|s| which::which(s).ok())
            .collect()
    }

    #[test]
    fn wrapper_survives_login_shells() {
        let args = ["a\\", "'", "\\\\", "$(x)", "line1\nline2", "a b", "\"q\""];
        let script = format!(
            "for a in {}; do printf '[%s]' \"$a\"; done",
            args.iter()
                .map(|a| posix_quote(a))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let want: String = args.iter().map(|a| format!("[{a}]")).collect();
        let wrapped = wrap_script(&script);
        let shells = shells();
        assert!(!shells.is_empty());
        for shell in shells {
            let out = run(
                &shell,
                &["-c".to_string(), wrapped.clone()],
                Duration::from_secs(10),
                1 << 16,
            )
            .unwrap();
            assert_eq!(out.status, Some(0), "{}", shell.display());
            assert_eq!(
                String::from_utf8_lossy(&out.stdout),
                want,
                "{}",
                shell.display()
            );
        }
    }

    #[test]
    fn run_caps_output_and_reports_status() {
        let sh = PathBuf::from("/bin/sh");
        let out = run(
            &sh,
            &[
                "-c".into(),
                "printf 0123456789; printf err >&2; exit 3".into(),
            ],
            Duration::from_secs(10),
            4,
        )
        .unwrap();
        assert_eq!(out.status, Some(3));
        assert_eq!(out.stdout, b"0123");
        assert!(out.truncated);
        assert_eq!(out.stderr, b"err");
    }

    #[test]
    fn run_times_out_and_kills_group() {
        let tmp = tempfile::tempdir().unwrap();
        let marker = tmp.path().join("survived");
        let script = format!(
            "(sleep 1; touch {}) & sleep 30",
            posix_quote(marker.to_str().unwrap())
        );
        let start = Instant::now();
        let err = run(
            Path::new("/bin/sh"),
            &["-c".into(), script],
            Duration::from_millis(200),
            1024,
        )
        .unwrap_err();
        assert!(err.contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(1500));
        assert!(!marker.exists());
    }
}
