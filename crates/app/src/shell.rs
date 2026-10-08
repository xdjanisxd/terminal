//! Application-owned shell policy; executable lookup stays in the PTY backend.

use std::{ffi::OsString, path::Path};
use terminal_config::ShellConfig;
use terminal_pty::{
    PortablePtyBackend, PortablePtySession, PtySize, PtySpawnConfig, PtyStartupObserver,
};

pub(crate) fn configured_spawn_config(shell: &ShellConfig, size: PtySize) -> PtySpawnConfig {
    let config = PtySpawnConfig::new(shell.program.clone().into(), size)
        .with_arguments(shell.args.iter().map(OsString::from))
        .with_environment(
            shell
                .env
                .iter()
                .map(|(key, value)| (key.into(), value.into())),
        );
    if is_powershell(Path::new(&shell.program)) && !has_command_mode(&shell.args) {
        with_powershell_integration(config)
    } else {
        with_cmd_integration(config)
    }
}

pub(crate) fn with_cmd_integration(config: PtySpawnConfig) -> PtySpawnConfig {
    // Only cmd local shells need this hook. Explicit /c invocations never
    // reach an interactive prompt; their arguments must remain untouched.
    let cmd = config.program().file_name().and_then(|name| name.to_str());
    if !cfg!(windows)
        || !cmd.is_some_and(|name| {
            name.eq_ignore_ascii_case("cmd.exe") || name.eq_ignore_ascii_case("cmd")
        })
        || config.arguments().iter().any(|arg| {
            arg.to_str().is_some_and(|arg| {
                arg.get(..2)
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("/c"))
            })
        })
    {
        return config;
    }
    with_cmd_prompt(config, std::env::var_os("PROMPT"))
}

fn with_cmd_prompt(config: PtySpawnConfig, inherited: Option<OsString>) -> PtySpawnConfig {
    // $e expands to ESC; ESC backslash terminates OSC. Append after the
    // original visible prompt so readiness follows cmd's initialization /k
    // command. No startup command is interpolated into shell arguments.
    const READY: &str = "$e]133;A$e\\";
    let mut environment = config.environment().to_vec();
    let existing = environment
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("PROMPT"));
    let mut prompt = existing
        .map(|(_, value)| value.clone())
        .or(inherited)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "$P$G".into());
    if !prompt.to_string_lossy().ends_with(READY) {
        prompt.push(READY);
    }
    environment.retain(|(key, _)| !key.eq_ignore_ascii_case("PROMPT"));
    environment.push(("PROMPT".into(), prompt));
    config.with_environment(environment)
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

pub(crate) fn spawn(
    backend: &PortablePtyBackend,
    config: PtySpawnConfig,
    observer: Option<PtyStartupObserver>,
) -> Result<PortablePtySession, String> {
    let program = config.program().to_owned();
    backend
        .spawn_observed(config, observer)
        .map_err(|error| format!("could not start local session program {program:?}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(program: &str, args: &[&str]) -> PtySpawnConfig {
        configured_spawn_config(
            &ShellConfig {
                env: Default::default(),
                program: program.into(),
                args: args.iter().map(|arg| (*arg).into()).collect(),
            },
            PtySize::new(24, 80).unwrap(),
        )
    }

    #[test]
    fn names_paths_and_exact_arguments_pass_through() {
        for program in [
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
    fn configured_child_environment_survives_shell_integration() {
        for program in ["bash", "cmd.exe", "pwsh"] {
            let shell = ShellConfig {
                program: program.into(),
                args: vec![],
                env: [
                    ("TERM".into(), "dumb".into()),
                    ("CUSTOM".into(), "two words".into()),
                ]
                .into(),
            };
            let config = configured_spawn_config(&shell, PtySize::new(24, 80).unwrap());
            assert!(
                config
                    .environment()
                    .contains(&("TERM".into(), "dumb".into()))
            );
            assert!(
                config
                    .environment()
                    .contains(&("CUSTOM".into(), "two words".into()))
            );
        }
    }

    #[test]
    fn cmd_prompt_preserves_custom_text_environment_and_is_idempotent() {
        let config = PtySpawnConfig::new("cmd.exe".into(), PtySize::new(24, 80).unwrap())
            .with_environment([
                ("KEEP".into(), "value".into()),
                ("Prompt".into(), "custom $P$G".into()),
            ]);
        let hooked = with_cmd_prompt(config, Some("ignored".into()));
        assert_eq!(
            hooked.environment(),
            &[
                ("KEEP".into(), "value".into()),
                ("PROMPT".into(), "custom $P$G$e]133;A$e\\".into())
            ]
        );
        let repeated = with_cmd_prompt(hooked.clone(), None);
        assert_eq!(repeated.environment(), hooked.environment());
        for inherited in [None, Some(OsString::new()), Some("$P$G".into())] {
            let config = with_cmd_prompt(
                PtySpawnConfig::new("cmd.exe".into(), PtySize::new(24, 80).unwrap()),
                inherited,
            );
            assert_eq!(config.environment()[0].1, "$P$G$e]133;A$e\\");
        }
    }

    #[cfg(windows)]
    #[test]
    fn cmd_hook_keeps_args_and_excludes_command_mode_and_other_programs() {
        for program in ["cmd", "CMD.EXE", r"C:\Windows\System32\cmd.exe"] {
            let config = request(program, &["/d", "/q", "/v:on", "/k", "echo initialized"]);
            assert_eq!(
                config.arguments(),
                &["/d", "/q", "/v:on", "/k", "echo initialized"]
            );
            assert_eq!(config.environment()[0].0, "PROMPT");
        }
        for args in [vec!["/c", "echo done"], vec!["/Cecho done"]] {
            let config = request("cmd.exe", &args);
            assert_eq!(config.arguments(), args.as_slice());
            assert!(config.environment().is_empty());
        }
        for program in ["mycmd.exe", "cmd.exe.backup", "cmd.cmd"] {
            assert!(request(program, &[]).environment().is_empty());
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
        let stages = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = std::sync::Arc::clone(&stages);
        let observer: PtyStartupObserver = std::sync::Arc::new(move |stage| {
            observed.lock().unwrap().push(stage);
        });
        let error = spawn(
            &PortablePtyBackend::new(),
            request(program, &[]),
            Some(observer),
        )
        .err()
        .expect("must fail, never fall back");
        assert!(error.contains(program), "{error}");
        assert!(error.contains("PTY spawn failed"), "{error}");
        let stages = stages.lock().unwrap();
        assert_eq!(stages.first(), Some(&"pty-create-begin"));
        assert!(stages.contains(&"child-spawn-requested"));
        assert!(!stages.contains(&"child-spawned"));
    }
}
