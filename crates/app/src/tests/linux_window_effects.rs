//! Explicit native acceptance test: one Wayland event loop per test process.

use super::*;
use winit::platform::wayland::{EventLoopBuilderExtWayland, WindowExtWayland};

#[test]
#[ignore = "requires a native Wayland compositor and GPU; run this test alone"]
fn native_wayland_blur_reload_recreation_and_shutdown() {
    struct Driver {
        app: Application,
        phase: usize,
        deadline: Instant,
        path: PathBuf,
    }

    impl Driver {
        fn reload(&mut self, source: &str) {
            std::fs::write(&self.path, source).unwrap();
            self.app.reload_config();
        }

        fn snapshot(&self, name: &str) {
            // Optional acceptance evidence; never used by the shipped app.
            if let Some(directory) = std::env::var_os("TERMINAL_BLUR_TEST_SCREENSHOTS") {
                let directory = PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                assert!(
                    std::process::Command::new("spectacle")
                        .args(["-b", "-n", "-o"])
                        .arg(directory.join(format!("{name}.png")))
                        .status()
                        .unwrap()
                        .success()
                );
            }
        }
    }

    impl ApplicationHandler<PtyWake> for Driver {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.app.resumed(event_loop);
            assert!(self.app.window.as_ref().unwrap().xdg_toplevel().is_some());
            assert!(self.app.renderer.is_some());
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
            self.app.window_event(event_loop, id, event);
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            self.app.about_to_wait(event_loop);
            if Instant::now() >= self.deadline {
                match self.phase {
                    0 => {
                        self.snapshot("initial-blur");
                        self.reload("[window]\nopacity = 0.0\nblur = false");
                        assert!(!self.app.config.window.blur);
                        assert_eq!(self.app.config.window.opacity, 0.0);
                    }
                    1 => {
                        self.reload("[window]\nopacity = 0.85\nblur = true");
                        self.reload("[window]\nopacity = 0.0\nblur = 'invalid'");
                        assert!(self.app.config.window.blur);
                        assert_eq!(self.app.config.window.opacity, 0.85);
                        for _ in 0..4 {
                            self.app.dispatch_command(Command::SelectLeft);
                        }
                    }
                    2 => {
                        self.snapshot("selection-blur");
                        self.app.dispatch_command(Command::OpenPalette);
                    }
                    3 => {
                        self.snapshot("palette-blur");
                        let id = self.app.window.as_ref().unwrap().id();
                        self.app.suspended(event_loop);
                        assert!(self.app.renderer.is_none());
                        self.app.resumed(event_loop);
                        assert_eq!(self.app.window.as_ref().unwrap().id(), id);
                        assert!(self.app.renderer.is_some());
                    }
                    4 => {
                        self.snapshot("surface-recreated");
                        let id = self.app.window.as_ref().unwrap().id();
                        self.app.renderer = None;
                        self.app.window = None;
                        self.app.create_window_and_renderer(event_loop);
                        assert_ne!(self.app.window.as_ref().unwrap().id(), id);
                        assert!(self.app.renderer.is_some());
                    }
                    5 => {
                        self.snapshot("window-recreated");
                        event_loop.exit();
                    }
                    _ => unreachable!(),
                }
                self.phase += 1;
                self.deadline = Instant::now() + Duration::from_secs(1);
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
        }

        fn exiting(&mut self, event_loop: &ActiveEventLoop) {
            self.app.exiting(event_loop);
            assert!(self.app.renderer.is_none());
            assert!(self.app.window.is_none());
        }
    }

    let path =
        std::env::temp_dir().join(format!("terminal-native-blur-{}.toml", std::process::id()));
    let mut app = Application {
        config_path: path.clone(),
        ..Application::default()
    };
    std::fs::write(&path, "[window]\nopacity = 0.85\nblur = true").unwrap();
    app.reload_config();
    app.parser
        .advance(
            &mut app.terminal,
            b"\x1b[31mRed foreground\x1b[0m \x1b[44mBlue background\x1b[0m\r\nSelect this line",
        )
        .unwrap();
    let mut builder = EventLoop::<PtyWake>::with_user_event();
    builder.with_wayland().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    let mut driver = Driver {
        app,
        phase: 0,
        deadline: Instant::now() + Duration::from_secs(1),
        path: path.clone(),
    };
    event_loop.run_app(&mut driver).unwrap();
    assert_eq!(driver.phase, 6);
    std::fs::remove_file(path).unwrap();
}
