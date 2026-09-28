# Application name and shared icon foundation

Status: Accepted

## Context

Installer and application-bundle work needs a stable identity and shared icon source. ADR-0008 deferred public branding while retaining internal package names.

## Decision

Use **Terminal** as the display name, `terminal` as the executable, and `io.github.xdjanisxd.terminal` as the stable desktop/icon and future bundle identifier. Retain all Cargo package names and portable artifact names.

Own one lossless 1024×1024 RGBA PNG under `assets/branding/source`. Commit deterministic platform variants beside the platform metadata. The initial project-owned artwork is explicitly a placeholder. Asset generation is a developer/release check, not a normal build prerequisite.

The app owns window identity and the Windows resource build hook; packaging owns desktop registration and bundle layout. Terminal semantics, configuration, renderer, and PTY ownership do not change.

## Consequences

Final artwork can replace the source without changing build architecture. Future Windows installers, Linux packages/AppImage, and macOS bundles reuse the assets and identity. This resolves only ADR-0008's public-branding deferral; its license, internal naming, target matrix, and unresolved minimum OS versions remain in force. Installer formats, signing, and publishing are not implemented here.
