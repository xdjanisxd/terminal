#![cfg(target_os = "linux")]

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct ApplicationProcess(Child);
impl Drop for ApplicationProcess {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn wait_until(mut condition: impl FnMut() -> bool, description: &str) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out: {description}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn running(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[test]
#[ignore = "requires native Linux display and GPU; launches the actual application"]
fn native_linux_shell_exit_closes_application_process_and_reaps_sessions() {
    let root = std::env::temp_dir().join(format!(
        "terminal-native-shell-exit-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    for program in ["bash", "zsh"] {
        for scenario in [
            "single-zero",
            "single-nonzero",
            "split",
            "tabs",
            "mixed",
            "startup",
        ] {
            let directory = root.join(format!("{program}-{scenario}"));
            fs::create_dir_all(&directory).unwrap();
            fs::create_dir(directory.join("home")).unwrap();
            fs::create_dir(directory.join("config")).unwrap();
            let temporary = directory.join("tmp");
            fs::create_dir(&temporary).unwrap();
            let panes = match scenario {
                "split" | "tabs" => 2,
                "mixed" => 3,
                _ => 1,
            };
            let leaf = |index: usize| {
                let command = if index + 1 == panes && panes > 1 {
                    format!(
                        "echo $$ > {}/pid-{index}; while [ ! -f {}/release ]; do sleep 0.05; done; exit 23",
                        directory.display(),
                        directory.display()
                    )
                } else {
                    format!(
                        "echo $$ > {}/pid-{index}; exit {}",
                        directory.display(),
                        if scenario == "single-zero" { 0 } else { 23 }
                    )
                };
                format!(
                    "{{ kind = 'pane', session = 'local_shell', startup_command = '{command}' }}"
                )
            };
            let mut config = format!(
                "[shell]\nprogram = '{program}'\n[ shell.env ]\nHOME = '{}'\n[workspace]\n",
                directory.join("home").display()
            );
            if scenario == "startup" {
                config.push_str(&format!("[[workspace.tabs]]\ntitle = 'Lifecycle'\nlayout = {{ kind = 'pane', session = {{ command = {{ program = '{program}', args = ['-c', 'exit 23'] }} }} }}\n"));
            } else if scenario == "tabs" {
                for index in 0..panes {
                    config.push_str(&format!(
                        "[[workspace.tabs]]\ntitle = 'Lifecycle'\nlayout = {}\n",
                        leaf(index)
                    ));
                }
            } else if scenario == "split" || scenario == "mixed" {
                config.push_str(&format!("[[workspace.tabs]]\ntitle = 'Lifecycle'\nlayout = {{ kind = 'split', axis = 'vertical', first = {}, second = {} }}\n", leaf(0), leaf(1)));
                if scenario == "mixed" {
                    config.push_str(&format!(
                        "[[workspace.tabs]]\ntitle = 'Lifecycle'\nlayout = {}\n",
                        leaf(2)
                    ));
                }
            } else {
                config.push_str(&format!(
                    "[[workspace.tabs]]\ntitle = 'Lifecycle'\nlayout = {}\n",
                    leaf(0)
                ));
            }
            let config_path = directory.join("workspace.toml");
            fs::write(&config_path, &config).unwrap();
            let saved = directory.join("config/terminal/workspaces.toml");
            fs::create_dir_all(saved.parent().unwrap()).unwrap();
            fs::write(&saved, "# must survive automatic exit\nworkspaces = []\n").unwrap();
            let log = directory.join("stderr.log");
            let mut process = ApplicationProcess(
                Command::new(env!("CARGO_BIN_EXE_terminal"))
                    .arg("--workspace")
                    .arg(&config_path)
                    .env("XDG_CONFIG_HOME", directory.join("config"))
                    .env("TMPDIR", &temporary)
                    .env("TERMINAL_RENDERER_DIAGNOSTICS", "1")
                    .stdout(Stdio::null())
                    .stderr(fs::File::create(&log).unwrap())
                    .spawn()
                    .unwrap(),
            );
            let app_pid = process.0.id();
            let mut shell_pids = Vec::new();
            if scenario != "startup" {
                wait_until(
                    || {
                        (0..panes).all(|index| {
                            fs::read_to_string(directory.join(format!("pid-{index}")))
                                .is_ok_and(|text| text.trim().parse::<u32>().is_ok())
                        })
                    },
                    "shell startup markers",
                );
                shell_pids = (0..panes)
                    .map(|index| {
                        fs::read_to_string(directory.join(format!("pid-{index}")))
                            .unwrap()
                            .trim()
                            .parse::<u32>()
                            .unwrap()
                    })
                    .collect();
                if panes > 1 {
                    wait_until(
                        || shell_pids[..panes - 1].iter().all(|pid| !running(*pid)),
                        "exited shells reaped",
                    );
                    wait_until(
                        || fs::read_dir(&temporary).unwrap().count() == 1,
                        "exited panes release their integration files",
                    );
                    assert!(
                        process.0.try_wait().unwrap().is_none(),
                        "{program} {scenario}: application exited with live pane; {}",
                        fs::read_to_string(&log).unwrap()
                    );
                    assert!(running(shell_pids[panes - 1]), "surviving shell was killed");
                    fs::write(directory.join("release"), "exit").unwrap();
                }
            }
            let mut status = None;
            wait_until(
                || {
                    status = process.0.try_wait().unwrap();
                    status.is_some()
                },
                "actual application process exit",
            );
            assert!(
                status.unwrap().success(),
                "{program} {scenario}: {}",
                fs::read_to_string(&log).unwrap()
            );
            assert!(!running(app_pid));
            assert_eq!(
                fs::read_dir(&temporary).unwrap().count(),
                0,
                "session resources leaked"
            );
            assert!(
                shell_pids.iter().all(|pid| !running(*pid)),
                "unreaped shell"
            );
            assert_eq!(
                fs::read_to_string(saved).unwrap(),
                "# must survive automatic exit\nworkspaces = []\n"
            );
            assert!(
                fs::read_to_string(&log)
                    .unwrap()
                    .contains("app event=last-shell-exited"),
                "lifecycle exit path not observed"
            );
            eprintln!("passed {program} {scenario}: app PID {app_pid}, shell PIDs {shell_pids:?}");
        }
    }
    eprintln!("native lifecycle evidence: {}", root.display());
}
