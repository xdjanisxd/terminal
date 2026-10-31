use super::*;

fn populate(app: &mut Application) {
    app.parser
        .advance(&mut app.terminal, b"\x1b[2J\x1b[H")
        .unwrap();
    for index in 0..13 {
        let line =
            format!("Link {index:02}: https://example.org/path/{index}  surrounding text\r\n");
        app.parser
            .advance(&mut app.terminal, line.as_bytes())
            .unwrap();
    }
    app.parser
        .advance(
            &mut app.terminal,
            "Unicode: https://例え.test/e\u{301}_(x).\r\n".as_bytes(),
        )
        .unwrap();
}

#[test]
fn modal_hint_keys_precedence_and_cancellation() {
    let mut app = Application::default();
    populate(&mut app);
    app.dispatch_command(Command::OpenUrlHints);
    assert_eq!(app.url_hints.as_ref().unwrap().candidates.len(), 14);
    let physical = PhysicalKey::Code(KeyCode::KeyL);
    app.handle_url_hint_key(&Key::Character("l".into()), physical, Some("l"), false);
    assert_eq!(app.url_hints.as_ref().unwrap().prefix, "L");
    app.handle_url_hint_key(
        &Key::Character("x".into()),
        PhysicalKey::Code(KeyCode::KeyX),
        Some("x"),
        false,
    );
    assert_eq!(app.url_hints.as_ref().unwrap().prefix, "L");
    app.handle_url_hint_key(
        &Key::Named(NamedKey::Backspace),
        PhysicalKey::Code(KeyCode::Backspace),
        None,
        false,
    );
    assert_eq!(app.url_hints.as_ref().unwrap().visible().count(), 14);
    app.handle_url_hint_key(
        &Key::Named(NamedKey::Escape),
        PhysicalKey::Code(KeyCode::Escape),
        None,
        false,
    );
    assert!(app.url_hints.is_none());
    app.dispatch_command(Command::OpenUrlHints);
    app.modifiers = ModifiersState::CONTROL;
    app.config = Config::parse("[[bindings]]\nkey = 'Ctrl+L'\ncommand = 'copy'").unwrap();
    app.hint_keys_held.insert(physical);
    assert!(app.route_url_hint_key(&Key::Character("l".into()), physical, Some("l"), true));
    assert!(app.url_hints.is_none()); // Explicit bindings also precede repeat handling.
    assert!(!app.hint_keys_held.contains(&physical));
    app.modifiers = ModifiersState::empty();
    app.config = Config::default();
    app.dispatch_command(Command::OpenUrlHints);
    app.parser
        .advance(&mut app.terminal, b"\x1b[Hchanged URL")
        .unwrap();
    app.validate_url_hints();
    assert!(app.url_hints.is_none());
    assert_eq!(
        configured_command(
            &Config::default(),
            PhysicalKey::Code(KeyCode::KeyU),
            ModifiersState::CONTROL | ModifiersState::SHIFT
        ),
        Some(Command::OpenUrlHints)
    );
}

#[test]
fn unavailable_url_activation_preserves_tui_routing_and_explicit_replacements() {
    let mut app = Application::default();
    let key = PhysicalKey::Code(KeyCode::KeyU);
    app.modifiers = ModifiersState::CONTROL | ModifiersState::SHIFT;
    assert_eq!(app.terminal_command(key), Some(Command::OpenUrlHints));
    app.parser
        .advance(&mut app.terminal, b"\x1b[?1049h")
        .unwrap();
    assert_eq!(app.terminal_command(key), None);
    app.config = Config::parse("[[bindings]]\nkey = 'Ctrl+Shift+U'\ncommand = 'copy'").unwrap();
    assert_eq!(app.terminal_command(key), Some(Command::Copy));
    app.config = Config::default();
    app.parser
        .advance(&mut app.terminal, b"\x1b[?1049l\x1b[?1000h")
        .unwrap();
    assert_eq!(app.terminal_command(key), None);
    app.parser
        .advance(&mut app.terminal, b"\x1b[?1000l")
        .unwrap();
    assert_eq!(app.terminal_command(key), Some(Command::OpenUrlHints));
}

