# Terminal branding

The source PNG and Windows ICO are reused unchanged from `feat/app-branding`.
The ICO is embedded in `terminal.exe` and reused by the MSI and Start Menu
shortcut. Keep these uses aligned; do not introduce separate installer branding.

The source artwork is retained under `source/terminal.png`. Windows packaging
validates executable icon resources against `windows/terminal.ico`.
