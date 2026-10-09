use super::*;
use std::fs;
use terminal_workspace::{LayoutDefinition, SessionDefinition, SplitAxis};

fn attach_workers(app: &mut Application, shell: &ShellConfig) {
    let active = app.active_runtime_pane;
    for pane in app.workspace.panes() {
        let (root, session) = app.workspace.pane_launch(pane.id).unwrap();
        let config = super::spawn_config_for_session(
            PtySize::new(24, 80).unwrap(),
            root,
            session,
            Some(shell),
        );
        let worker = terminal_pty::PtyWorker::start(
            super::shell::spawn(&super::PortablePtyBackend::new(), config, None).unwrap(),
        )
        .unwrap();
        let pending = app
            .workspace
            .pane_startup_command(pane.id)
            .filter(|_| shell::startup_command_allowed(Some(shell)))
            .map(str::to_owned);
        if pane.id == active {
            app.pty = Some(worker);
            app.pending_startup_command = pending;
        } else {
            let runtime = app.inactive_panes.get_mut(&pane.id).unwrap();
            runtime.pty = Some(worker);
            runtime.pending_startup_command = pending;
        }
    }
}

fn wait_for_prompts(app: &mut Application, shell: &ShellConfig, count: u64) {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut ready = false;
    while Instant::now() < deadline {
        if let Some(event) = app
            .pty
            .as_mut()
            .unwrap()
            .recv_timeout(Duration::from_millis(20))
            .unwrap()
        {
            app.handle_pty_event(event);
        }
        for (pane_id, runtime) in &mut app.inactive_panes {
            if let Some(event) = runtime
                .pty
                .as_mut()
                .unwrap()
                .recv_timeout(Duration::from_millis(20))
                .unwrap()
            {
                super::handle_background_pty_event(*pane_id, runtime, event);
            }
        }
        // Wait for the command to finish and another prompt, proving
        // later output does not dispatch the pending command twice.
        ready = app.terminal.shell_prompt_version() >= count
            && app
                .inactive_panes
                .values()
                .all(|runtime| runtime.terminal.shell_prompt_version() >= count);
        if ready {
            break;
        }
    }

    assert!(
        ready,
        "shell {:?} did not reach {count} prompts",
        shell.program
    );
}

