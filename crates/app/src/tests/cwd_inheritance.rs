use super::*;

fn uri(path: &std::path::Path) -> String {
    let path = path.to_str().unwrap().replace('\\', "/");
    let prefix = if path.starts_with('/') {
        "file://"
    } else {
        "file:///"
    };
    let encoded: String = path
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'/' | b':' | b'-' | b'_' | b'.') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("{prefix}{encoded}")
}

fn report(app: &mut Application, uri: &str) {
    app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(
        format!("\x1b]7;{uri}\x07").into_bytes(),
    )));
}

#[test]
fn cwd_inheritance_tracks_focused_pane_without_promoting_roots_or_commands() {
    let state = saved_workspaces::TestWorkspaceState::new();
    let first_dir = state.path().parent().unwrap().join("first é # directory");
    let second_dir = first_dir.join("second");
    std::fs::create_dir_all(&second_dir).unwrap();
    let mut app = Application::default();
    let first = app.active_runtime_pane;
    app.workspace
        .set_focused_pane_startup_command(Some("echo keep source only".into()));
    report(&mut app, &uri(&first_dir));
    app.create_pane(Some(SplitAxis::Horizontal));
    let second = app.active_runtime_pane;
    assert_eq!(app.inherited_cwd, Some(first_dir.clone()));
    assert_eq!(app.workspace.pane_launch(second).unwrap().0, None);
    assert_eq!(app.workspace.pane_startup_command(second), None);
    assert_eq!(
        app.workspace.pane_startup_command(first),
        Some("echo keep source only")
    );
    report(&mut app, &uri(&second_dir));
    app.create_pane(None);
    assert_eq!(app.inherited_cwd, Some(second_dir));
    assert_eq!(
        app.workspace
            .pane_launch(app.active_runtime_pane)
            .unwrap()
            .0,
        None
    );
    // Snapshot before any prompt still retains the inherited launch directory.
    assert!(matches!(
        &app.snapshot_workspace().tabs[1].layout,
        LayoutDefinition::Pane {
            project_root: Some(_),
            ..
        }
    ));
    app.workspace.focus_tab(-1);
    app.workspace.focus_pane_id(first);
    app.activate_pane(first);
    app.create_pane(Some(SplitAxis::Vertical));
    assert_eq!(app.inherited_cwd, Some(first_dir));
}

#[test]
fn cwd_inheritance_respects_explicit_tab_workspace_and_project_roots() {
    let directory = std::env::current_dir().unwrap();
    for tab_root in [false, true] {
        for axis in [None, Some(SplitAxis::Horizontal)] {
            let mut app = Application::default();
            let mut definition = app.workspace.definition();
            let root = PathBuf::from("explicit root keeps existing validation");
            if tab_root {
                definition.tabs[0].project_root = Some(root.clone());
            } else {
                definition.project_root = Some(root.clone());
            }
            app.load_workspace(&definition);
            report(&mut app, &uri(&directory));
            app.create_pane(axis);
            assert_eq!(
                app.workspace
                    .pane_launch(app.active_runtime_pane)
                    .unwrap()
                    .0,
                Some(root)
            );
            assert_eq!(app.inherited_cwd, None);
            report(&mut app, &uri(&directory));
            app.create_pane_at_root(axis, Some("project action".into()));
            assert_eq!(
                app.workspace
                    .pane_launch(app.active_runtime_pane)
                    .unwrap()
                    .0,
                Some("project action".into())
            );
            assert_eq!(app.inherited_cwd, None);
        }
    }
}

#[test]
fn initial_session_keeps_inherited_process_cwd_unless_a_root_is_configured() {
    let size = PtySize::new(24, 80).unwrap();
    let session = SessionDefinition::LocalShell;
    let inherited = spawn_config_for_session(size, None, session.clone(), None);
    assert_eq!(inherited.working_directory(), None);

    let configured = PathBuf::from("project é 日本語 with spaces");
    let explicit = spawn_config_for_session(size, Some(configured.clone()), session, None);
    assert_eq!(explicit.working_directory(), Some(configured.as_path()));
}

