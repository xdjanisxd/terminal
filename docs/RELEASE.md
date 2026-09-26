# Portable releases

The `Portable release` GitHub Actions workflow verifies, builds, packages, and smoke-tests native binaries. Every archive contains exactly `terminal` (`terminal.exe` on Windows), `README.md`, `config.example.toml`, and `LICENSE`. The example config is reference material and is not installed as an active `config.toml`.

| Runner | Rust target | Release asset |
| --- | --- | --- |
| `windows-2025` | `x86_64-pc-windows-msvc` | `terminal-windows-x86_64.zip` |
| `windows-11-vs2026-arm` | `aarch64-pc-windows-msvc` | `terminal-windows-aarch64.zip` |
| `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | `terminal-linux-x86_64.tar.gz` |
| `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | `terminal-linux-aarch64.tar.gz` |
| `macos-15-intel` | `x86_64-apple-darwin` | `terminal-macos-x86_64.tar.gz` |
| `macos-15` | `aarch64-apple-darwin` | `terminal-macos-aarch64.tar.gz` |

These are native builds. The workflow runs workspace formatting, check, unit and integration tests, doc tests, and Clippy on each runner before release builds. Its packaging step checks the archive manifest and launches the extracted release executable with `--help` from a temporary directory outside the checkout. Existing native PTY tests and Windows PowerShell tests remain in the workspace test suite. The smoke test does not create a graphical window; interactive shell and GPU behavior still need human testing on each platform. All six runner combinations are configured for CI validation; they are only confirmed as built once the workflow has completed successfully on GitHub.

The Linux binaries target GNU libc and are dynamically linked. The workflow checks `ldd` for missing shared libraries on its build runners. Users need a compatible GNU libc system and installed graphics, window-system, and font libraries; compatibility with older distributions or every desktop environment is not implied by a successful runner smoke test. The macOS archives contain a command-line binary, not an app bundle, and are unsigned and not notarized. Gatekeeper may require the user to approve opening the binary. The Windows PowerShell OSC 7 integration script is compiled into the executable; it does not need a file from the source checkout.

## Publishing

Merge a validated change to `master`, then push an annotated or lightweight tag in the form `vX.Y.Z` (for example, `v0.1.0`) at the release commit. A tag push runs the same six verification and packaging jobs. Only after all succeed does the workflow create a GitHub Release with the six archives attached. The workflow uses the repository's built-in `GITHUB_TOKEN` with release contents permission; it needs no separate secret. Ordinary branch pushes, pull requests, and manual workflow runs upload inspection artifacts but do not publish a Release. A failed target blocks publication of the complete release set.

Packaging and local smoke testing can be repeated on a matching native host after a release build:

```sh
cargo build --locked --release -p terminal-app --bin terminal --target <rust-target>
python scripts/package_release.py <rust-target>
```

The script rejects a target that does not match the host, avoiding a misleading executable smoke result from cross-compilation. Installer formats, code signing, notarization, package-manager publishing, and auto-update are outside this first portable release slice.
