use super::*;
use terminal_pty::PtyExitStatus;

fn exit_active(app: &mut Application, code: i32) {
    app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Exited(
        PtyExitStatus::code(code),
    )));
    app.close_exited_panes();
}

#[test]
fn shell_exit_final_pane_requests_application_shutdown_for_all_statuses() {
    for code in [0, 23] {
        let mut app = Application {
            pending_startup_command: Some("must not run".into()),
            ..Application::default()
        };
        exit_active(&mut app, code);
        assert!(app.session_exit_requested);
        assert!(app.pty.is_none());
        assert!(app.pending_startup_command.is_none());
        // A repeated event cannot remove an unrelated pane or restart a shell.
        exit_active(&mut app, code);
        app.start_local_shell();
        assert!(app.pty.is_none());
    }
}

#[test]
fn shell_exit_removes_pane_and_restores_surviving_runtime() {
    let mut app = Application::default();
    let first = app.workspace.active_pane();
    app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Bytes(b"A".to_vec())));
    app.dispatch_command(Command::SplitVertical);
    let second = app.workspace.active_pane();
    exit_active(&mut app, 23);
    assert_eq!(app.workspace.active_pane(), first);
    assert_eq!(app.active_runtime_pane, first);
    assert_eq!(app.terminal.screen().cell(0, 0).unwrap().character(), 'A');
    assert!(!app.inactive_panes.contains_key(&second));
    assert_eq!(app.workspace.panes().len(), 1);
    assert!(!app.session_exit_requested);
}

#[test]
fn shell_exit_background_pane_and_tab_preserve_focus() {
    let mut app = Application::default();
    let first = app.workspace.active_pane();
    app.dispatch_command(Command::SplitVertical);
    let second = app.workspace.active_pane();
    app.exited_panes.insert(first);
    app.close_exited_panes();
    assert_eq!(app.active_runtime_pane, second);
    assert_eq!(app.workspace.active_pane(), second);
    app.dispatch_command(Command::NewTab);
    let third = app.workspace.active_pane();
    app.exited_panes.insert(second);
    app.close_exited_panes();
    assert_eq!(app.workspace.tabs().len(), 1);
    assert_eq!(app.active_runtime_pane, third);
    assert!(!app.session_exit_requested);
    exit_active(&mut app, 0);
    assert!(app.session_exit_requested);
}

#[test]
fn shell_exit_active_tab_selects_surviving_tab() {
    let mut app = Application::default();
    let first = app.workspace.active_pane();
    app.dispatch_command(Command::NewTab);
    exit_active(&mut app, 0);
    assert_eq!(app.workspace.tabs().len(), 1);
    assert_eq!(app.active_runtime_pane, first);
    assert!(!app.session_exit_requested);
}

#[test]
fn shell_exit_simultaneous_events_are_deduplicated() {
    let mut app = Application::default();
    app.dispatch_command(Command::SplitVertical);
    app.dispatch_command(Command::NewTab);
    for pane in app.workspace.panes() {
        app.exited_panes.insert(pane.id);
        app.exited_panes.insert(pane.id);
    }
    app.close_exited_panes();
    assert!(app.session_exit_requested);
    assert!(app.inactive_panes.is_empty());
    assert!(app.exited_panes.is_empty());
}

#[test]
fn shell_exit_does_not_change_saved_workspace_or_explicit_last_pane_close() {
    let state = saved_workspaces::TestWorkspaceState::new();
    let mut app = Application {
        saved_workspaces_path: state.path(),
        ..Application::default()
    };
    app.dispatch_command(Command::SplitVertical);
    app.save_workspace("Before exit".into());
    let before = std::fs::read(&app.saved_workspaces_path).unwrap();
    exit_active(&mut app, 0);
    app.dispatch_command(Command::ClosePane);
    assert!(!app.session_exit_requested);
    exit_active(&mut app, 23);
    assert_eq!(std::fs::read(&app.saved_workspaces_path).unwrap(), before);
}

