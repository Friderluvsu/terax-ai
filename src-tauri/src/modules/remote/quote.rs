pub fn posix_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use proptest::prelude::*;

    #[test]
    fn quotes_basic_values() {
        assert_eq!(posix_quote(""), "''");
        assert_eq!(posix_quote("abc"), "'abc'");
        assert_eq!(posix_quote("a'b"), "'a'\\''b'");
        assert_eq!(posix_quote("$(x) `y` \\ \n"), "'$(x) `y` \\ \n'");
    }

    #[cfg(unix)]
    fn sh_printf(arg: &str) -> String {
        let script = format!("printf %s {}", posix_quote(arg));
        let out = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .output()
            .expect("run sh");
        assert!(out.status.success());
        String::from_utf8(out.stdout).expect("utf8")
    }

    #[cfg(unix)]
    #[test]
    fn round_trips_through_sh() {
        for s in [
            "",
            "plain",
            "'",
            "''",
            "a'b'c",
            "a\\",
            "\\\\",
            "$(touch /tmp/x)",
            "`id`",
            "$HOME",
            "a\nb",
            " lead and trail ",
            "*?[]{}~!#&;|<>",
            "-n",
            "%s%d",
            "\u{e9}\u{4e2d}",
        ] {
            assert_eq!(sh_printf(s), s, "{s:?}");
        }
    }

    #[cfg(unix)]
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn round_trips_arbitrary(s in "[^\u{0}]{0,40}") {
            prop_assert_eq!(sh_printf(&s), s);
        }
    }
}