#[test]
fn linux_saved_workspace_native_cwd_and_startup_commands() {
    use std::time::{Duration, Instant};
    for (program, args) in [
        ("bash", vec![]),
        ("bash", vec!["--noprofile", "--norc"]),
        ("bash", vec!["--rcfile", "USER_RC"]),
        ("zsh", vec![]),
        ("zsh", vec!["-f"]),
        ("zsh", vec!["-l"]),
    ] {
        if std::process::Command::new(program)
            .arg("--version")
            .output()
            .is_err()
        {
            eprintln!("native workspace test skipped unavailable shell {program}");
            continue;
        }
        let mut shell = ShellConfig {
            program: program.into(),
            args: args.into_iter().map(str::to_owned).collect(),
            env: Default::default(),
        };
        let state = saved_workspaces::TestWorkspaceState::new();
        let path = state.path();
        let home = path.parent().unwrap().join("shell home");
        let zdot = home.join("z dot files");
        fs::create_dir_all(&zdot).unwrap();
        shell
            .env
            .insert("HOME".into(), home.to_string_lossy().into_owned());
        shell.env.insert("KEEP".into(), "kept value".into());
        if shell.args.last().is_some_and(|arg| arg == "USER_RC") {
            let rc = home.join("custom rc");
            fs::write(
                &rc,
                "source ~/.bashrc\nPROMPT_COMMAND='printf scalar-hook'\n",
            )
            .unwrap();
            *shell.args.last_mut().unwrap() = rc.to_string_lossy().into_owned();
        }
        fs::write(home.join(".bashrc"), "sleep 0.03\ncd ~\nexport INITIALIZED=yes\nPROMPT_COMMAND=('printf old-hook')\nPS1='custom-bash> '\n").unwrap();
        fs::write(home.join(".zshenv"), "ZDOTDIR=\"$HOME/z dot files\"\n").unwrap();
        fs::write(zdot.join(".zshrc"), "sleep 0.03\ncd ~\nexport INITIALIZED=yes\nprecmd_functions=(user_hook)\nuser_hook() { printf old-hook; }\nPS1='custom-zsh> '\n").unwrap();
        fs::write(zdot.join(".zlogin"), "cd ~\nexport LOGIN_INITIALIZED=yes\n").unwrap();
        let mut definition = terminal_workspace::WorkspaceDefinition::default();
        let roots: Vec<_> = ["first pane", "second pane"]
            .iter()
            .map(|name| path.parent().unwrap().join(name))
            .collect();
        for root in &roots {
            fs::create_dir_all(root).unwrap();
        }
        let pane = |index: usize| LayoutDefinition::Pane {
            session: SessionDefinition::LocalShell,
            project_root: Some(roots[index].clone()),
            startup_command: Some(format!(
                "test \"$KEEP\" = 'kept value' && printf 'startup-ok-{index}\\n%s\\n' \"$PWD\" >> startup-result.txt"
            )),
        };
        definition.tabs[0].layout = LayoutDefinition::Split {
            axis: SplitAxis::Vertical,
            first_share: 400_000,
            first: Box::new(pane(0)),
            second: Box::new(pane(1)),
        };
        definition.tabs[0].active_pane = 1;
        let mut store = super::SavedWorkspaces::default();
        store
            .save("Native startup".into(), definition.clone())
            .unwrap();
        store.write(&path).unwrap();
        let mut app = Application {
            saved_workspaces_path: path,
            ..Application::default()
        };
        app.open_saved_workspace("Native startup");
        assert_eq!(app.workspace.definition(), definition);
        let active = app.active_runtime_pane;
        // Headless tests have no winit wake proxy. Attach real workers via
        // the production session config; use the actual foreground and
        // background event handlers for parsing and command dispatch.
        attach_workers(&mut app, &shell);
        wait_for_prompts(&mut app, &shell, 2);
        // Metadata from actual prompts must survive save/load, including
        // the inactive pane. Roots include spaces and Unicode test paths.
        assert_eq!(app.snapshot_workspace(), definition);
        for pane in app.workspace.panes() {
            let worker = if pane.id == active {
                app.pty.as_mut().unwrap()
            } else {
                app.inactive_panes
                    .get_mut(&pane.id)
                    .unwrap()
                    .pty
                    .as_mut()
                    .unwrap()
            };
            worker
                .write(b"mkdir -p 'changed % #; directory'; cd 'changed % #; directory'\r".to_vec())
                .unwrap();
        }
        let changed_deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < changed_deadline {
            if let Some(event) = app
                .pty
                .as_mut()
                .unwrap()
                .recv_timeout(Duration::from_millis(20))
                .unwrap()
            {
                app.handle_pty_event(event);
            }
            for (pane_id, runtime) in &mut app.inactive_panes {
                if let Some(event) = runtime
                    .pty
                    .as_mut()
                    .unwrap()
                    .recv_timeout(Duration::from_millis(20))
                    .unwrap()
                {
                    super::handle_background_pty_event(*pane_id, runtime, event);
                }
            }
            if app.terminal.shell_prompt_version() >= 3
                && app
                    .inactive_panes
                    .values()
                    .all(|r| r.terminal.shell_prompt_version() >= 3)
            {
                break;
            }
        }
        let changed_roots: Vec<_> = roots
            .iter()
            .map(|root| root.join("changed % #; directory"))
            .collect();
        let mut changed = definition.clone();
        super::apply_pane_roots(
            &mut changed.tabs[0].layout,
            &mut changed_roots.iter().cloned().map(Some),
        );
        assert_eq!(app.snapshot_workspace(), changed);
        app.save_workspace("Changed".into());
        assert_eq!(
            super::SavedWorkspaces::load(&app.saved_workspaces_path)
                .unwrap()
                .workspaces
                .iter()
                .find(|w| w.name == "Changed")
                .unwrap()
                .definition,
            changed
        );
        // Switching runtime ownership must not restore the pending command.
        let other = *app.inactive_panes.keys().next().unwrap();
        app.activate_pane(other);
        assert!(app.pending_startup_command.is_none());
        app.activate_pane(active);
        app.pty.as_mut().unwrap().shutdown_and_join().unwrap();
        for runtime in app.inactive_panes.values_mut() {
            runtime.pty.as_mut().unwrap().shutdown_and_join().unwrap();
        }
        assert!(app.pending_startup_command.is_none());
        assert!(
            app.inactive_panes
                .values()
                .all(|runtime| runtime.pending_startup_command.is_none())
        );
        assert_eq!(app.workspace.definition(), definition);
        for (index, root) in roots.iter().enumerate() {
            let read_result = |name: &str| fs::read_to_string(root.join(name)).unwrap();
            let result = read_result("startup-result.txt");
            let lines = result.lines().collect::<Vec<_>>();
            assert_eq!(lines.len(), 2, "shell {:?}: {result:?}", shell.program);
            assert_eq!(lines[0], format!("startup-ok-{index}"));
            assert_eq!(
                fs::canonicalize(lines[1]).unwrap(),
                root.canonicalize().unwrap(),
                "shell {:?}",
                shell.program
            );
        }
        app.open_saved_workspace("Changed");
        assert_eq!(app.workspace.definition(), changed);
        attach_workers(&mut app, &shell);
        wait_for_prompts(&mut app, &shell, 2);
        assert_eq!(app.snapshot_workspace(), changed);
        app.pty.as_mut().unwrap().shutdown_and_join().unwrap();
        for runtime in app.inactive_panes.values_mut() {
            runtime.pty.as_mut().unwrap().shutdown_and_join().unwrap();
        }
        for (index, root) in changed_roots.iter().enumerate() {
            assert_eq!(
                fs::read_to_string(root.join("startup-result.txt")).unwrap(),
                format!("startup-ok-{index}\n{}\n", root.display())
            );
        }
    }
}

