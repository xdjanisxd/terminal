//! Linux local-shell startup policy. No user files are modified.
use std::{
    ffi::OsString,
    fs, io,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use terminal_pty::PtySpawnConfig;

pub(super) struct IntegrationFiles(PathBuf);
impl Drop for IntegrationFiles {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
impl IntegrationFiles {
    fn new() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "terminal-shell-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }
    fn write(&self, name: &str, text: &str) -> io::Result<()> {
        fs::write(self.0.join(name), text)
    }
}

struct Options {
    login: bool,
    no_rc: bool,
    rcfile: Option<OsString>,
}

// A narrow allowlist prevents hooks/input in scripts, -c/-s invocations,
// restricted/POSIX modes, and options whose operands we cannot interpret.
fn interactive_options(config: &PtySpawnConfig) -> Option<Options> {
    let bash = config.program().file_name().is_some_and(|s| s == "bash");
    let zsh = config.program().file_name().is_some_and(|s| s == "zsh");
    let mut login = false;

    let mut no_rc = false;
    let mut rcfile = None;
    let mut args = config.arguments().iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--login") if bash => login = true,
            Some("--noprofile") if bash => {}
            Some("--norc") if bash => no_rc = true,
            Some("--rcfile" | "--init-file") if bash => {
                let path = args.next()?;
                rcfile = Some(path.clone());
            }
            Some(flags)
                if flags.starts_with('-')
                    && !flags.starts_with("--")
                    && flags.len() > 1
                    && flags[1..]
                        .chars()
                        .all(|c| matches!(c, 'i' | 'l') || (zsh && c == 'f')) =>
            {
                login |= flags.contains('l');
                no_rc |= flags.contains('f');
            }
            _ => return None,
        }
    }
    Some(Options {
        login,
        no_rc,
        rcfile,
    })
}

pub(super) fn startup_command_allowed(config: &PtySpawnConfig) -> bool {
    !matches!(
        config.program().file_name().and_then(|s| s.to_str()),
        Some("bash" | "zsh")
    ) || interactive_options(config).is_some()
}

