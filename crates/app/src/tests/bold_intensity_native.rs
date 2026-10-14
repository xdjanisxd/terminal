//! Run alone on KDE Wayland with the installed JetBrains Mono Nerd Font family.
use super::*;
use std::fs;
use winit::platform::wayland::{EventLoopBuilderExtWayland, WindowExtWayland};

#[test]
#[ignore = "requires KDE Wayland, GPU and JetBrainsMono Nerd Font Mono"]
fn native_wayland_bold_intensity_reload_size_and_recreation() {
    struct Driver {
        app: Application,
        path: PathBuf,
        phase: usize,
        deadline: Instant,
        metrics: Option<terminal_renderer::CellMetrics>,
    }
    impl Driver {
        fn reload(&mut self, value: &str) {
            fs::write(&self.path, format!("[font]\nfamily = 'JetBrainsMono Nerd Font Mono'\nsize = 16\nbold_intensity = {value}\n")).unwrap();
            self.app.reload_config();
        }

        fn capture(&self, name: &str) {
            if let Some(directory) = std::env::var_os("TERMINAL_BOLD_TEST_SCREENSHOTS") {
                let directory = PathBuf::from(directory);
                fs::create_dir_all(&directory).unwrap();
                assert!(
                    std::process::Command::new("timeout")
                        .args(["10", "spectacle", "-i", "-a", "-b", "-n", "-o"])
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
            self.metrics = Some(self.app.renderer.as_ref().unwrap().cell_metrics());
        }

        fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
            self.app.window_event(event_loop, id, event);
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            self.app.about_to_wait(event_loop);
            if Instant::now() >= self.deadline {
                match self.phase {
                    0 => {
                        self.metrics = Some(self.app.renderer.as_ref().unwrap().cell_metrics());
                        self.capture("startup-075");
                        self.reload("1.25");
                        assert_eq!(self.app.config.font.bold_intensity, 1.25);
                        assert_eq!(
                            self.app.renderer.as_ref().unwrap().cell_metrics(),
                            self.metrics.unwrap()
                        );
                    }
                    1 => {
                        self.capture("reload-still-075");
                        self.reload("nan");
                        assert_eq!(self.app.config.font.bold_intensity, 1.25);
                        self.app.dispatch_command(Command::IncreaseFontSize);
                        assert_eq!(self.app.font_size, 17);
                    }
                    2 => {
                        self.capture("size-17");
                        self.app.dispatch_command(Command::ResetFontSize);
                        assert_eq!(self.app.font_size, 16);
                        assert_eq!(
                            self.app.renderer.as_ref().unwrap().cell_metrics(),
                            self.metrics.unwrap()
                        );
                    }
                    3 => {
                        self.capture("reset-16");
                        let id = self.app.window.as_ref().unwrap().id();
                        self.app.suspended(event_loop);
                        assert!(self.app.renderer.is_none());
                        self.app.resumed(event_loop);
                        assert_eq!(self.app.window.as_ref().unwrap().id(), id);
                        assert_eq!(
                            self.app.renderer.as_ref().unwrap().cell_metrics(),
                            self.metrics.unwrap()
                        );
                    }
                    4 => {
                        self.capture("recreated-125");
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
        }
    }
    let path = std::env::temp_dir().join(format!(
        "terminal-bold-intensity-{}.toml",
        std::process::id()
    ));
    let app = Application {
        config_path: path.clone(),
        ..Application::default()
    };
    let mut driver = Driver {
        app,
        path: path.clone(),
        phase: 0,
        deadline: Instant::now() + Duration::from_secs(2),
        metrics: None,
    };
    driver.reload("0.75");
    driver.app.parser.advance(&mut driver.app.terminal, "\x1b[?25lRegular Ag0@ é e\u{301} ┌─┐ 日本語 \u{e0b0}\r\n\x1b[1mBold Ag0@ é e\u{301} ┌─┐ 日本語 \u{e0b0}\r\n\x1b[22mReset Ag0@\r\n".as_bytes()).unwrap();
    let mut builder = EventLoop::<PtyWake>::with_user_event();
    builder.with_wayland().with_any_thread(true);
    let event_loop = builder.build().unwrap();
    event_loop.run_app(&mut driver).unwrap();
    fs::remove_file(path).unwrap();
}
