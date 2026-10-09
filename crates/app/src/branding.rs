//! Application identity belongs to the native app, not terminal semantics.

use winit::window::{Window, WindowAttributes};

pub fn window_attributes() -> WindowAttributes {
    let attributes = Window::default_attributes().with_title("Terminal");

    #[cfg(target_os = "linux")]
    let attributes = {
        use winit::platform::wayland::WindowAttributesExtWayland;

        // Winit shares this identity with its X11 backend (WM_CLASS).
        let app_id = "io.github.xdjanisxd.terminal";
        // X11 needs an alpha visual at creation, including for later config reloads.
        attributes.with_name(app_id, app_id).with_transparent(true)
    };

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

    attributes
}