#[test]
fn cwd_inheritance_missing_remote_invalid_and_deleted_reports_fall_back() {
    let state = saved_workspaces::TestWorkspaceState::new();
    let path = state.path().parent().unwrap().join("removed directory");
    std::fs::create_dir_all(&path).unwrap();
    let mut app = Application::default();
    report(&mut app, &uri(&path));
    // Malformed metadata does not replace the last valid report in terminal-core.
    report(&mut app, "file:///invalid%GG");
    app.create_pane(None);
    assert_eq!(app.inherited_cwd, Some(path.clone()));
    std::fs::remove_dir(&path).unwrap();
    assert_eq!(cwd::launch_root(None, app.inherited_cwd.as_ref()), None);
    let explicit = PathBuf::from("missing explicit path");
    assert_eq!(
        cwd::launch_root(Some(explicit.clone()), app.inherited_cwd.as_ref()),
        Some(explicit)
    );
    std::fs::write(state.path(), "regular file").unwrap();
    for reported in [
        None,
        Some(uri(&path)),
        Some("file://ssh-host/tmp".into()),
        Some(uri(&state.path())),
    ] {
        let mut app = Application::default();
        if let Some(reported) = reported {
            report(&mut app, &reported);
        }
        app.create_pane(Some(SplitAxis::Vertical));
        assert_eq!(app.inherited_cwd, None);
    }
}

#[test]
fn cwd_inheritance_preserves_saved_pane_roots_and_clears_runtime_on_restore() {
    let mut app = Application::default();
    let definition = app.workspace.definition();
    report(&mut app, &uri(&std::env::current_dir().unwrap()));
    app.create_pane(None);
    assert!(app.inherited_cwd.is_some());
    let mut restored = definition;
    if let LayoutDefinition::Pane { project_root, .. } = &mut restored.tabs[0].layout {
        *project_root = Some("saved pane root".into());
    }
    app.load_workspace(&restored);
    assert_eq!(app.inherited_cwd, None);
    assert_eq!(
        app.workspace
            .pane_launch(app.active_runtime_pane)
            .unwrap()
            .0,
        Some("saved pane root".into())
    );
    assert_eq!(app.workspace.definition(), restored);
}

#[cfg(target_os = "linux")]
#[test]
fn cwd_inheritance_native_bash_and_zsh_report_and_launch() {
    fn start(app: &mut Application, shell: &ShellConfig, root: Option<PathBuf>) {
        let config = spawn_config_for_session(
            pty_size_for_terminal(app.terminal.dimensions()),
            root,
            SessionDefinition::LocalShell,
            Some(shell),
        );
        app.pty = Some(
            PtyWorker::start(shell::spawn(&PortablePtyBackend::new(), config, None).unwrap())
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.terminal.shell_prompt_version() == 0 && Instant::now() < deadline {
            if let Some(event) = app
                .pty
                .as_mut()
                .unwrap()
                .recv_timeout(Duration::from_millis(20))
                .unwrap()
            {
                app.handle_pty_event(event);
            }
        }
        assert!(
            app.terminal.shell_prompt_version() > 0,
            "{:?} did not report readiness",
            shell.program
        );
    }

    for program in ["/bin/bash", "/bin/zsh"] {
        if !std::path::Path::new(program).is_file() {
            eprintln!("skipping unavailable shell {program}");
            continue;
        }
        let state = saved_workspaces::TestWorkspaceState::new();
        let directory = state.path().parent().unwrap().join("inherited é # cwd");
        std::fs::create_dir_all(&directory).unwrap();
        let shell = ShellConfig {
            program: program.into(),
            args: if program.ends_with("bash") {
                vec!["--noprofile".into(), "--norc".into()]
            } else {
                vec!["-f".into()]
            },
            env: Default::default(),
        };
        let process_directory = std::env::current_dir().unwrap();
        let mut inherited_app = Application::default();
        start(&mut inherited_app, &shell, None);
        assert_eq!(
            inherited_app
                .terminal
                .working_directory_uri()
                .and_then(local_cwd_from_osc7),
            Some(process_directory)
        );
        inherited_app.shutdown_sessions();

        let mut app = Application::default();
        start(&mut app, &shell, Some(directory.clone()));
        assert_eq!(
            app.terminal
                .working_directory_uri()
                .and_then(local_cwd_from_osc7),
            Some(directory.clone())
        );
        for axis in [Some(SplitAxis::Horizontal), None] {
            app.create_pane(axis);
            assert_eq!(app.inherited_cwd, Some(directory.clone()));
            let root = cwd::launch_root(
                app.workspace
                    .pane_launch(app.active_runtime_pane)
                    .unwrap()
                    .0,
                app.inherited_cwd.as_ref(),
            );
            start(&mut app, &shell, root);
            assert_eq!(
                app.terminal
                    .working_directory_uri()
                    .and_then(local_cwd_from_osc7),
                Some(directory.clone())
            );
        }
        app.shutdown_sessions();
    }
}
