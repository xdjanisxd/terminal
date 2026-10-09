# Current Task

Branch: `feat/linux-window-opacity`. Implementation complete. No commit or push.

Config validation, Linux alpha-capable windows, background-only projection,
premultiplied surface selection, background replacement draws, and sRGB
composition are implemented. Focused config/renderer tests and native GPU
readback passed. Normal Rust validation and release build passed. Native KDE
Wayland and XWayland transparency, reload, and window lifecycle were checked
with Vulkan / Intel Iris Xe. XWayland selection and UI acceptance passed.

See [plan](../LINUX_WINDOW_OPACITY_PLAN.md) and
[report](../LINUX_WINDOW_OPACITY.md). Remaining limits include native Wayland
keyboard acceptance, unavailable Xorg/Windows/macOS environments, and a
Wayland shutdown error reproduced in the unchanged baseline. Blur remains out
of scope; the report contains the alpha contract and handoff.
