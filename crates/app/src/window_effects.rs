//! Per-window compositor effects, independent of renderer and opacity state.

use std::sync::Arc;
use winit::event_loop::OwnedDisplayHandle;
use winit::window::Window;

#[cfg(target_os = "linux")]
mod wayland;

pub struct WindowEffects {
    #[cfg(target_os = "linux")]
    blur: Option<wayland::Blur>,
    #[cfg(target_os = "linux")]
    initialized: bool,
    #[cfg(target_os = "linux")]
    enabled: bool,
    #[cfg(target_os = "linux")]
    legacy_enabled: bool,
    // Drop the adapter before the foreign surface and display it borrows.
    #[cfg(target_os = "linux")]
    window: Arc<Window>,
    #[cfg(target_os = "linux")]
    display: OwnedDisplayHandle,
}

impl WindowEffects {
    pub fn new(window: Arc<Window>, display: OwnedDisplayHandle, enabled: bool) -> Self {
        #[cfg(target_os = "linux")]
        let mut effects = Self {
            blur: None,
            initialized: false,
            enabled: false,
            legacy_enabled: false,
            window,
            display,
        };
        #[cfg(not(target_os = "linux"))]
        let mut effects = {
            let _ = (window, display);
            Self {}
        };
        effects.set_blur(enabled);
        effects
    }

    pub fn set_blur(&mut self, enabled: bool) {
        #[cfg(target_os = "linux")]
        {
            use winit::platform::wayland::WindowExtWayland;
            if self.window.xdg_toplevel().is_none() {
                return;
            }
            self.enabled = enabled;
            if enabled && !self.initialized {
                self.initialized = true;
                self.blur = wayland::Blur::new(&self.window, &self.display);
            }
            self.dispatch_pending();
        }
        #[cfg(not(target_os = "linux"))]
        let _ = enabled;
    }

    /// winit reads the shared socket; only dispatch our queue, never block here.
    pub fn dispatch_pending(&mut self) {
        #[cfg(target_os = "linux")]
        {
            let mut successor = false;
            if let Some(blur) = &mut self.blur {
                let changed = blur.update(self.enabled);
                successor = blur.available();
                if changed {
                    self.window.request_redraw();
                }
            }
            let legacy_enabled = self.enabled && !successor;
            if legacy_enabled != self.legacy_enabled {
                self.window.set_blur(legacy_enabled);
                self.legacy_enabled = legacy_enabled;
                self.window.request_redraw();
            }
        }
    }
}
