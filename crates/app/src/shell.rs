//! Application-owned shell policy; executable lookup stays in the PTY backend.

use std::{ffi::OsString, path::Path};
use terminal_config::ShellConfig;
use terminal_pty::{PortablePtyBackend, PortablePtySession, PtyBackend, PtySize, PtySpawnConfig};

pub(crate) fn configured_spawn_config(shell: &ShellConfig, size: PtySize) -> PtySpawnConfig {
    let config = PtySpawnConfig::new(shell.program.clone().into(), size)
        .with_arguments(shell.args.iter().map(OsString::from));
    if is_powershell(Path::new(&shell.program)) && !has_command_mode(&shell.args) {
        with_powershell_integration(config)
    } else {
        config
    }
}

// Recognize only standard executable basenames, without probing or guessing.
fn is_powershell(program: &Path) -> bool {
    let Some(name) = program.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    #[cfg(windows)]
    {
        ["pwsh", "pwsh.exe", "powershell", "powershell.exe"]
            .iter()
            .any(|candidate| name.eq_ignore_ascii_case(candidate))
    }
    #[cfg(not(windows))]
    {
        matches!(name, "pwsh" | "powershell")
    }
}

// PowerShell accepts abbreviated command/file options. Do not append another
// command to explicit command invocations, including positional script paths.
fn has_command_mode(args: &[String]) -> bool {
    args.iter().any(|arg| {
        let option = arg.to_ascii_lowercase();
        let Some(option) = option.strip_prefix('-') else {
            return option.ends_with(".ps1");
        };
        (!option.is_empty()
            && ["command", "commandwithargs", "encodedcommand", "file"]
                .iter()
                .any(|mode| mode.starts_with(option)))
            || option.ends_with(".ps1")
    })
}

pub(crate) fn with_powershell_integration(config: PtySpawnConfig) -> PtySpawnConfig {
    let mut args = config.arguments().to_vec();
    args.extend([
        OsString::from("-NoExit"),
        OsString::from("-Command"),
        OsString::from(include_str!("powershell_osc7.ps1")),
    ]);
    config.with_arguments(args)
}

pub(crate) fn spawn(config: PtySpawnConfig) -> Result<PortablePtySession, String> {
    let program = config.program().to_owned();
    PortablePtyBackend::new()
        .spawn(config)
        .map_err(|error| format!("could not start local session program {program:?}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(program: &str, args: &[&str]) -> PtySpawnConfig {
        configured_spawn_config(
            &ShellConfig {
                program: program.into(),
                args: args.iter().map(|arg| (*arg).into()).collect(),
            },
            PtySize::new(24, 80).unwrap(),
        )
    }

    #[test]
    fn names_paths_and_exact_arguments_pass_through() {
        for program in [
            "cmd.exe",
            "bash",
            "/usr/bin/fish",
            r"C:\Program Files\Git\bin\bash.exe",
            "wsl.exe",
        ] {
            let empty = request(program, &[]);
            assert_eq!(empty.program(), Path::new(program));
            assert!(empty.arguments().is_empty());
            let config = request(program, &["--option", "two words", "", "'literal'"]);
            assert_eq!(
                config.arguments(),
                &["--option", "two words", "", "'literal'"]
            );
            assert!(config.environment().is_empty());
        }
    }

    #[test]
    fn powershell_detection_is_narrow() {
        for program in ["pwsh", "powershell", "/usr/bin/pwsh"] {
            assert!(is_powershell(Path::new(program)));
        }
        for program in [
            "pwsh-preview",
            "mypowershell",
            "powershell.cmd",
            "bash",
            "cmd.exe",
            "wsl.exe",
            "pwsh.exe.backup",
        ] {
            assert!(!is_powershell(Path::new(program)));
        }
        #[cfg(windows)]
        for program in [
            "PWSH.EXE",
            r"C:\Program Files\PowerShell\7\pwsh.exe",
            "powershell.exe",
        ] {
            assert!(is_powershell(Path::new(program)));
        }
        #[cfg(not(windows))]
        for program in ["PWSH", "pwsh.exe"] {
            assert!(!is_powershell(Path::new(program)));
        }
    }

    #[test]
    fn interactive_powershell_preserves_options_and_adds_prompt_hook() {
        let config = request("pwsh", &["-NoLogo", "-NoProfile"]);
        assert_eq!(
            &config.arguments()[..4],
            &["-NoLogo", "-NoProfile", "-NoExit", "-Command"]
        );
        assert!(
            config.arguments()[4]
                .to_string_lossy()
                .contains("function global:prompt")
        );
        assert_eq!(request("pwsh", &[]).arguments().len(), 3);
    }

    #[test]
    fn explicit_powershell_command_modes_keep_exact_args() {
        for args in [
            vec!["-Command", "echo test"],
            vec!["-c", "echo test"],
            vec!["-File", "script.ps1"],
            vec!["-EncodedCommand", "encoded"],
            vec!["script.ps1"],
        ] {
            assert_eq!(request("pwsh", &args).arguments(), args.as_slice());
        }
    }

    #[test]
    fn missing_configured_executable_reports_program_without_fallback() {
        let program = "terminal-intentionally-missing-shell-9c80d2";
        let error = spawn(request(program, &[]))
            .err()
            .expect("must fail, never fall back");
        assert!(error.contains(program), "{error}");
        assert!(error.contains("PTY spawn failed"), "{error}");
    }
}
