//! Run alone on KDE Wayland; commands use the real app handlers and native renderer.
use super::*;
use winit::platform::wayland::{EventLoopBuilderExtWayland, WindowExtWayland};

#[test]
#[ignore = "requires KDE Wayland and a GPU; run this test alone"]
fn native_wayland_palette_theme_opacity_and_interaction() {
    native_panel_acceptance(false);
}

#[test]
#[ignore = "requires KDE Wayland and a GPU; run this test alone"]
fn native_wayland_tab_picker_theme_opacity_and_interaction() {
    native_panel_acceptance(true);
}

fn native_panel_acceptance(tab_picker: bool) {
    struct Driver {
        app: Application,
        tab_picker: bool,
        background: Option<(Arc<Window>, Renderer)>,
        phase: usize,
        deadline: Instant,
        path: PathBuf,
        capture_position: [u32; 2],
        fractional: Option<[u32; 4]>,
    }
    impl Driver {
        fn geometry(&mut self, x: u32, y: u32, width: u32, height: u32) {
            self.capture_position = [x, y];
            let marker = format!("Terminal palette acceptance {}", std::process::id());
            self.app.window.as_ref().unwrap().set_title(&marker);
            let path = self.path.with_extension("js");
            let background = if let Some([x, y, width, height]) = self.fractional {
                format!(
                    "if (w.caption.indexOf('Palette contrast fixture {}') === 0) {{ w.setMaximize(false, false); w.frameGeometry = {{ x: {x}, y: {y}, width: {width}, height: {height} }}; w.setMaximize(true, true); }}",
                    std::process::id()
                )
            } else {
                String::new()
            };
            std::fs::write(&path, format!(
                "for (const w of workspace.windowList()) {{ {background} if (w.pid === {} && w.caption.indexOf('Palette contrast fixture') !== 0) {{ w.frameGeometry = {{ x: {x}, y: {y}, width: {width}, height: {height} }}; w.keepAbove = true; workspace.activeWindow = w; }} }}",
                std::process::id()
            )).unwrap();
            let plugin = format!("terminal-palette-geometry-{}", std::process::id());
            let output = std::process::Command::new("qdbus6")
                .args([
                    "org.kde.KWin",
                    "/Scripting",
                    "org.kde.kwin.Scripting.loadScript",
                ])
                .arg(&path)
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
            std::fs::remove_file(path).unwrap();
        }
        fn configure(&mut self, index: usize) {
            let light = index >= 8;
            let opacity = [0.0, 0.5, 0.85, 1.0][(index % 8) / 2];
            let blur = index % 2 == 1;
            let (bg, fg, selected, selected_fg) = if light {
                ("#f0f2f5", "#202731", "#d4e0ef", "#202731")
            } else {
                ("#15191f", "#dde2ea", "#334c6b", "#ffffff")
            };
            std::fs::write(&self.path, format!(
                "[window]\nopacity = {opacity}\nblur = {blur}\n[theme]\nbackground = '{bg}'\nforeground = '{fg}'\nselection_background = '{selected}'\nselection_foreground = '{selected_fg}'"
            )).unwrap();
            self.app.reload_config();
            assert_eq!(self.app.config.window.opacity, opacity);
            assert_eq!(self.app.config.window.blur, blur);
            if let Some(window) = &self.app.window {
                let size = window.inner_size();
                let scale = window.scale_factor();
                self.geometry(
                    self.capture_position[0],
                    self.capture_position[1],
                    (size.width as f64 / scale).round() as u32,
                    (size.height as f64 / scale).round() as u32,
                );
            }
        }
        fn snapshot(&self, name: &str) {
            let Some(directory) = std::env::var_os("TERMINAL_PALETTE_TEST_SCREENSHOTS") else {
                return;
            };
            let directory = PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            let marker = format!("Terminal palette acceptance {}", std::process::id());
            self.app.window.as_ref().unwrap().set_title(&marker);
            // KDE cannot grant winit activation without an input token. Restrict
            // this temporary script to this test process's two native windows.
            let script = directory.join("capture.js");
            std::fs::write(&script, format!(
                "for (const w of workspace.windowList()) {{ if (w.pid === {}) {{ w.desktops = [workspace.currentDesktop]; w.minimized = false; if (w.caption.indexOf('Palette contrast fixture') !== 0) {{ w.keepAbove = true; }} }} }}",
                std::process::id())).unwrap();
            let plugin = format!("terminal-palette-capture-{}", std::process::id());
            let output = std::process::Command::new("qdbus6")
                .args([
                    "org.kde.KWin",
                    "/Scripting",
                    "org.kde.kwin.Scripting.loadScript",
                ])
                .arg(&script)
                .arg(&plugin)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
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
            let raw = directory.join("compositor.png");
            assert!(
                std::process::Command::new("spectacle")
                    .args(["-b", "-n", "-f", "-d", "200", "-o"])
                    .arg(&raw)
                    .status()
                    .unwrap()
                    .success()
            );
            let window = self.app.window.as_ref().unwrap();
            let size = window.inner_size();
            let (x, y) = if window.is_maximized() {
                (0, 0)
            } else {
                (self.capture_position[0], self.capture_position[1])
            };
            let capture_scale: u32 = std::env::var("TERMINAL_PALETTE_CAPTURE_SCALE")
                .unwrap_or_else(|_| "1".into())
                .parse()
                .unwrap();
            assert!(
                std::process::Command::new("magick")
                    .arg(&raw)
                    .args([
                        "-crop",
                        &format!(
                            "{}x{}+{}+{}",
                            (size.width as f64 / window.scale_factor()).round() as u32
                                * capture_scale,
                            (size.height as f64 / window.scale_factor()).round() as u32
                                * capture_scale,
                            x * capture_scale,
                            y * capture_scale
                        ),
                        "+repage",
                        "-resize",
                        &format!("{}x{}", size.width, size.height),
                    ])
                    .arg(directory.join(format!("{name}.png")))
                    .status()
                    .unwrap()
                    .success()
            );
            std::fs::remove_file(raw).unwrap();
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
        }
        fn open(&mut self) {
            self.app.dispatch_command(if self.tab_picker {
                Command::OpenTabPicker
            } else {
                Command::OpenPalette
            });
        }
        fn state(&self) -> &Palette {
            if self.tab_picker {
                self.app.tab_picker.as_ref()
            } else {
                self.app.palette.as_ref()
            }
            .unwrap()
        }
        fn key(&mut self, key: NamedKey) {
            if self.tab_picker {
                self.app.handle_tab_picker_key(&Key::Named(key), None);
            } else {
                self.app.handle_palette_key(&Key::Named(key), None);
            }
        }
        fn query(&mut self, query: &str) {
            if self.tab_picker {
                self.app
                    .handle_tab_picker_key(&Key::Character(query.into()), Some(query));
            } else {
                self.app
                    .handle_palette_key(&Key::Character(query.into()), Some(query));
            }
        }
    }
    impl ApplicationHandler<PtyWake> for Driver {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title(format!("Palette contrast fixture {}", std::process::id()))
                            .with_maximized(true),
                    )
                    .unwrap(),
            );
            assert!(window.xdg_toplevel().is_some());
            let renderer = Renderer::new(Arc::clone(&window)).unwrap();
            window.request_redraw();
            self.background = Some((window, renderer));
            self.app.resumed(event_loop);
            let window = self.app.window.as_ref().unwrap();
            assert!(window.xdg_toplevel().is_some());
            window.set_title("Terminal palette acceptance");
            window.focus_window();
            let _ = window.request_inner_size(PhysicalSize::new(800, 600));
            self.app.parser.advance(&mut self.app.terminal,
                b"\x1b[31mPalette acceptance\x1b[0m\r\nForeground remains sharp over a detailed desktop.\r\n\x1b[44mANSI background\x1b[0m  Ctrl+Shift+P / Enter / Escape\r\n").unwrap();
            self.app.invalidate_frame();
            self.geometry(200, 100, 800, 600);
        }
        fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
            if let Some((window, renderer)) = &mut self.background
                && id == window.id()
            {
                match event {
                    WindowEvent::Resized(size) => {
                        renderer.resize(size.width, size.height);
                        window.request_redraw();
                    }
                    WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                        renderer.set_scale_factor(scale_factor).unwrap();
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        let size = window.inner_size();
                        let metrics = renderer.cell_metrics();
                        let columns = (size.width / metrics.width()).max(1) as usize;
                        let rows = (size.height / metrics.height()).max(1) as usize;
                        let mut terminal =
                            TerminalState::new(TerminalDimensions::new(columns, rows).unwrap());
                        let mut source = String::new();
                        for row in 0..rows {
                            for col in 0..columns {
                                let color = if (col / 2 + row) % 2 == 0 {
                                    "240;190;100"
                                } else {
                                    "40;75;115"
                                };
                                source.push_str(&format!(
                                    "\x1b[{};{}H\x1b[48;2;{color}m ",
                                    row + 1,
                                    col + 1
                                ));
                            }
                        }
                        TerminalParser::default()
                            .advance(&mut terminal, source.as_bytes())
                            .unwrap();
                        renderer.redraw_terminal(&terminal);
                    }
                    _ => {}
                }
                return;
            }
            // Keep unrelated live user input out of the scripted test matrix.
            if !matches!(
                event,
                WindowEvent::KeyboardInput { .. }
                    | WindowEvent::MouseInput { .. }
                    | WindowEvent::MouseWheel { .. }
                    | WindowEvent::CursorMoved { .. }
                    | WindowEvent::ModifiersChanged(_)
            ) {
                self.app.window_event(event_loop, id, event);
            }
        }
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            self.app.about_to_wait(event_loop);
            if Instant::now() < self.deadline {
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
                return;
            }
            // Capture only after the app has presented the changed configuration.
            // Wayland frame callbacks can be delayed while a capture tool runs.
            if self.app.frame.dirty {
                assert!(
                    Instant::now() < self.deadline + Duration::from_secs(5),
                    "phase {} did not present its changed configuration",
                    self.phase
                );
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    Instant::now() + Duration::from_millis(100),
                ));
                return;
            }
            match self.phase {
                0 => {
                    self.geometry(200, 100, 800, 600);
                    self.snapshot("closed-dark-zero");
                    assert_eq!(
                        configured_command(
                            &self.app.config,
                            PhysicalKey::Code(if self.tab_picker {
                                KeyCode::Space
                            } else {
                                KeyCode::KeyP
                            }),
                            ModifiersState::CONTROL | ModifiersState::SHIFT
                        ),
                        Some(if self.tab_picker {
                            Command::OpenTabPicker
                        } else {
                            Command::OpenPalette
                        })
                    );
                    if self.tab_picker {
                        self.app
                            .workspace
                            .set_active_tab_custom_title(Some("Fixture tab 00".into()));
                        for index in 1..30 {
                            self.app.dispatch_command(Command::NewTab);
                            self.app.workspace.set_active_tab_custom_title(Some(format!(
                                "Fixture tab {index:02} {}",
                                "界e\u{301} long title ".repeat(8)
                            )));
                        }
                        let pane = self.app.workspace.tabs()[0].active_pane;
                        self.app.record_background_output(pane);
                    }
                    self.open();
                    self.query("a");
                    self.key(NamedKey::ArrowUp);
                    assert_eq!(self.state().selected(), 0);
                    self.key(NamedKey::ArrowDown);
                    assert_eq!(self.state().selected(), 1);
                    self.key(NamedKey::ArrowUp);
                    assert_eq!(self.state().selected(), 0);
                    for _ in 0..12 {
                        self.key(NamedKey::ArrowDown);
                    }
                }
                1..=16 => {
                    let i = self.phase - 1;
                    assert_eq!(self.state().selected(), 12);
                    assert_eq!(self.state().query(), "a");
                    self.snapshot(&format!(
                        "{}-{}-{}",
                        if i < 8 { "dark" } else { "light" },
                        ["0", "05", "085", "1"][(i % 8) / 2],
                        if i % 2 == 1 { "blur" } else { "sharp" }
                    ));
                    if self.phase < 16 {
                        self.configure(self.phase);
                    } else {
                        self.configure(5);
                        std::fs::write(&self.path, "[window]\nopacity = nan").unwrap();
                        self.app.reload_config();
                        assert_eq!(self.app.config.window.opacity, 0.85);
                        self.query("zzzz-no-match");
                        if !self.tab_picker {
                            self.key(NamedKey::Enter);
                            assert!(self.app.panel_open());
                        }
                    }
                }
                17 => {
                    self.snapshot("no-matches");
                    if self.tab_picker {
                        let pane = self.app.workspace.active_pane();
                        self.key(NamedKey::Enter);
                        assert!(!self.app.panel_open());
                        assert_eq!(self.app.workspace.active_pane(), pane);
                    }
                    self.key(NamedKey::Escape);
                    let active = self.app.workspace.active_pane();
                    if self.tab_picker {
                        self.open();
                        self.query("Fixture tab 01");
                        self.key(NamedKey::Enter);
                        assert!(self.app.active_tab_title().starts_with("Fixture tab 01"));
                        self.app.dispatch_command(Command::SplitVertical);
                    } else {
                        self.open();
                        self.query("split vertical");
                        self.key(NamedKey::Enter);
                    }
                    assert!(!self.app.panel_open());
                    assert_eq!(
                        self.app
                            .workspace
                            .active_tab()
                            .pane_rects(
                                self.app
                                    .content_rect(self.app.window.as_ref().unwrap().inner_size())
                            )
                            .len(),
                        2
                    );
                    assert_ne!(self.app.workspace.active_pane(), active);
                }
                18 => {
                    self.snapshot("split-command-executed");
                    self.open();
                    for _ in 0..25 {
                        self.key(NamedKey::ArrowDown);
                    }
                    let pane = self.app.workspace.active_pane();
                    let selection = self.app.terminal.selected_text();
                    let offset = self.app.terminal.viewport_offset();
                    let id = self.app.window.as_ref().unwrap().id();
                    let device_id = winit::event::DeviceId::dummy();
                    self.app.window_event(
                        event_loop,
                        id,
                        WindowEvent::CursorMoved {
                            device_id,
                            position: PhysicalPosition::new(15.0, 16.0),
                        },
                    );
                    assert_eq!(self.app.chrome.hovered, Some(0));
                    self.app.window_event(
                        event_loop,
                        id,
                        WindowEvent::CursorMoved {
                            device_id,
                            position: PhysicalPosition::new(20.0, 200.0),
                        },
                    );
                    assert!(self.app.chrome.hovered.is_none());
                    for state in [ElementState::Pressed, ElementState::Released] {
                        self.app.window_event(
                            event_loop,
                            id,
                            WindowEvent::MouseInput {
                                device_id,
                                state,
                                button: MouseButton::Left,
                            },
                        );
                    }
                    self.app.window_event(
                        event_loop,
                        id,
                        WindowEvent::MouseWheel {
                            device_id,
                            delta: MouseScrollDelta::LineDelta(0.0, 3.0),
                            phase: winit::event::TouchPhase::Moved,
                        },
                    );
                    self.app
                        .window_event(event_loop, id, WindowEvent::Focused(false));
                    self.app
                        .window_event(event_loop, id, WindowEvent::Focused(true));
                    assert_eq!(self.app.workspace.active_pane(), pane);
                    assert_eq!(self.app.terminal.selected_text(), selection);
                    assert_eq!(self.app.terminal.viewport_offset(), offset);
                    assert!(self.app.panel_open());
                }
                19 => {
                    self.snapshot("split-window-palette");
                    self.geometry(200, 100, 320, 190);
                }
                20 => {
                    assert_eq!(
                        self.app
                            .renderer
                            .as_ref()
                            .unwrap()
                            .diagnostic_state()
                            .configured_size(),
                        SurfaceSize::new(320, 190)
                    );
                    self.snapshot("small-window");
                    self.geometry(200, 100, 800, 600);
                    self.app.window.as_ref().unwrap().set_maximized(true);
                }
                21 => {
                    self.snapshot("maximized");
                    self.app.window.as_ref().unwrap().set_maximized(false);
                }
                22 => {
                    assert_eq!(
                        self.app
                            .renderer
                            .as_ref()
                            .unwrap()
                            .diagnostic_state()
                            .configured_size(),
                        SurfaceSize::new(800, 600)
                    );
                    self.snapshot("restored");
                    self.key(NamedKey::Escape);
                    let pane = self.app.workspace.active_pane();
                    for _ in 0..3 {
                        self.open();
                        self.key(NamedKey::Escape);
                        assert_eq!(self.app.workspace.active_pane(), pane);
                    }
                    self.open();
                    self.query(if self.tab_picker {
                        "Fixture tab 00"
                    } else {
                        "new tab"
                    });
                    self.key(NamedKey::Enter);
                    assert!(!self.app.panel_open());
                    assert_eq!(
                        self.app.workspace.tabs().len(),
                        if self.tab_picker { 30 } else { 2 }
                    );
                    if self.tab_picker {
                        assert_eq!(self.app.active_tab_title(), "Fixture tab 00");
                    }
                }
                23 => {
                    self.snapshot("new-tab-executed");
                    if let Some(monitor) = self
                        .app
                        .window
                        .as_ref()
                        .unwrap()
                        .available_monitors()
                        // Wayland monitor metadata reports integer backing scale;
                        // the window's fractional scale arrives after the move.
                        .find(|m| m.scale_factor() > 1.0)
                    {
                        let scale = monitor.scale_factor();
                        let position = monitor.position();
                        let size = monitor.size();
                        let x = (position.x as f64 / scale).round() as u32;
                        let y = (position.y as f64 / scale).round() as u32;
                        self.fractional = Some([
                            x,
                            y,
                            (size.width as f64 / scale).round() as u32,
                            (size.height as f64 / scale).round() as u32,
                        ]);
                        self.configure(5);
                        self.open();
                        self.geometry(x + 100, y + 100, 600, 500);
                    } else {
                        event_loop.exit();
                    }
                }
                24 => {
                    assert!(self.app.window.as_ref().unwrap().scale_factor() > 1.0);
                    assert!(self.app.panel_open());
                    self.snapshot("fractional-scale-blur");
                    self.configure(13);
                }
                25 => {
                    self.snapshot("fractional-scale-light");
                    self.key(NamedKey::Escape);
                    event_loop.exit();
                }
                _ => unreachable!(),
            }
            self.phase += 1;
            self.deadline = Instant::now() + Duration::from_millis(900);
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
        }
        fn exiting(&mut self, event_loop: &ActiveEventLoop) {
            self.app.exiting(event_loop);
            self.background = None;
        }
    }
    let path = std::env::temp_dir().join(format!(
        "terminal-palette-native-{}.toml",
        std::process::id()
    ));
    let app = Application {
        config_path: path.clone(),
        ..Application::default()
    };
    let mut driver = Driver {
        app,
        tab_picker,
        background: None,
        phase: 0,
        deadline: Instant::now() + Duration::from_secs(1),
        path: path.clone(),
        capture_position: [200, 100],
        fractional: None,
    };
    driver.configure(0);
    let mut builder = EventLoop::<PtyWake>::with_user_event();
    builder.with_wayland().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        event_loop.run_app(&mut driver).unwrap();
    }));
    // Also release GPU owners before the Wayland event loop on assertion failure.
    driver.app.renderer = None;
    driver.background = None;
    driver.app.window = None;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    assert!(matches!(driver.phase, 24 | 26));
    eprintln!(
        "native panel acceptance: tab_picker={tab_picker} phases={} fractional_scale={}",
        driver.phase,
        driver.fractional.is_some()
    );
    std::fs::remove_file(path).unwrap();
}
