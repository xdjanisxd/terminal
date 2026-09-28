//! Application identity belongs to the native app, not terminal semantics.

use winit::window::{Window, WindowAttributes};

pub fn window_attributes() -> WindowAttributes {
    let attributes = Window::default_attributes().with_title("Terminal");

    #[cfg(target_os = "windows")]
    let attributes = {
        use winit::platform::windows::{IconExtWindows, WindowAttributesExtWindows};
        use winit::window::Icon;

        match Icon::from_resource(1, Some(winit::dpi::PhysicalSize::new(256, 256))) {
            Ok(icon) => attributes
                .with_window_icon(Some(icon.clone()))
                .with_taskbar_icon(Some(icon)),
            Err(error) => {
                eprintln!("could not load Terminal application icon: {error}");
                attributes
            }
        }
    };

    #[cfg(target_os = "linux")]
    let attributes = {
        const APP_ID: &str = "io.github.xdjanisxd.terminal";
        let attributes = winit::platform::wayland::WindowAttributesExtWayland::with_name(
            attributes, APP_ID, "terminal",
        );
        winit::platform::x11::WindowAttributesExtX11::with_name(attributes, APP_ID, "terminal")
    };

    attributes
}