#[test]
fn linux_panes_without_startup_commands_remain_idle_and_interactive() {
    for (program, args) in [("bash", vec!["--noprofile", "--norc"]), ("zsh", vec!["-f"])] {
        if std::process::Command::new(program)
            .arg("--version")
            .output()
            .is_err()
        {
            continue;
        }
        let state = saved_workspaces::TestWorkspaceState::new();
        let root = state.path().parent().unwrap().to_owned();
        fs::create_dir_all(&root).unwrap();
        let shell = ShellConfig {
            program: program.into(),
            args: args.iter().map(|s| (*s).into()).collect(),
            env: Default::default(),
        };
        let definition = terminal_workspace::WorkspaceDefinition {
            project_root: Some(root.clone()),
            ..Default::default()
        };
        let mut app = Application::default();
        app.load_workspace(&definition);
        attach_workers(&mut app, &shell);
        wait_for_prompts(&mut app, &shell, 1);
        assert!(app.pending_startup_command.is_none());
        assert_eq!(app.terminal.shell_prompt_version(), 1);
        assert!(!root.join("typed-result").exists());
        app.pty
            .as_mut()
            .unwrap()
            .write(b"printf typed > typed-result\r".to_vec())
            .unwrap();
        wait_for_prompts(&mut app, &shell, 2);
        assert_eq!(
            fs::read_to_string(root.join("typed-result")).unwrap_or_else(|e| panic!(
                "{program}: {e}; cwd={:?}; expected={root:?}",
                app.terminal.working_directory_uri()
            )),
            "typed"
        );
        app.pty.as_mut().unwrap().shutdown_and_join().unwrap();
    }
}
