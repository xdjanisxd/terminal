//! Native application lifecycle and component wiring.

use std::sync::Arc;

use terminal_core::{TerminalDimensions, TerminalState};
use terminal_renderer::{RedrawOutcome, Renderer};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct Application {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    terminal: TerminalState,
}

impl Default for Application {
    fn default() -> Self {
        Self {
            window: None,
            renderer: None,
            terminal: TerminalState::new(TerminalDimensions::new(80, 24).expect("valid default")),
        }
    }
}

impl Application {
    fn create_window_and_renderer(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attributes = Window::default_attributes().with_title("Terminal");
            match event_loop.create_window(attributes) {
                Ok(window) => self.window = Some(Arc::new(window)),
                Err(error) => {
                    eprintln!("could not create terminal window: {error}");
                    event_loop.exit();
                    return;
                }
            }
        }

        let window = self.window.as_ref().unwrap();
        match Renderer::new(Arc::clone(window)) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
                window.request_redraw();
            }
            Err(error) => {
                eprintln!("could not initialize terminal renderer: {error}");
                event_loop.exit();
            }
        }
    }
}

impl ApplicationHandler for Application {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.renderer.is_none() {
            self.create_window_and_renderer(event_loop);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.renderer = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if self
                    .renderer
                    .as_mut()
                    .is_some_and(|renderer| renderer.resize(size.width, size.height))
                {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                match renderer.redraw_terminal(&self.terminal) {
                    RedrawOutcome::Reconfigured => window.request_redraw(),
                    RedrawOutcome::Exit => event_loop.exit(),
                    RedrawOutcome::Presented | RedrawOutcome::Skipped => {}
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("could not create terminal event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop
        .run_app(&mut Application::default())
        .expect("terminal event loop failed");
}