#[test]
fn shell_exit_real_shell_workers_close_foreground_and_background_sessions() {
    use std::time::{Duration, Instant};
    #[cfg(target_os = "linux")]
    let programs = ["bash", "zsh"];
    #[cfg(windows)]
    let programs = ["cmd.exe", "powershell.exe"];
    #[cfg(not(any(target_os = "linux", windows)))]
    let programs = ["sh"];
    for program in programs {
        #[cfg(target_os = "linux")]
        if program == "zsh"
            && std::process::Command::new(program)
                .arg("--version")
                .output()
                .is_err()
        {
            eprintln!("Zsh lifecycle regression skipped: zsh is unavailable");
            continue;
        }
        let mut app = Application::default();
        app.dispatch_command(Command::SplitVertical);
        app.dispatch_command(Command::NewTab);
        let panes = app.workspace.panes();
        attach_shell_workers(&mut app, program);
        for pane in panes {
            let worker = if pane.id == app.active_runtime_pane {
                app.pty.as_ref().unwrap()
            } else {
                app.inactive_panes[&pane.id].pty.as_ref().unwrap()
            };
            worker.write(b"exit 23\r".to_vec()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(15);
            while app.workspace.pane_launch(pane.id).is_some() && !app.session_exit_requested {
                app.drain_pty_events(false);
                assert!(
                    Instant::now() < deadline,
                    "{program}: exit did not close {:?}",
                    pane.id
                );
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(app.session_exit_requested, "{program}");
        assert!(app.pty.is_none());
        assert!(app.inactive_panes.is_empty());
    }
}

#[test]
fn shell_exit_workspace_replacement_discards_stale_exit_identity() {
    let mut app = Application::default();
    app.exited_panes.insert(app.active_runtime_pane);
    app.load_workspace(&terminal_workspace::WorkspaceDefinition::default());
    app.close_exited_panes();
    assert!(!app.session_exit_requested);
    assert!(app.exited_panes.is_empty());
}

fn attach_shell_workers(app: &mut Application, program: &str) {
    let shell = ShellConfig {
        program: program.into(),
        args: Vec::new(),
        env: Default::default(),
    };
    for pane in app.workspace.panes() {
        let (root, session) = app.workspace.pane_launch(pane.id).unwrap();
        let config =
            spawn_config_for_session(PtySize::new(24, 80).unwrap(), root, session, Some(&shell));
        let worker =
            PtyWorker::start(shell::spawn(&PortablePtyBackend::new(), config, None).unwrap())
                .unwrap();
        let pending = app
            .workspace
            .pane_startup_command(pane.id)
            .filter(|_| shell::startup_command_allowed(Some(&shell)))
            .map(str::to_owned);
        if pane.id == app.active_runtime_pane {
            app.pty = Some(worker);
            app.pending_startup_command = pending;
        } else {
            let runtime = app.inactive_panes.get_mut(&pane.id).unwrap();
            runtime.pty = Some(worker);
            runtime.pending_startup_command = pending;
        }
    }
}

#[test]
fn shell_exit_during_shutdown_cannot_restart_or_remove_survivors_twice() {
    let mut app = Application::default();
    app.dispatch_command(Command::SplitVertical);
    #[cfg(windows)]
    let program = "cmd.exe";
    #[cfg(not(windows))]
    let program = "sh";
    attach_shell_workers(&mut app, program);
    app.pty
        .as_ref()
        .unwrap()
        .write(b"exit 23\r".to_vec())
        .unwrap();
    // Stop while the natural exit may still be racing its notification.
    app.shutdown_sessions();
    assert!(app.pty.is_none());
    assert!(app.inactive_panes.is_empty());
    app.shutdown_sessions();
    exit_active(&mut app, 0);
    app.start_local_shell();
    assert!(app.pty.is_none());
    assert!(app.exited_panes.is_empty());
}

#[test]
fn shell_exit_eof_alone_does_not_close_a_running_pane() {
    let mut app = Application::default();
    app.handle_pty_event(PtyWorkerEvent::Output(PtyOutput::Eof));
    app.close_exited_panes();
    assert!(!app.session_exit_requested);
    assert_eq!(app.workspace.panes().len(), 1);
}

#[test]
fn shell_exit_of_explicitly_closed_pane_is_ignored() {
    let mut app = Application::default();
    app.dispatch_command(Command::SplitVertical);
    let closing = app.active_runtime_pane;
    app.exited_panes.insert(closing);
    app.close_pane();
    app.close_exited_panes();
    assert!(!app.session_exit_requested);
    assert_eq!(app.workspace.panes().len(), 1);
}

#[test]
#[cfg(any(target_os = "linux", windows))]
fn shell_exit_during_saved_workspace_restoration_closes_all_restored_workers() {
    use std::time::{Duration, Instant};
    #[cfg(target_os = "linux")]
    let program = "bash";
    #[cfg(windows)]
    let program = "cmd.exe";
    let state = saved_workspaces::TestWorkspaceState::new();
    let mut app = Application {
        saved_workspaces_path: state.path(),
        ..Application::default()
    };
    app.config.shell = Some(ShellConfig {
        program: program.into(),
        args: Vec::new(),
        env: Default::default(),
    });
    app.workspace
        .set_focused_pane_startup_command(Some("exit 23".into()));
    app.dispatch_command(Command::SplitVertical);
    app.workspace
        .set_focused_pane_startup_command(Some("exit 0".into()));
    app.save_workspace("Exit on restore".into());
    let before = std::fs::read(&app.saved_workspaces_path).unwrap();
    attach_shell_workers(&mut app, program);
    app.exited_panes.insert(app.active_runtime_pane);
    // Replacing live workers also invalidates already-observed exits.
    app.open_saved_workspace("Exit on restore");
    assert!(app.exited_panes.is_empty());
    assert!(!app.session_exit_requested);
    attach_shell_workers(&mut app, program);
    let deadline = Instant::now() + Duration::from_secs(15);
    while !app.session_exit_requested {
        app.drain_pty_events(false);
        assert!(
            Instant::now() < deadline,
            "restored shell startup exit was not handled"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.pty.is_none());
    assert!(app.inactive_panes.is_empty());
    assert_eq!(std::fs::read(&app.saved_workspaces_path).unwrap(), before);
}

#[test]
fn shell_exit_collapses_zoomed_split_layout_without_losing_focus() {
    let mut app = Application::default();
    let first = app.workspace.active_pane();
    app.dispatch_command(Command::SplitVertical);
    let second = app.workspace.active_pane();
    app.dispatch_command(Command::TogglePaneZoom);
    app.exited_panes.insert(first);
    app.close_exited_panes();
    assert_eq!(app.active_runtime_pane, second);
    assert!(app.workspace.active_tab().is_zoomed());
    let viewport = PaneRect {
        x: 0,
        y: 0,
        width: 101,
        height: 51,
    };
    assert_eq!(
        app.workspace.active_tab().pane_rects(viewport),
        vec![(second, viewport)]
    );
    assert!(!app.session_exit_requested);
}
