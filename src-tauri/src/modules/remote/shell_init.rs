use std::path::PathBuf;

use super::quote::posix_quote;
use crate::modules::pty::shell_init::FISH_REINSTALL_PROMPT;

const SCRIPTS: [(&str, &str); 6] = [
    ("zsh/.zshenv", include_str!("../pty/scripts/zshenv.zsh")),
    ("zsh/.zprofile", include_str!("../pty/scripts/zprofile.zsh")),
    ("zsh/.zshrc", include_str!("../pty/scripts/zshrc.zsh")),
    ("zsh/.zlogin", include_str!("../pty/scripts/zlogin.zsh")),
    ("bash/bashrc", include_str!("../pty/scripts/bashrc.bash")),
    ("fish/init.fish", include_str!("../pty/scripts/init.fish")),
];

const LAUNCH_MARKER: &str = "if [ -f \"$_tx_dir/ok\" ]; then\n";

#[derive(Debug, Clone)]
pub struct RemotePty {
    pub socket_dir: PathBuf,
    pub integration: bool,
}

pub fn spawn_cwd(cwd: Option<&str>) -> Option<String> {
    let cwd = cwd.filter(|s| !s.trim().is_empty())?;
    if cwd.starts_with('/') && !cwd.contains('\0') {
        Some(cwd.to_string())
    } else {
        log::warn!("remote pty cwd {cwd:?} is not an absolute path; opening home");
        None
    }
}

fn normalized(content: &str) -> String {
    content.replace("\r\n", "\n")
}

