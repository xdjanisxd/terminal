#![cfg(target_os = "linux")]

use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
fn wait(mut condition: impl FnMut() -> bool, message: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out: {message}");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn directory() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "terminal-execute-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
#[ignore = "requires native KDE Wayland and GPU; launches the actual application"]
fn native_execute_exact_arguments_and_exit_lifecycle() {
    assert_eq!(std::env::var("XDG_SESSION_TYPE").unwrap(), "wayland");
    let root = directory();
    let probe = root.join("probe space 日本語.py");
    fs::write(
        &probe,
        r#"#!/usr/bin/python3
import os, sys, time
from pathlib import Path
root = Path(sys.argv[1])
(root / 'arguments').write_bytes(b'\0'.join(os.fsencode(x) for x in sys.argv[3:]))
(root / 'cwd').write_text(os.getcwd())
(root / 'tty').write_text(str(all(os.isatty(i) for i in range(3))))
(root / 'pid').write_text(str(os.getpid()))
while not (root / 'release').exists(): time.sleep(.01)
sys.exit(int(sys.argv[2]))
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&probe, fs::Permissions::from_mode(0o700)).unwrap();
    let config = root.join("config.toml");
    fs::write(&config, "[shell]\nprogram = 'terminal-missing-shell'\n[workspace]\n[[workspace.tabs]]\ntitle = 'must not launch'\nlayout = { kind = 'pane', session = 'local_shell' }\n[[workspace.tabs]]\ntitle = 'must not launch either'\nlayout = { kind = 'pane', session = 'local_shell' }\n").unwrap();
    let saved = root.join("config/terminal/workspaces.toml");
    fs::create_dir_all(saved.parent().unwrap()).unwrap();
    fs::write(&saved, "# preserved\nworkspaces = []\n").unwrap();
    for (index, syntax) in ["-e", "--execute", "--"].into_iter().enumerate() {
        let case = root.join(index.to_string());
        fs::create_dir(&case).unwrap();
        let args = [
            "two words 日本語.txt",
            "",
            "'literal'",
            "\"quoted\"",
            "$HOME; touch unexpected",
            "--help",
            "--workspace",
            "-e",
            "--",
        ];
        let log = case.join("stderr.log");
        let mut app = Process(
            Command::new(env!("CARGO_BIN_EXE_terminal"))
                .args(["--workspace"])
                .arg(&config)
                .arg("--project-root")
                .arg(&case)
                .arg(syntax)
                .arg(&probe)
                .arg(&case)
                .arg(if index == 1 { "23" } else { "0" })
                .args(args)
                .env("XDG_CONFIG_HOME", root.join("config"))
                .env("TERMINAL_RENDERER_DIAGNOSTICS", "1")
                .stdout(Stdio::null())
                .stderr(fs::File::create(&log).unwrap())
                .spawn()
                .unwrap(),
        );
        wait(
            || {
                assert!(
                    app.0.try_wait().unwrap().is_none(),
                    "{}",
                    fs::read_to_string(&log).unwrap()
                );
                case.join("pid").exists()
            },
            "direct child ready",
        );
        let pid: u32 = fs::read_to_string(case.join("pid"))
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            fs::read(case.join("arguments")).unwrap(),
            args.join("\0").as_bytes()
        );
        assert_eq!(
            fs::read_to_string(case.join("cwd")).unwrap(),
            case.to_str().unwrap()
        );
        assert_eq!(fs::read_to_string(case.join("tty")).unwrap(), "True");
        assert!(app.0.try_wait().unwrap().is_none());
        fs::write(case.join("release"), "exit").unwrap();
        let mut status = None;
        wait(
            || {
                status = app.0.try_wait().unwrap();
                status.is_some()
            },
            "window/application exit",
        );
        assert!(
            status.unwrap().success(),
            "{}",
            fs::read_to_string(&log).unwrap()
        );
        assert!(
            !Path::new(&format!("/proc/{pid}")).exists(),
            "child not reaped"
        );
        assert!(
            fs::read_to_string(&log)
                .unwrap()
                .contains("app event=last-shell-exited")
        );
        assert_eq!(
            fs::read_to_string(&saved).unwrap(),
            "# preserved\nworkspaces = []\n"
        );
        eprintln!("passed {syntax}: exact argv, PTY, cwd, exit and reaping");
    }
    eprintln!("native execution evidence: {}", root.display());
}

#[test]
#[ignore = "requires native KDE Wayland, GPU, Vim and Neovim"]
fn native_execute_editors_with_unicode_and_spaces() {
    let root = directory();
    let config = root.join("config.toml");
    fs::write(
        &config,
        "[workspace]\n[[workspace.tabs]]\ntitle = 'Terminal'\nlayout = { kind = 'pane', session = 'local_shell' }\n",
    )
    .unwrap();
    for (program, syntax) in [("vim", "-e"), ("nvim", "--execute")] {
        let document = root.join(format!("{program} çalışma 日本語 file.txt"));
        let marker = root.join(format!("{program}.opened"));
        fs::write(&document, "Native Terminal Unicode path acceptance\n").unwrap();
        let log = root.join(format!("{program}.stderr.log"));
        let record = format!("call writefile([expand('%:p')], '{}')", marker.display());
        let mut app = Process(
            Command::new(env!("CARGO_BIN_EXE_terminal"))
                .arg("--workspace")
                .arg(&config)
                .args([syntax, program, "-u", "NONE", "-n", "-c"])
                .arg(&record)
                .args(["-c", "call timer_start(2000, {-> execute('qa!')})"])
                .arg(&document)
                .env("XDG_CONFIG_HOME", root.join("config"))
                .env("TERMINAL_RENDERER_DIAGNOSTICS", "1")
                .stdout(Stdio::null())
                .stderr(fs::File::create(&log).unwrap())
                .spawn()
                .unwrap(),
        );
        wait(
            || {
                assert!(
                    app.0.try_wait().unwrap().is_none(),
                    "{}",
                    fs::read_to_string(&log).unwrap()
                );
                marker.exists()
            },
            "editor opening document",
        );
        assert_eq!(
            fs::read_to_string(&marker).unwrap().trim(),
            document.to_str().unwrap()
        );
        let mut status = None;
        wait(
            || {
                status = app.0.try_wait().unwrap();
                status.is_some()
            },
            "editor/window exit",
        );
        assert!(
            status.unwrap().success(),
            "{}",
            fs::read_to_string(&log).unwrap()
        );
        assert!(
            fs::read_to_string(&log)
                .unwrap()
                .contains("app event=last-shell-exited")
        );
        eprintln!("passed {program}: opened exact Unicode/space path and closed on exit");
    }
    eprintln!("native editor evidence: {}", root.display());
}

#[test]
#[ignore = "requires native Wayland and GPU"]
fn native_execute_missing_program_exits_without_shell_fallback() {
    let root = directory();
    let log = root.join("missing-program.log");
    let mut app = Process(
        Command::new(env!("CARGO_BIN_EXE_terminal"))
            .args(["-e", "terminal-intentionally-missing-executable-9c80d2"])
            .env("XDG_CONFIG_HOME", root.join("config"))
            .stdout(Stdio::null())
            .stderr(fs::File::create(&log).unwrap())
            .spawn()
            .unwrap(),
    );
    wait(
        || app.0.try_wait().unwrap().is_some(),
        "failed launch exits application",
    );
    assert!(
        fs::read_to_string(&log)
            .unwrap()
            .contains("terminal-intentionally-missing-executable-9c80d2")
    );
    eprintln!(
        "passed missing command: reported executable, no shell fallback; {}",
        root.display()
    );
}
