//! Local OSC 7 launch policy. Never called while consuming PTY output.

use std::path::PathBuf;

pub(crate) fn local_cwd_from_osc7(uri: &str) -> Option<PathBuf> {
    decode_local_uri(uri, cfg!(windows), |host| {
        terminal_platform::local_hostnames()
            .iter()
            .any(|local| host.eq_ignore_ascii_case(local))
    })
    .map(PathBuf::from)
}

/// Explicit roots retain their existing validation/error behavior. Only a
/// best-effort inherited directory is checked here and allowed to fall back.
pub(crate) fn launch_root(
    explicit: Option<PathBuf>,
    inherited: Option<&PathBuf>,
) -> Option<PathBuf> {
    explicit.or_else(|| inherited.filter(|path| path.is_dir()).cloned())
}

fn decode_local_uri(
    uri: &str,
    windows: bool,
    is_local_host: impl FnOnce(&str) -> bool,
) -> Option<String> {
    if !uri.get(..7)?.eq_ignore_ascii_case("file://") {
        return None;
    }
    let (host, path) = uri[7..].split_once('/')?;
    if !host
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
        || !(host.is_empty() || host.eq_ignore_ascii_case("localhost") || is_local_host(host))
    {
        return None;
    }
    let mut decoded = vec![b'/'];
    let mut bytes = path.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = (bytes.next()? as char).to_digit(16)?;
            let low = (bytes.next()? as char).to_digit(16)?;
            decoded.push((high * 16 + low) as u8);
        } else if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
                    | b'/'
            )
        {
            decoded.push(byte);
        } else {
            return None;
        }
    }
    let path = String::from_utf8(decoded).ok()?;
    if path.chars().any(char::is_control) || path.starts_with("//") {
        return None;
    }
    let drive = path.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic)
        && path.as_bytes().get(2) == Some(&b':');
    if windows {
        // Require a fully qualified drive path, never UNC, device namespaces,
        // drive-relative paths, or paths rooted on an unspecified current drive.
        if !drive || path.as_bytes().get(3) != Some(&b'/') {
            return None;
        }
        if path[3..].contains(['\\', ':', '?', '*', '"', '<', '>', '|']) {
            return None;
        }
        Some(path[1..].to_owned())
    } else {
        // A Windows report must not be mistaken for a local POSIX path.
        (!drive).then_some(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(uri: &str, windows: bool) -> Option<String> {
        decode_local_uri(uri, windows, |host| {
            host.eq_ignore_ascii_case("my-machine.local")
        })
    }

    #[test]
    fn local_authority_and_posix_paths() {
        for host in ["", "localhost", "LOCALHOST", "MY-machine.local"] {
            assert_eq!(
                decode(
                    &format!("FiLe://{host}/tmp/two%20words/%C3%A9%23%3F"),
                    false
                ),
                Some("/tmp/two words/é#?".into())
            );
        }
        assert_eq!(decode("file:///", false), Some("/".into()));
        assert_eq!(
            decode("file:///Users/me/project", false),
            Some("/Users/me/project".into())
        );
        for uri in [
            "file://ssh-host/tmp",
            "file://my-machine/tmp",
            "file://localhost.evil/tmp",
            "file://user@localhost/tmp",
            "file://localhost:22/tmp",
            "file://127.0.0.1/tmp",
            "file://[::1]/tmp",
            "file://local%68ost/tmp",
            "file:/tmp",
            "https:///tmp",
            "file:///tmp?query",
            "file:///tmp#fragment",
            "file:///tmp%",
            "file:///tmp%GG",
            "file:///tmp%FF",
            "file:///tmp%00",
            "file:///tmp%0A",
            "file:////server/share",
            "file:///%2Fserver/share",
            "file:///C:/project",
            "file:///c:relative",
        ] {
            assert_eq!(decode(uri, false), None, "{uri}");
        }
    }

    #[test]
    fn native_machine_names_are_accepted_without_environment_aliases() {
        let path = if cfg!(windows) {
            "/C:/project"
        } else {
            "/tmp/project"
        };
        let expected = local_cwd_from_osc7(&format!("file://{path}"));
        assert!(expected.is_some());
        for host in terminal_platform::local_hostnames() {
            assert_eq!(
                local_cwd_from_osc7(&format!("file://{host}{path}")),
                expected
            );
        }
        assert_eq!(
            local_cwd_from_osc7(&format!("file://remote.invalid{path}")),
            None
        );
    }

    #[test]
    fn windows_requires_local_fully_qualified_drive_paths() {
        assert_eq!(
            decode("file:///C:/two%20words/%C3%A9", true),
            Some("C:/two words/é".into())
        );
        assert_eq!(decode("file://localhost/c:/", true), Some("c:/".into()));
        for uri in [
            "file:///C:",
            "file:///C:relative",
            "file:///tmp",
            "file:///1:/tmp",
            "file://server/C:/tmp",
            "file://localhost/share/tmp",
            "file:////server/share",
            "file:///%5C%5Cserver/share",
            "file:///%5C%5C%3F/C:/tmp",
            "file:///C:/a%5Cb",
            "file:///C:/a%3Astream",
            "file:///C:/a%3Fb",
            "file:///C:/a%00",
        ] {
            assert_eq!(decode(uri, true), None, "{uri}");
        }
    }
}
