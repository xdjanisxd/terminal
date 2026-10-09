//! Native window effects through winit; no renderer or protocol object ownership.

use winit::window::Window;

/// Request compositor blur on Linux Wayland. The KDE protocol has no success
/// acknowledgement; unsupported compositors keep the existing opacity behavior.
pub fn set_blur(window: &Window, enabled: bool) {
    #[cfg(target_os = "linux")]
    {
        use winit::platform::wayland::WindowExtWayland;

        if window.xdg_toplevel().is_some() {
            // winit owns the connection and blur object, including release on
            // disable/drop. The default whole-surface region follows geometry.
            window.set_blur(enabled);
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (window, enabled);
}
