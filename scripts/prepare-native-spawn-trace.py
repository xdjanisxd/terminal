"""Generate an opt-in, diagnostic-only portable-pty 0.9.0 overlay under target.

Build with cargo --config target/native-spawn-trace/patch.toml build --release
-p terminal-app. Never ship that build. The normal dependency is not modified.
"""

import argparse
import os
from pathlib import Path
import shutil


TRACE = r'''
// Diagnostic overlay only: buffer monotonic stamps, flush after spawn returns.
thread_local! {
    static TRACE: std::cell::RefCell<Option<(std::time::Instant, Vec<(&'static str, u128)>)>> =
        std::cell::RefCell::new(None);
}
pub(crate) fn trace_start() {
    if std::env::var("TERMINAL_STARTUP_DIAGNOSTICS").as_deref() == Ok("1") {
        TRACE.with(|t| *t.borrow_mut() = Some((std::time::Instant::now(), Vec::with_capacity(32))));
    }
}
pub(crate) fn trace(stage: &'static str) {
    TRACE.with(|t| {
        if let Some((start, records)) = t.borrow_mut().as_mut() {
            records.push((stage, start.elapsed().as_micros()));
        }
    });
}
pub(crate) fn trace_flush() {
    use std::io::Write;
    TRACE.with(|t| {
        if let Some((_, records)) = t.borrow_mut().take() {
            let text: String = records.into_iter().map(|(stage, us)|
                format!("native-startup elapsed_us={} stage={}\n", us, stage)).collect();
            if let Some(mut path) = std::env::var_os("TERMINAL_STARTUP_LOG") {
                path.push(".native");
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = file.write_all(text.as_bytes());
                }
            } else {
                eprint!("{}", text);
            }
        }
    });
}
'''


def replace_once(text, old, new):
    if text.count(old) != 1:
        raise ValueError(f"Expected exactly one backend anchor: {old!r}")
    return text.replace(old, new, 1)


def prepare(source, output):
    # Copy into a fresh directory; never modify registry sources or overwrite data.
    shutil.copytree(source, output / "portable-pty")
    backend = output / "portable-pty/src"

    def edit(name, substitutions):
        path = backend / name
        text = path.read_text(encoding="utf-8")
        for old, new in substitutions:
            text = replace_once(text, old, new)
        path.write_text(text, encoding="utf-8")

    edit("win/conpty.rs", [
        ("        let stdin = Pipe::new()?;", '        super::trace_start();\n        super::trace("input-pipe-begin");\n        let stdin = Pipe::new()?;\n        super::trace("input-pipe-end");'),
        ("        let stdout = Pipe::new()?;", '        super::trace("output-pipe-begin");\n        let stdout = Pipe::new()?;\n        super::trace("output-pipe-end");\n        super::trace("pseudoconsole-begin");'),
        ("        let master = ConPtyMasterPty {", '        super::trace("pseudoconsole-end");\n        let master = ConPtyMasterPty {'),
        ("        let inner = self.inner.lock().unwrap();\n        let child = inner.con.spawn_command(cmd)?;", '        super::trace("spawn-lock-begin");\n        let inner = self.inner.lock().unwrap();\n        super::trace("spawn-lock-acquired");\n        let result = inner.con.spawn_command(cmd);\n        super::trace("backend-spawn-end");\n        super::trace_flush();\n        let child = result?;'),
    ])
    edit("win/psuedocon.rs", [
        ("        let mut attrs =", '        super::trace("attributes-begin");\n        let mut attrs ='),
        ("        let mut pi:", '        super::trace("attributes-end");\n        let mut pi:'),
        ("        let cwd = cmd.current_directory();", '        super::trace("cwd-begin");\n        let cwd = cmd.current_directory();\n        super::trace("cwd-end");\n        super::trace("environment-utf16-begin");\n        let mut environment = cmd.environment_block();\n        super::trace("environment-utf16-end");\n        super::trace("create-process-begin");'),
        ("cmd.environment_block().as_mut_slice().as_mut_ptr()", "environment.as_mut_slice().as_mut_ptr()"),
        ("        if res == 0 {", '        super::trace("create-process-end");\n        if res == 0 {'),
        ("        Ok(WinChild {", '        super::trace("handles-owned");\n        Ok(WinChild {'),
    ])
    edit("cmdbuilder.rs", [
        ("        let exe: OsString = if self.is_default_prog() {", '        crate::win::trace("executable-resolution-begin");\n        let exe: OsString = if self.is_default_prog() {'),
        ("        Self::append_quoted(&exe, &mut cmdline);", '        crate::win::trace("executable-resolution-end");\n        crate::win::trace("quoting-utf16-begin");\n        Self::append_quoted(&exe, &mut cmdline);'),
        ("        Ok((exe, cmdline))", '        crate::win::trace("quoting-utf16-end");\n        Ok((exe, cmdline))'),
    ])
    with (backend / "win/mod.rs").open("a", encoding="utf-8") as file:
        file.write(TRACE)
    path = (output / "portable-pty").resolve().as_posix()
    (output / "patch.toml").write_text(
        f'[patch.crates-io]\nportable-pty = {{ path = "{path}" }}\n', encoding="utf-8"
    )


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=root / "target/native-spawn-trace")
    arguments = parser.parse_args()
    registry = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")) / "registry/src"
    matches = list(registry.glob("*/portable-pty-0.9.0"))
    if len(matches) != 1:
        raise SystemExit("Expected one cached portable-pty 0.9.0 source")
    prepare(matches[0], arguments.output)
    print(f"Diagnostic overlay: {arguments.output / 'patch.toml'}")
