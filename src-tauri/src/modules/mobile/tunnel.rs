use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use crate::modules::lsp;

const QUICK_TUNNEL_SUFFIX: &str = ".trycloudflare.com";

pub fn find_cloudflared() -> Option<PathBuf> {
    lsp::env::resolve_binary("cloudflared").or_else(|| {
        [
            "/opt/homebrew/bin/cloudflared",
            "/usr/local/bin/cloudflared",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
    })
}

pub fn spawn(
    binary: &PathBuf,
    port: u16,
    mut on_line: impl FnMut(&str) + Send + 'static,
    on_exit: impl FnOnce() + Send + 'static,
) -> std::io::Result<Child> {
    let mut child = Command::new(binary)
        .args([
            "tunnel",
            "--no-autoupdate",
            "--url",
            &format!("http://127.0.0.1:{port}"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child.stderr.take().expect("piped stderr");
    std::thread::Builder::new()
        .name("terax-mobile-tunnel".into())
        .stack_size(128 * 1024)
        .spawn(move || {
            for line in BufReader::new(stderr).lines() {
                match line {
                    Ok(line) => on_line(&line),
                    Err(_) => break,
                }
            }
            on_exit();
        })?;
    Ok(child)
}

/// Extracts the assigned quick tunnel URL from a cloudflared log line.
pub fn parse_quick_tunnel_url(line: &str) -> Option<String> {
    line.match_indices("https://").find_map(|(start, scheme)| {
        let rest = &line[start + scheme.len()..];
        let host_len = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '.'))
            .unwrap_or(rest.len());
        let host = &rest[..host_len];
        let label = host.strip_suffix(QUICK_TUNNEL_SUFFIX)?;
        let valid = !label.is_empty()
            && label != "api"
            && label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        valid.then(|| format!("https://{host}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_quick_tunnel_url_from_banner() {
        let line = "2026-11-02T10:00:00Z INF |  https://brave-cat-sun-moon.trycloudflare.com                    |";
        assert_eq!(
            parse_quick_tunnel_url(line).as_deref(),
            Some("https://brave-cat-sun-moon.trycloudflare.com")
        );
    }

    #[test]
    fn ignores_other_hosts_and_api_endpoint() {
        assert_eq!(
            parse_quick_tunnel_url("see https://developers.cloudflare.com/x"),
            None
        );
        assert_eq!(
            parse_quick_tunnel_url("https://api.trycloudflare.com/tunnel"),
            None
        );
        assert_eq!(
            parse_quick_tunnel_url("https://evil.trycloudflare.com.attacker.io"),
            None
        );
        assert_eq!(
            parse_quick_tunnel_url("https://a.b.trycloudflare.com"),
            None
        );
        assert_eq!(parse_quick_tunnel_url("no url here"), None);
        assert_eq!(
            parse_quick_tunnel_url(
                "INF Thank you for trying Cloudflare Tunnel (https://www.cloudflare.com/website-terms/) \
                 then https://developers.cloudflare.com/cloudflare-one/connections/connect-apps"
            ),
            None
        );
        assert_eq!(
            parse_quick_tunnel_url(
                "terms https://www.cloudflare.com/x then https://ok-1.trycloudflare.com |"
            )
            .as_deref(),
            Some("https://ok-1.trycloudflare.com")
        );
    }
}
