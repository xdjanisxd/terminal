#![cfg(windows)]

use std::fs;
use std::path::Path;
use std::process::Command;

const INTEGRATION: &str = include_str!("../src/powershell_osc7.ps1");

fn run_script(directory: &Path, body: &str, custom_prompt: bool) -> String {
    let custom_prompt = if custom_prompt {
        "function prompt { 'CUSTOM> ' }"
    } else {
        ""
    };
    let command = format!("{custom_prompt}\n& {{ {INTEGRATION} }}\n{body}");
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &command])
        .current_dir(directory.parent().unwrap_or(directory))
        .env("TERMINAL_TEST_TARGET", directory)
        .output()
        .expect("Windows PowerShell should be available for the native smoke test");
    assert!(
        output.status.success(),
        "PowerShell failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("PowerShell output should be UTF-8")
}

#[test]
fn prompt_reports_drive_paths_spaces_and_unicode_without_changing_custom_prompt() {
    let root =
        std::env::temp_dir().join(format!("terminal-powershell-osc7-{}", std::process::id()));
    for (name, encoded) in [
        ("normal", "normal"),
        ("with spaces", "with%20spaces"),
        ("目录", "%E7%9B%AE%E5%BD%95"),
    ] {
        let directory = root.join(name);
        fs::create_dir_all(&directory).unwrap();
        let output = run_script(
            &directory,
            "Set-Location -LiteralPath $env:TERMINAL_TEST_TARGET; prompt",
            true,
        );
        assert!(output.contains("CUSTOM> "), "{output:?}");
        assert!(output.contains("\x1b]7;file:///"), "{output:?}");
        assert!(output.contains(encoded), "{output:?}");
        assert_eq!(output.matches("\x1b]7;").count(), 1);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn default_prompt_still_renders() {
    let output = run_script(&std::env::temp_dir(), "prompt", false);
    assert!(output.contains("\x1b]7;file:///"), "{output:?}");
    assert!(output.contains('>'), "{output:?}");
}

#[test]
fn sourcing_twice_does_not_wrap_recursively_and_unavailable_locations_are_safe() {
    let output = run_script(
        &std::env::temp_dir(),
        &format!(
            "& {{ {INTEGRATION} }}\nprompt\nSet-Location Env:\nprompt\nfunction Get-Location {{ throw 'unavailable' }}\nprompt"
        ),
        true,
    );
    assert_eq!(output.matches("CUSTOM> ").count(), 3, "{output:?}");
    assert_eq!(output.matches("\x1b]7;").count(), 1, "{output:?}");
}