#[cfg(target_os = "linux")]
#[test]
fn hint_input_never_writes_to_real_pty() {
    use super::tests::{SmokeSession, SmokeTransport};
    // Reuse the existing PTY transport recorder; it observes actual writes.
    let config = PtySpawnConfig::new("/bin/cat".into(), PtySize::new(24, 80).unwrap());
    let session = shell::spawn(&PortablePtyBackend::new(), config, None).unwrap();
    let transport = Arc::new(std::sync::Mutex::new(SmokeTransport::default()));
    let worker = PtyWorker::start(SmokeSession {
        session,
        transport: Arc::clone(&transport),
    })
    .unwrap();
    let mut app = Application {
        pty: Some(worker),
        ..Application::default()
    };
    populate(&mut app);
    app.dispatch_command(Command::OpenUrlHints);
    for key in [
        Key::Character("l".into()),
        Key::Character("z".into()),
        Key::Named(NamedKey::Backspace),
        Key::Named(NamedKey::Escape),
    ] {
        app.handle_url_hint_key(
            &key,
            PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified),
            None,
            false,
        );
    }
    app.dispatch_command(Command::OpenUrlHints);
    app.parser
        .advance(&mut app.terminal, b"\x1b[Hchanged URL")
        .unwrap();
    let key = PhysicalKey::Code(KeyCode::KeyA);
    assert!(app.route_url_hint_key(&Key::Character("a".into()), key, Some("a"), false));
    assert!(app.url_hints.is_none());
    assert!(app.route_url_hint_key(&Key::Character("a".into()), key, Some("a"), true));
    // A barrier write ensures all previously queued input would have arrived.
    app.write_to_pty(b"barrier\n".to_vec());
    let deadline = Instant::now() + Duration::from_secs(2);
    while transport.lock().unwrap().writes_started.is_empty() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(
        transport.lock().unwrap().writes_started,
        [b"barrier\n".to_vec()]
    );
    app.pty.as_mut().unwrap().shutdown_and_join().unwrap();
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires installed Vim and Neovim; real PTY compatibility acceptance"]
fn native_url_hints_preserve_vim_and_neovim_input() {
    let path =
        std::env::temp_dir().join(format!("terminal-url-hints-tui-{}.txt", std::process::id()));
    std::fs::write(&path, "https://example.org/TUI\n").unwrap();
    for program in ["/usr/bin/vim", "/usr/bin/nvim"] {
        let config = PtySpawnConfig::new(program.into(), PtySize::new(24, 80).unwrap())
            .with_arguments(
                ["-u", "NONE", "-n", "-i", "NONE", "-c", "set mouse=a"]
                    .into_iter()
                    .map(OsString::from)
                    .chain(std::iter::once(path.clone().into_os_string())),
            )
            .with_environment([(OsString::from("TERM"), OsString::from("xterm-256color"))]);
        let session = shell::spawn(&PortablePtyBackend::new(), config, None).unwrap();
        let mut worker = PtyWorker::start(session).unwrap();
        let mut app = Application::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && app.terminal.active_screen() != ScreenKind::Alternate {
            if let Some(PtyWorkerEvent::Output(PtyOutput::Bytes(bytes))) =
                worker.recv_timeout(Duration::from_millis(50)).unwrap()
            {
                let replies = parse_terminal_output(&mut app.parser, &mut app.terminal, &bytes);
                for reply in replies {
                    worker.write(reply).unwrap();
                }
            }
        }
        assert_eq!(
            app.terminal.active_screen(),
            ScreenKind::Alternate,
            "{program} must enter the actual alternate screen"
        );
        app.modifiers = ModifiersState::CONTROL | ModifiersState::SHIFT;
        assert_eq!(app.terminal_command(PhysicalKey::Code(KeyCode::KeyU)), None);
        app.dispatch_command(Command::OpenUrlHints);
        assert!(app.url_hints.is_none());
        assert!(url_hints::detect(&app.terminal).is_empty());
        // Normal editing still reaches the application; no hint interception.
        worker.write(b"iTUI_INPUT\x1b:q!\r".to_vec()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && app.terminal.active_screen() == ScreenKind::Alternate {
            if let Some(PtyWorkerEvent::Output(PtyOutput::Bytes(bytes))) =
                worker.recv_timeout(Duration::from_millis(50)).unwrap()
            {
                for reply in parse_terminal_output(&mut app.parser, &mut app.terminal, &bytes) {
                    worker.write(reply).unwrap();
                }
            }
        }
        assert_eq!(
            app.terminal.active_screen(),
            ScreenKind::Primary,
            "{program} must accept input and exit alternate screen"
        );
        worker.shutdown_and_join().unwrap();
        eprintln!(
            "native URL hints: {program} alternate-screen guard and normal editing/quit passed"
        );
    }
    std::fs::remove_file(path).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires KDE Wayland and GPU; run alone"]
fn native_wayland_url_hints_theme_opacity_and_interaction() {
    use winit::platform::wayland::{EventLoopBuilderExtWayland, WindowExtWayland};
    struct Driver {
        app: Application,
        phase: usize,
        deadline: Instant,
        path: PathBuf,
        evidence: PathBuf,
        initial_settles: usize,
    }
    impl Driver {
        fn geometry(&self) {
            self.geometry_size(900, 620);
        }
        fn geometry_size(&self, width: u32, height: u32) {
            let script_path = self.path.with_extension("js");
            std::fs::write(&script_path, format!("for (const w of workspace.windowList()) {{ if (w.pid === {}) {{ w.frameGeometry = {{ x: 100, y: 100, width: {width}, height: {height} }}; w.keepAbove = true; workspace.activeWindow = w; }} }}", std::process::id())).unwrap();
            let plugin = format!("terminal-url-hints-{}", std::process::id());
            let output = std::process::Command::new("qdbus6")
                .args([
                    "org.kde.KWin",
                    "/Scripting",
                    "org.kde.kwin.Scripting.loadScript",
                ])
                .arg(&script_path)
                .arg(&plugin)
                .output()
                .unwrap();
            assert!(output.status.success());
            let id = String::from_utf8(output.stdout).unwrap().trim().to_owned();
            assert!(
                std::process::Command::new("qdbus6")
                    .args([
                        "org.kde.KWin",
                        &format!("/Scripting/Script{id}"),
                        "org.kde.kwin.Script.run"
                    ])
                    .status()
                    .unwrap()
                    .success()
            );
            assert!(
                std::process::Command::new("qdbus6")
                    .args([
                        "org.kde.KWin",
                        "/Scripting",
                        "org.kde.kwin.Scripting.unloadScript",
                        &plugin
                    ])
                    .status()
                    .unwrap()
                    .success()
            );
            std::fs::remove_file(script_path).unwrap();
        }
        fn configure(&mut self, index: usize) {
            let light = index >= 6;
            let opacity = [0.0, 0.85, 1.0][(index % 6) / 2];
            let blur = index % 2 == 1;
            let (bg, fg, selected, selected_fg) = if light {
                ("#f0f2f5", "#202731", "#d4e0ef", "#202731")
            } else {
                ("#15191f", "#dde2ea", "#334c6b", "#ffffff")
            };
            std::fs::write(&self.path, format!("[window]\nopacity = {opacity}\nblur = {blur}\n[theme]\nbackground = '{bg}'\nforeground = '{fg}'\nselection_background = '{selected}'\nselection_foreground = '{selected_fg}'")).unwrap();
            self.app.reload_config();
            self.app
                .window
                .as_ref()
                .unwrap()
                .set_title("Terminal URL Hint Mode acceptance");
            self.geometry();
            populate(&mut self.app);
            self.app.dispatch_command(Command::OpenUrlHints);
            assert_eq!(self.app.url_hints.as_ref().unwrap().candidates.len(), 14);
            eprintln!(
                "native URL hints theme={} opacity={opacity} blur={blur} scale={}",
                if light { "light" } else { "dark" },
                self.app.window.as_ref().unwrap().scale_factor()
            );
        }
        fn snapshot(&self, name: &str) {
            assert!(
                std::process::Command::new("spectacle")
                    .args(["-b", "-n", "-a", "-o"])
                    .arg(self.evidence.join(format!("{name}.png")))
                    .status()
                    .unwrap()
                    .success()
            );
        }
    }
    impl ApplicationHandler<PtyWake> for Driver {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.app.resumed(event_loop);
            assert!(self.app.window.as_ref().unwrap().xdg_toplevel().is_some());
            if let Some(mut pty) = self.app.pty.take() {
                pty.shutdown_and_join().unwrap();
            }
            self.app
                .window
                .as_ref()
                .unwrap()
                .set_title("Terminal URL Hint Mode acceptance");
            self.geometry();
        }
        fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
            // Capture tools change focus, and live user input must not alter
            // the scripted matrix. Exercise focus/input explicitly below.
            if !matches!(
                event,
                WindowEvent::Focused(_)
                    | WindowEvent::KeyboardInput { .. }
                    | WindowEvent::ModifiersChanged(_)
                    | WindowEvent::MouseInput { .. }
                    | WindowEvent::MouseWheel { .. }
                    | WindowEvent::CursorMoved { .. }
            ) {
                self.app.window_event(event_loop, id, event);
            }
        }
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            self.app.about_to_wait(event_loop);
            if Instant::now() >= self.deadline {
                match self.phase {
                    0 => self.configure(0),
                    1..=12 => {
                        if self.phase == 1 && self.app.url_hints.is_none() {
                            // Fractional-scale surface metadata can arrive after
                            // the first geometry request. Cancellation is correct;
                            // reactivate only after applying that initial resize.
                            self.initial_settles += 1;
                            assert!(self.initial_settles <= 8, "initial geometry did not settle");
                            self.app.apply_pending_resize();
                            populate(&mut self.app);
                            self.app.dispatch_command(Command::OpenUrlHints);
                            self.deadline = Instant::now() + Duration::from_millis(650);
                            event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
                            return;
                        }
                        assert!(
                            self.app.url_hints.is_some(),
                            "hint session must survive unchanged geometry before capture"
                        );
                        self.snapshot(&format!("theme-{:02}", self.phase - 1));
                        if self.phase < 12 {
                            self.configure(self.phase);
                        } else {
                            self.app.handle_url_hint_key(
                                &Key::Character("l".into()),
                                PhysicalKey::Code(KeyCode::KeyL),
                                Some("l"),
                                false,
                            );
                            assert_eq!(self.app.url_hints.as_ref().unwrap().visible().count(), 7);
                        }
                    }
                    13 => {
                        self.snapshot("filtered-L");
                        self.app.handle_url_hint_key(
                            &Key::Named(NamedKey::Backspace),
                            PhysicalKey::Code(KeyCode::Backspace),
                            None,
                            false,
                        );
                        assert_eq!(self.app.url_hints.as_ref().unwrap().visible().count(), 14);
                        self.app.handle_url_hint_key(
                            &Key::Named(NamedKey::Escape),
                            PhysicalKey::Code(KeyCode::Escape),
                            None,
                            false,
                        );
                        assert!(self.app.url_hints.is_none());
                        // Exercise the native pointer path while dragging: opening must be cancelled.
                        let rect = self.app.pane_rects().unwrap()[0].1;
                        let metrics = self.app.renderer.as_ref().unwrap().cell_metrics();
                        let position = PhysicalPosition::new(
                            rect.x as f64 + 12.0 * metrics.width() as f64,
                            rect.y as f64 + metrics.height() as f64 / 2.0,
                        );
                        self.app.pointer_position = Some(position);
                        self.app.modifiers = ModifiersState::CONTROL;
                        let id = self.app.window.as_ref().unwrap().id();
                        let device_id = winit::event::DeviceId::dummy();
                        self.app.window_event(
                            event_loop,
                            id,
                            WindowEvent::MouseInput {
                                device_id,
                                state: ElementState::Pressed,
                                button: MouseButton::Left,
                            },
                        );
                        assert!(self.app.url_click.is_some());
                        self.app.window_event(
                            event_loop,
                            id,
                            WindowEvent::CursorMoved {
                                device_id,
                                position: PhysicalPosition::new(position.x + 30.0, position.y),
                            },
                        );
                        assert!(self.app.url_click.is_none());
                        self.app.window_event(
                            event_loop,
                            id,
                            WindowEvent::MouseInput {
                                device_id,
                                state: ElementState::Released,
                                button: MouseButton::Left,
                            },
                        );
                        assert!(self.app.terminal.has_selection());
                        self.app.modifiers = ModifiersState::empty();
                        self.app.dispatch_command(Command::OpenUrlHints);
                        self.app
                            .parser
                            .advance(&mut self.app.terminal, b"\x1b[?1049hhttps://example.org")
                            .unwrap();
                        self.app.validate_url_hints();
                        assert!(self.app.url_hints.is_none());
                        self.app
                            .parser
                            .advance(&mut self.app.terminal, b"\x1b[?1049l")
                            .unwrap();
                        self.app.dispatch_command(Command::OpenUrlHints);
                    }
                    14 => {
                        self.app.dispatch_command(Command::OpenUrlHints);
                        // Real hint completion uses the platform's default browser handler.
                        self.app.handle_url_hint_key(
                            &Key::Character("a".into()),
                            PhysicalKey::Code(KeyCode::KeyA),
                            Some("a"),
                            false,
                        );
                        assert!(self.app.url_hints.is_none());
                        assert!(self.app.workspace_notice.is_none());
                        eprintln!(
                            "native URL hints: filter/backspace/escape, Ctrl-drag selection, alternate screen guard, and default-browser dispatch passed"
                        );
                    }
                    15 => {
                        self.geometry();
                        let rect = self.app.pane_rects().unwrap()[0].1;
                        let metrics = self.app.renderer.as_ref().unwrap().cell_metrics();
                        self.app.pointer_position = Some(PhysicalPosition::new(
                            rect.x as f64 + 12.0 * metrics.width() as f64,
                            rect.y as f64 + metrics.height() as f64 / 2.0,
                        ));
                        self.app.modifiers = ModifiersState::CONTROL;
                        let id = self.app.window.as_ref().unwrap().id();
                        let device_id = winit::event::DeviceId::dummy();
                        self.app.window_event(
                            event_loop,
                            id,
                            WindowEvent::MouseInput {
                                device_id,
                                state: ElementState::Pressed,
                                button: MouseButton::Left,
                            },
                        );
                        assert!(self.app.url_click.is_some());
                        self.app.window_event(
                            event_loop,
                            id,
                            WindowEvent::MouseInput {
                                device_id,
                                state: ElementState::Released,
                                button: MouseButton::Left,
                            },
                        );
                        assert!(self.app.url_click.is_none());
                        assert!(self.app.workspace_notice.is_none());
                        eprintln!("native Ctrl+Click default-browser dispatch passed");
                        self.app.modifiers = ModifiersState::empty();
                        self.app.dispatch_command(Command::SplitVertical);
                        if let Some(mut pty) = self.app.pty.take() {
                            pty.shutdown_and_join().unwrap();
                        }
                        populate(&mut self.app);
                        self.app.dispatch_command(Command::OpenUrlHints);
                    }
                    16 => {
                        assert_eq!(
                            self.app
                                .workspace
                                .active_tab()
                                .pane_rects(
                                    self.app.content_rect(
                                        self.app.window.as_ref().unwrap().inner_size()
                                    )
                                )
                                .len(),
                            2
                        );
                        assert_eq!(
                            self.app.url_hints.as_ref().unwrap().candidates,
                            url_hints::detect(&self.app.terminal)
                        );
                        self.snapshot("active-split-pane");
                        let id = self.app.window.as_ref().unwrap().id();
                        self.app
                            .window_event(event_loop, id, WindowEvent::Focused(false));
                        assert!(self.app.url_hints.is_none());
                        self.app
                            .window_event(event_loop, id, WindowEvent::Focused(true));
                        self.app.dispatch_command(Command::OpenUrlHints);
                        self.app.window_event(
                            event_loop,
                            id,
                            WindowEvent::MouseWheel {
                                device_id: winit::event::DeviceId::dummy(),
                                delta: MouseScrollDelta::LineDelta(0.0, 1.0),
                                phase: winit::event::TouchPhase::Moved,
                            },
                        );
                        assert!(self.app.url_hints.is_none());
                        self.app.dispatch_command(Command::OpenUrlHints);
                        self.geometry_size(800, 540);
                    }
                    17 => {
                        assert!(self.app.url_hints.is_none()); // Native resize changed terminal dimensions.
                        populate(&mut self.app);
                        self.app.dispatch_command(Command::OpenUrlHints);
                        self.app
                            .parser
                            .advance(&mut self.app.terminal, b"\x1b[?1000h")
                            .unwrap();
                        self.app.validate_url_hints();
                        assert!(self.app.url_hints.is_none());
                        assert!(self.app.url_at_pointer().is_none());
                        self.app
                            .parser
                            .advance(&mut self.app.terminal, b"\x1b[?1000l")
                            .unwrap();
                        eprintln!(
                            "native active-pane scope, focus/scroll/resize cancellation, and mouse-reporting guard passed"
                        );
                        event_loop.exit();
                    }
                    _ => unreachable!(),
                }
                self.phase += 1;
                self.deadline = Instant::now() + Duration::from_millis(650);
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
        }
    }
    let path = std::env::temp_dir().join(format!(
        "terminal-url-hints-native-{}.toml",
        std::process::id()
    ));
    let evidence = std::env::var_os("TERMINAL_URL_HINT_SCREENSHOTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terminal-url-hints-native"));
    std::fs::create_dir_all(&evidence).unwrap();
    let app = Application {
        config_path: path.clone(),
        ..Application::default()
    };
    let mut driver = Driver {
        app,
        phase: 0,
        deadline: Instant::now() + Duration::from_secs(1),
        path: path.clone(),
        evidence,
        initial_settles: 0,
    };
    let mut builder = EventLoop::<PtyWake>::with_user_event();
    builder.with_wayland().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        event_loop.run_app(&mut driver).unwrap()
    }));
    driver.app.renderer = None;
    driver.app.window_effects = None;
    driver.app.window = None;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    assert_eq!(driver.phase, 18);
    std::fs::remove_file(path).unwrap();
}