pub fn integration_hash() -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for (rel, content) in SCRIPTS {
        for b in rel
            .bytes()
            .chain([0])
            .chain(normalized(content).into_bytes())
            .chain([0])
        {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("{h:016x}")
}

fn launch_section() -> String {
    let fish_init =
        format!("source \"$TERAX_FISH_INIT\"; set -e TERAX_FISH_INIT; {FISH_REINSTALL_PROMPT}");
    [
        LAUNCH_MARKER.to_string(),
        "  case \"${SHELL##*/}\" in\n".to_string(),
        "    zsh)\n".to_string(),
        "      unset TERAX_USER_ZDOTDIR\n".to_string(),
        "      if [ -n \"${ZDOTDIR:-}\" ] && [ \"$ZDOTDIR\" != \"$_tx_dir/zsh\" ]; then export TERAX_USER_ZDOTDIR=\"$ZDOTDIR\"; fi\n".to_string(),
        "      export ZDOTDIR=\"$_tx_dir/zsh\"\n".to_string(),
        "      exec \"$SHELL\" -l ;;\n".to_string(),
        "    bash)\n".to_string(),
        "      exec \"$SHELL\" --rcfile \"$_tx_dir/bash/bashrc\" -i ;;\n".to_string(),
        "    fish)\n".to_string(),
        "      export fish_features=no-mark-prompt TERAX_FISH_INIT=\"$_tx_dir/fish/init.fish\"\n".to_string(),
        format!("      exec \"$SHELL\" -l -i -C {} ;;\n", posix_quote(&fish_init)),
        "  esac\n".to_string(),
        "fi\n".to_string(),
        "exec \"${SHELL:-/bin/sh}\" -l\n".to_string(),
    ]
    .concat()
}

pub fn bootstrap(cwd: Option<&str>, blocks: bool, integration: bool) -> String {
    let mut s = String::new();
    if let Some(cwd) = cwd {
        s.push_str(&format!("cd -- {} 2>/dev/null || cd\n", posix_quote(cwd)));
    }
    if !integration {
        s.push_str("exec \"${SHELL:-/bin/sh}\" -l\n");
        return s;
    }
    s.push_str("export TERAX_TERMINAL=1 COLORTERM=truecolor\n");
    s.push_str(if blocks {
        "export TERAX_BLOCKS=1\n"
    } else {
        "unset TERAX_BLOCKS\n"
    });
    s.push_str("_tx_root=\"${HOME:-}/.cache/terax/shell-integration\"\n");
    s.push_str(&format!("_tx_dir=\"$_tx_root/{}\"\n", integration_hash()));
    s.push_str(
        "if [ ! -f \"$_tx_dir/ok\" ] && _tx_tmp=$(mkdir -p \"$_tx_root\" 2>/dev/null && mktemp -d \"$_tx_root/.tmp.XXXXXX\" 2>/dev/null); then\n",
    );
    s.push_str("  mkdir \"$_tx_tmp/zsh\" \"$_tx_tmp/bash\" \"$_tx_tmp/fish\" &&\n");
    for (rel, content) in SCRIPTS {
        s.push_str(&format!(
            "  printf %s {} > \"$_tx_tmp/{rel}\" &&\n",
            posix_quote(&normalized(content))
        ));
    }
    s.push_str("  : > \"$_tx_tmp/ok\" &&\n");
    s.push_str("  { [ -d \"$_tx_dir\" ] || mv -f \"$_tx_tmp\" \"$_tx_dir\" 2>/dev/null; rm -rf \"$_tx_dir/${_tx_tmp##*/}\"; }\n");
    s.push_str("  rm -rf \"$_tx_tmp\"\n");
    s.push_str("fi\n");
    s.push_str(&launch_section());
    s
}

#[cfg(unix)]
pub fn pty_argv(
    host: &str,
    cwd: Option<&str>,
    blocks: bool,
    remote: &RemotePty,
) -> Result<(PathBuf, Vec<String>), String> {
    let args = ssh_args(&remote.socket_dir, host, cwd, blocks, remote.integration)?;
    Ok((super::control::ssh_binary()?, args))
}

#[cfg(not(unix))]
pub fn pty_argv(
    host: &str,
    cwd: Option<&str>,
    blocks: bool,
    remote: &RemotePty,
) -> Result<(PathBuf, Vec<String>), String> {
    let _ = (host, cwd, blocks, remote);
    Err("SSH workspaces are not supported on this platform".into())
}

#[cfg(unix)]
fn ssh_args(
    socket_dir: &std::path::Path,
    host: &str,
    cwd: Option<&str>,
    blocks: bool,
    integration: bool,
) -> Result<Vec<String>, String> {
    use super::control::{build_args, control_path, Mode};
    let wrapped = super::exec::wrap_script(&bootstrap(cwd, blocks, integration));
    build_args(
        Mode::Interactive,
        &control_path(socket_dir),
        &["-tt"],
        host,
        &[wrapped.as_str()],
    )
}

#[cfg(unix)]
pub fn pty_options(app: &tauri::AppHandle, integration: bool) -> Result<RemotePty, String> {
    use tauri::Manager;
    let state = app.state::<super::RemoteState>();
    let socket_dir = state.socket_dir(&app.config().identifier)?;
    Ok(RemotePty {
        socket_dir,
        integration,
    })
}

#[cfg(not(unix))]
pub fn pty_options(app: &tauri::AppHandle, integration: bool) -> Result<RemotePty, String> {
    let _ = (app, integration);
    Err("SSH workspaces are not supported on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_cwd_accepts_only_absolute_posix_paths() {
        assert_eq!(spawn_cwd(None), None);
        assert_eq!(spawn_cwd(Some("")), None);
        assert_eq!(spawn_cwd(Some("  ")), None);
        assert_eq!(spawn_cwd(Some("~/src")), None);
        assert_eq!(spawn_cwd(Some("src")), None);
        assert_eq!(spawn_cwd(Some("C:\\Users")), None);
        assert_eq!(spawn_cwd(Some("/a\0b")), None);
        assert_eq!(spawn_cwd(Some("/srv/a b'c")).as_deref(), Some("/srv/a b'c"));
        assert_eq!(spawn_cwd(Some("/")).as_deref(), Some("/"));
    }

    #[test]
    fn bootstrap_without_integration_only_changes_dir() {
        assert_eq!(
            bootstrap(Some("/srv/a b'c"), true, false),
            "cd -- '/srv/a b'\\''c' 2>/dev/null || cd\nexec \"${SHELL:-/bin/sh}\" -l\n"
        );
        assert_eq!(
            bootstrap(None, false, false),
            "exec \"${SHELL:-/bin/sh}\" -l\n"
        );
    }

    #[test]
    fn bootstrap_launch_snapshot() {
        let s = bootstrap(Some("/srv/app"), true, true);
        assert!(s.starts_with(
            "cd -- '/srv/app' 2>/dev/null || cd\n\
             export TERAX_TERMINAL=1 COLORTERM=truecolor\n\
             export TERAX_BLOCKS=1\n\
             _tx_root=\"${HOME:-}/.cache/terax/shell-integration\"\n"
        ));
        assert!(s.contains(&format!("_tx_dir=\"$_tx_root/{}\"\n", integration_hash())));
        let tail = &s[s.find(LAUNCH_MARKER).unwrap()..];
        assert_eq!(
            tail,
            "if [ -f \"$_tx_dir/ok\" ]; then\n\
             \x20 case \"${SHELL##*/}\" in\n\
             \x20   zsh)\n\
             \x20     unset TERAX_USER_ZDOTDIR\n\
             \x20     if [ -n \"${ZDOTDIR:-}\" ] && [ \"$ZDOTDIR\" != \"$_tx_dir/zsh\" ]; then export TERAX_USER_ZDOTDIR=\"$ZDOTDIR\"; fi\n\
             \x20     export ZDOTDIR=\"$_tx_dir/zsh\"\n\
             \x20     exec \"$SHELL\" -l ;;\n\
             \x20   bash)\n\
             \x20     exec \"$SHELL\" --rcfile \"$_tx_dir/bash/bashrc\" -i ;;\n\
             \x20   fish)\n\
             \x20     export fish_features=no-mark-prompt TERAX_FISH_INIT=\"$_tx_dir/fish/init.fish\"\n\
             \x20     exec \"$SHELL\" -l -i -C 'source \"$TERAX_FISH_INIT\"; set -e TERAX_FISH_INIT; functions -q __terax_install_prompt; and __terax_install_prompt' ;;\n\
             \x20 esac\n\
             fi\n\
             exec \"${SHELL:-/bin/sh}\" -l\n"
        );
        let classic = bootstrap(None, false, true);
        assert!(classic
            .starts_with("export TERAX_TERMINAL=1 COLORTERM=truecolor\nunset TERAX_BLOCKS\n"));
        assert!(!classic.contains("TERAX_BLOCKS=1"));
    }

    #[test]
    fn rcfile_never_with_login_flag() {
        for blocks in [false, true] {
            for integration in [false, true] {
                for line in bootstrap(Some("/srv"), blocks, integration).lines() {
                    if line.contains("--rcfile") {
                        assert!(
                            !line.split_whitespace().any(|w| w == "-l" || w == "--login"),
                            "{line}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn integration_hash_is_stable_hex() {
        let h = integration_hash();
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h, integration_hash());
    }

    #[cfg(unix)]
    #[test]
    fn pty_argv_snapshot() {
        let args = ssh_args(
            std::path::Path::new("/Users/u/.terax/cm"),
            "oracle",
            Some("/srv/a b'c"),
            true,
            false,
        )
        .unwrap();
        let script = "cd -- '/srv/a b'\\''c' 2>/dev/null || cd\nexec \"${SHELL:-/bin/sh}\" -l\n";
        assert_eq!(
            args,
            vec![
                "-o".to_string(),
                "ControlMaster=auto".into(),
                "-o".into(),
                "ControlPath=/Users/u/.terax/cm/%C".into(),
                "-o".into(),
                "ControlPersist=600".into(),
                "-o".into(),
                "ServerAliveInterval=30".into(),
                "-o".into(),
                "RemoteCommand=none".into(),
                "-o".into(),
                "ForwardAgent=no".into(),
                "-tt".into(),
                "--".into(),
                "oracle".into(),
                super::super::exec::wrap_script(script),
            ]
        );
        assert!(ssh_args(
            std::path::Path::new("/tmp/cm"),
            "-oProxyCommand=x",
            None,
            false,
            true
        )
        .is_err());
    }

    #[cfg(unix)]
    #[cfg(unix)]
    #[test]
    fn bootstrap_install_leaves_no_nested_tmp_when_dir_exists() {
        let home = tempfile::tempdir().unwrap();
        let dir = home
            .path()
            .join(".cache/terax/shell-integration")
            .join(integration_hash());
        std::fs::create_dir_all(&dir).unwrap();
        let script = bootstrap(None, false, true).replace(&launch_section(), "");
        for _ in 0..2 {
            let st = std::process::Command::new("sh")
                .arg("-c")
                .arg(&script)
                .env("HOME", home.path())
                .status()
                .unwrap();
            assert!(st.success());
        }
        let nested: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".tmp."))
            .collect();
        assert!(nested.is_empty());
        let root = dir.parent().unwrap();
        let stray = std::fs::read_dir(root)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(".tmp."))
            .count();
        assert_eq!(stray, 0);
    }

    mod exec_tests {
        use super::super::*;
        use std::path::{Path, PathBuf};
        use std::process::Command;

        const FAKE_SHELL: &str = "#!/bin/sh\n\
            printf 'argv0=%s\\n' \"${0##*/}\"\n\
            for a in \"$@\"; do printf 'arg=%s\\n' \"$a\"; done\n\
            printf 'pwd=%s\\n' \"$(pwd -P)\"\n\
            for v in TERAX_TERMINAL COLORTERM TERAX_BLOCKS ZDOTDIR TERAX_USER_ZDOTDIR fish_features TERAX_FISH_INIT; do\n\
              eval \"printf '%s=%s\\\\n' $v \\\"\\${$v-<unset>}\\\"\"\n\
            done\n";

        struct Env {
            _tmp: tempfile::TempDir,
            home: PathBuf,
            bin: PathBuf,
        }

        fn env() -> Env {
            use std::os::unix::fs::PermissionsExt;
            let tmp = tempfile::tempdir().unwrap();
            let root = std::fs::canonicalize(tmp.path()).unwrap();
            let home = root.join("home");
            let bin = root.join("bin");
            std::fs::create_dir_all(&home).unwrap();
            std::fs::create_dir_all(&bin).unwrap();
            for name in ["zsh", "bash", "fish", "tcsh"] {
                let p = bin.join(name);
                std::fs::write(&p, FAKE_SHELL).unwrap();
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            Env {
                _tmp: tmp,
                home,
                bin,
            }
        }

        fn run(env: &Env, home: &Path, shell: &str, script: &str) -> String {
            let out = Command::new("/bin/sh")
                .arg("-c")
                .arg(script)
                .env_clear()
                .env("HOME", home)
                .env("SHELL", env.bin.join(shell))
                .env("PATH", "/usr/bin:/bin")
                .current_dir("/")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).unwrap()
        }

        fn vars(blocks: bool, zdotdir: &str, fish: (&str, &str)) -> String {
            format!(
                "TERAX_TERMINAL=1\nCOLORTERM=truecolor\nTERAX_BLOCKS={}\nZDOTDIR={zdotdir}\n\
                 TERAX_USER_ZDOTDIR=<unset>\nfish_features={}\nTERAX_FISH_INIT={}\n",
                if blocks { "1" } else { "<unset>" },
                fish.0,
                fish.1
            )
        }

        const PLAIN_VARS: &str = "TERAX_TERMINAL=<unset>\nCOLORTERM=<unset>\nTERAX_BLOCKS=<unset>\n\
            ZDOTDIR=<unset>\nTERAX_USER_ZDOTDIR=<unset>\nfish_features=<unset>\nTERAX_FISH_INIT=<unset>\n";

        #[test]
        fn launches_each_shell_with_integration() {
            let e = env();
            let cwd = e.home.join("a b'c $(x)");
            std::fs::create_dir(&cwd).unwrap();
            let cwd_s = cwd.to_str().unwrap();
            let dir = format!(
                "{}/.cache/terax/shell-integration/{}",
                e.home.display(),
                integration_hash()
            );
            let script = bootstrap(Some(cwd_s), true, true);

            assert_eq!(
                run(&e, &e.home, "zsh", &script),
                format!(
                    "argv0=zsh\narg=-l\npwd={cwd_s}\n{}",
                    vars(true, &format!("{dir}/zsh"), ("<unset>", "<unset>"))
                )
            );
            assert_eq!(
                run(&e, &e.home, "bash", &script),
                format!(
                    "argv0=bash\narg=--rcfile\narg={dir}/bash/bashrc\narg=-i\npwd={cwd_s}\n{}",
                    vars(true, "<unset>", ("<unset>", "<unset>"))
                )
            );
            assert_eq!(
                run(&e, &e.home, "fish", &script),
                format!(
                    "argv0=fish\narg=-l\narg=-i\narg=-C\narg=source \"$TERAX_FISH_INIT\"; set -e TERAX_FISH_INIT; {FISH_REINSTALL_PROMPT}\npwd={cwd_s}\n{}",
                    vars(
                        true,
                        "<unset>",
                        ("no-mark-prompt", &format!("{dir}/fish/init.fish"))
                    )
                )
            );
            assert_eq!(
                run(&e, &e.home, "tcsh", &script),
                format!(
                    "argv0=tcsh\narg=-l\npwd={cwd_s}\n{}",
                    vars(true, "<unset>", ("<unset>", "<unset>"))
                )
            );

            for (rel, content) in SCRIPTS {
                let written = std::fs::read_to_string(format!("{dir}/{rel}")).unwrap();
                assert_eq!(written, normalized(content), "{rel}");
            }
            let leftovers: Vec<_> =
                std::fs::read_dir(e.home.join(".cache/terax/shell-integration"))
                    .unwrap()
                    .flatten()
                    .map(|d| d.file_name())
                    .collect();
            assert_eq!(leftovers.len(), 1, "{leftovers:?}");
        }

        #[test]
        fn launches_each_shell_without_integration() {
            let e = env();
            let cwd = e.home.join("x y'z");
            std::fs::create_dir(&cwd).unwrap();
            let cwd_s = cwd.to_str().unwrap();
            let script = bootstrap(Some(cwd_s), true, false);
            for shell in ["zsh", "bash", "fish", "tcsh"] {
                assert_eq!(
                    run(&e, &e.home, shell, &script),
                    format!("argv0={shell}\narg=-l\npwd={cwd_s}\n{PLAIN_VARS}")
                );
            }
            assert!(!e.home.join(".cache").exists());
        }

        #[test]
        fn classic_tabs_do_not_export_blocks() {
            let e = env();
            let script = bootstrap(None, false, true);
            let out = run(&e, &e.home, "zsh", &script);
            assert!(out.contains("TERAX_BLOCKS=<unset>\n"), "{out}");
            assert!(out.contains("TERAX_TERMINAL=1\n"), "{out}");
        }

        #[test]
        fn missing_cwd_falls_back_to_home() {
            let e = env();
            let script = bootstrap(Some("/definitely/missing dir"), false, false);
            let out = run(&e, &e.home, "bash", &script);
            assert!(
                out.contains(&format!("pwd={}\n", e.home.display())),
                "{out}"
            );
        }

        #[test]
        fn existing_scripts_are_not_rewritten() {
            let e = env();
            let script = bootstrap(None, false, true);
            run(&e, &e.home, "zsh", &script);
            let dir = e
                .home
                .join(".cache/terax/shell-integration")
                .join(integration_hash());
            std::fs::write(dir.join("zsh/.zshrc"), "kept").unwrap();
            run(&e, &e.home, "bash", &script);
            assert_eq!(
                std::fs::read_to_string(dir.join("zsh/.zshrc")).unwrap(),
                "kept"
            );
        }

        #[test]
        fn unwritable_home_falls_back_to_plain_login_shell() {
            let e = env();
            let file_home = e.home.join("not-a-dir");
            std::fs::write(&file_home, "x").unwrap();
            let script = bootstrap(None, true, true);
            for shell in ["zsh", "bash", "fish"] {
                let out = run(&e, &file_home, shell, &script);
                let mut lines = out.lines();
                assert_eq!(lines.next(), Some(format!("argv0={shell}").as_str()));
                assert_eq!(lines.next(), Some("arg=-l"));
                assert!(lines.next().unwrap().starts_with("pwd="));
                assert!(!out.contains("--rcfile"));
            }
        }

        #[test]
        fn user_zdotdir_is_preserved() {
            let e = env();
            let script = bootstrap(None, false, true);
            let out = Command::new("/bin/sh")
                .arg("-c")
                .arg(&script)
                .env_clear()
                .env("HOME", &e.home)
                .env("SHELL", e.bin.join("zsh"))
                .env("ZDOTDIR", "/etc/zdot")
                .env("PATH", "/usr/bin:/bin")
                .output()
                .unwrap();
            let out = String::from_utf8(out.stdout).unwrap();
            assert!(out.contains("TERAX_USER_ZDOTDIR=/etc/zdot\n"), "{out}");
        }

        #[test]
        fn wrapped_bootstrap_runs_through_sh() {
            let e = env();
            let cwd = e.home.join("q'uote d");
            std::fs::create_dir(&cwd).unwrap();
            let script = bootstrap(Some(cwd.to_str().unwrap()), true, true);
            let wrapped = super::super::super::exec::wrap_script(&script);
            let out = run(&e, &e.home, "bash", &wrapped);
            assert!(out.starts_with("argv0=bash\narg=--rcfile\n"), "{out}");
            assert!(out.contains(&format!("pwd={}\n", cwd.display())), "{out}");
            assert!(!out.lines().any(|l| l == "arg=-l"), "{out}");
        }

        #[test]
        #[ignore = "needs TERAX_SSH_TEST_HOST"]
        fn remote_bootstrap_writes_scripts() {
            let Ok(host) = std::env::var("TERAX_SSH_TEST_HOST") else {
                return;
            };
            let tmp = tempfile::Builder::new()
                .prefix("tx")
                .tempdir_in("/tmp")
                .unwrap();
            let dir = tmp.path().join("cm");
            crate::modules::remote::control::ensure_socket_dir(&dir).unwrap();
            let script = bootstrap(None, false, true)
                .replace(&launch_section(), "test -f \"$_tx_dir/ok\" && printf ok\n");
            let out = crate::modules::remote::exec::exec(
                &dir,
                &host,
                &script,
                std::time::Duration::from_secs(30),
                4096,
            )
            .unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout), "ok");
        }
    }
}