pub(super) fn prepare(
    config: PtySpawnConfig,
) -> io::Result<(PtySpawnConfig, Option<IntegrationFiles>)> {
    let marked = config
        .environment()
        .iter()
        .any(|(key, value)| key == "TERMINAL_SHELL_INTEGRATION" && value == "1");
    if !marked {
        return Ok((config, None));
    }
    let mut env = config.environment().to_vec();
    env.retain(|(key, _)| key != "TERMINAL_SHELL_INTEGRATION");
    let bash = config
        .program()
        .file_name()
        .is_some_and(|name| name == "bash");
    let zsh = config
        .program()
        .file_name()
        .is_some_and(|name| name == "zsh");
    if !bash && !zsh {
        return Ok((config.with_environment(env), None));
    }
    let Some(options) = interactive_options(&config) else {
        return Ok((config.with_environment(env), None));
    };
    let Options {
        login,
        no_rc,
        rcfile,
    } = options;
    // Bash does not offer an after-profile rc hook for login shells. Keep
    // real login semantics rather than emulate profiles in a non-login shell.
    if bash && login {
        return Ok((config.with_environment(env), None));
    }
    let files = IntegrationFiles::new()?;
    env.retain(|(key, _)| !key.to_string_lossy().starts_with("__TERMINAL_"));
    if let Some(root) = config.working_directory() {
        env.push(("__TERMINAL_ROOT".into(), root.as_os_str().to_owned()));
    }
    if bash {
        files.write("bashrc", include_str!("linux_bashrc.sh"))?;
        env.push((
            "__TERMINAL_NO_RC".into(),
            if no_rc { "1" } else { "0" }.into(),
        ));
        if let Some(path) = rcfile {
            env.push(("__TERMINAL_RCFILE".into(), path));
        }
        let args = if no_rc {
            let previous = env
                .iter()
                .rev()
                .find(|(k, _)| k == "PROMPT_COMMAND")
                .map(|(_, v)| v.clone())
                .or_else(|| std::env::var_os("PROMPT_COMMAND"));
            env.retain(|(k, _)| k != "PROMPT_COMMAND");
            if let Some(previous) = previous {
                env.push(("__TERMINAL_PRIOR_PROMPT_COMMAND".into(), previous));
            }
            env.push((
                "__TERMINAL_BASHRC".into(),
                files.0.join("bashrc").into_os_string(),
            ));
            env.push((
                "PROMPT_COMMAND".into(),
                "builtin source \"$__TERMINAL_BASHRC\"".into(),
            ));
            config.arguments().to_vec()
        } else {
            vec![
                "--noprofile".into(),
                "--rcfile".into(),
                files.0.join("bashrc").into_os_string(),
                "-i".into(),
            ]
        };
        Ok((
            config.with_arguments(args).with_environment(env),
            Some(files),
        ))
    } else {
        let original = env
            .iter()
            .rev()
            .find(|(k, _)| k == "ZDOTDIR")
            .map(|(_, v)| v.clone())
            .or_else(|| std::env::var_os("ZDOTDIR"));
        if let Some(original) = original {
            env.push(("__TERMINAL_ZDOTDIR".into(), original));
        }
        env.retain(|(key, _)| key != "ZDOTDIR");
        env.push(("ZDOTDIR".into(), files.0.as_os_str().to_owned()));
        env.push((
            "__TERMINAL_INTEGRATION_DIR".into(),
            files.0.as_os_str().to_owned(),
        ));
        env.push((
            "__TERMINAL_NO_RC".into(),
            if no_rc { "1" } else { "0" }.into(),
        ));
        files.write(".zshenv", include_str!("linux_zshenv.zsh"))?;
        files.write("hook.zsh", include_str!("linux_zsh_hook.zsh"))?;
        for name in [".zprofile", ".zshrc", ".zlogin"] {
            files.write(name, &format!("__terminal_source_user {name}\nprecmd_functions=(${{precmd_functions:#__terminal_report_prompt}} __terminal_report_prompt)\n"))?;
        }
        let args: Vec<OsString> = if login {
            vec!["-il".into()]
        } else {
            vec!["-i".into()]
        };
        Ok((
            config.with_arguments(args).with_environment(env),
            Some(files),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_pty::PtySize;

    fn request(program: &str, args: &[&str]) -> PtySpawnConfig {
        super::super::with_posix_integration(
            PtySpawnConfig::new(program.into(), PtySize::new(24, 80).unwrap())
                .with_arguments(args.iter().map(OsString::from))
                .with_environment([("KEEP".into(), "two words".into())]),
        )
    }

    #[test]
    fn scripts_command_modes_login_bash_and_unknown_options_are_unchanged() {
        for program in ["bash", "zsh"] {
            for args in [
                vec!["-c", "printf done"],
                vec!["-ic", "printf done"],
                vec!["script.sh"],
                vec!["-s"],
                vec!["--", "script.sh"],
                vec!["--unknown"],
                vec!["-r"],
            ] {
                let config = request(program, &args);
                let (prepared, files) = prepare(config.clone()).unwrap();
                assert!(files.is_none());
                assert!(!startup_command_allowed(&prepared));
                assert_eq!(prepared.arguments(), config.arguments());
                assert_eq!(
                    prepared.environment(),
                    &[("KEEP".into(), "two words".into())]
                );
            }
        }
        for args in [vec!["-l"], vec!["--login"], vec!["-il"]] {
            let config = request("bash", &args);
            let (prepared, files) = prepare(config.clone()).unwrap();
            assert!(files.is_none());
            assert_eq!(prepared.arguments(), config.arguments());
        }
        // A direct command request has no local-shell marker.
        let raw = PtySpawnConfig::new("bash".into(), PtySize::new(24, 80).unwrap());
        assert_eq!(prepare(raw.clone()).unwrap().0, raw);
    }

    #[test]
    fn rcfile_environment_cwd_and_private_file_lifetime_are_preserved() {
        use std::os::unix::fs::PermissionsExt;
        for program in ["bash", "zsh"] {
            let config = request(program, &[]).with_working_directory("/tmp/two words".into());
            let (prepared, files) = prepare(config).unwrap();
            let files = files.unwrap();
            let path = files.0.clone();
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                prepared.working_directory(),
                Some(std::path::Path::new("/tmp/two words"))
            );
            assert!(
                prepared
                    .environment()
                    .contains(&("KEEP".into(), "two words".into()))
            );
            assert!(
                prepared
                    .environment()
                    .contains(&("__TERMINAL_ROOT".into(), "/tmp/two words".into()))
            );
            drop(files);
            assert!(!path.exists());
        }
        let (config, _files) = prepare(request("bash", &["--rcfile", "/tmp/user rc"])).unwrap();
        assert!(
            config
                .environment()
                .contains(&("__TERMINAL_RCFILE".into(), "/tmp/user rc".into()))
        );
    }
}
