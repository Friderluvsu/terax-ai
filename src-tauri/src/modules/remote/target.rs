pub const MAX_ALIAS_LEN: usize = 255;

fn is_alias_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | ':' | '%' | '[' | ']' | '-')
}

pub fn validate_alias(alias: &str) -> Result<(), String> {
    if alias.is_empty() {
        return Err("empty ssh alias".into());
    }
    if alias.len() > MAX_ALIAS_LEN {
        return Err("ssh alias too long".into());
    }
    if alias.starts_with('-') {
        return Err("ssh alias must not start with '-'".into());
    }
    if !alias.chars().all(is_alias_char) {
        return Err("ssh alias contains invalid characters".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn accepts_valid_aliases() {
        for a in [
            "oracle",
            "imac-remote",
            "server_local",
            "user@host.example.com",
            "host:22",
            "[::1]",
            "a.b-c_d%eth0",
            "a-",
        ] {
            assert!(validate_alias(a).is_ok(), "{a}");
        }
    }

    #[test]
    fn rejects_invalid_aliases() {
        let long = "a".repeat(MAX_ALIAS_LEN + 1);
        for a in [
            "",
            "-oProxyCommand=touch /tmp/x",
            "-o",
            "-",
            "--",
            "a b",
            " a",
            "a ",
            "a;b",
            "a|b",
            "a&b",
            "a$(id)",
            "a`id`",
            "a\nb",
            "a\rb",
            "a\tb",
            "a\0b",
            "a'b",
            "a\"b",
            "a\\b",
            "a*",
            "a=b",
            "a/b",
            "\u{e9}",
            long.as_str(),
        ] {
            assert!(validate_alias(a).is_err(), "{a:?}");
        }
        assert!(validate_alias(&"a".repeat(MAX_ALIAS_LEN)).is_ok());
    }

    proptest! {
        #[test]
        fn accepted_aliases_are_safe(s in ".{0,300}") {
            if validate_alias(&s).is_ok() {
                prop_assert!(!s.starts_with('-'));
                prop_assert!(!s.is_empty() && s.len() <= MAX_ALIAS_LEN);
                prop_assert!(s.chars().all(|c| c.is_ascii_graphic()));
                prop_assert!(!s.contains(['\'', '"', '`', '$', ';', '|', '&', '\\', ' ', '\n']));
            }
        }

        #[test]
        fn alphabet_aliases_accepted_unless_leading_dash(s in "[A-Za-z0-9._@:%\\[\\]_-]{1,255}") {
            prop_assert_eq!(validate_alias(&s).is_ok(), !s.starts_with('-'));
        }
    }
}
