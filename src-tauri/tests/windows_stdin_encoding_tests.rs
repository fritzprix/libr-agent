//! Windows stdin encoding: PowerShell BOM must not leak into native pipes.

use std::path::PathBuf;
use std::process::Command;

use tauri_mcp_agent_lib::utils::env::get_isolated_env;
use tauri_mcp_agent_lib::utils::powershell_encoding::{
    SET_UTF8_NO_BOM, SET_UTF8_NO_BOM_VOIDED, UTF8_NO_BOM_CTOR,
};

fn bundled_skill_script(skill: &str, script: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("bundled_skills")
        .join(skill)
        .join("scripts")
        .join(script)
}

fn python_cmd() -> Command {
    for bin in ["python3", "python"] {
        let probe = Command::new(bin)
            .arg("-c")
            .arg("import sys")
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if probe {
            return Command::new(bin);
        }
    }
    Command::new("python3")
}

fn load_sanitize_secret(script_path: &PathBuf) -> String {
    format!(
        r#"
from pathlib import Path
source = Path({path:?}).read_text(encoding="utf-8")
start = source.index("def sanitize_secret")
rest = source[start:]
next_def = rest.find("\ndef ", 1)
chunk = rest if next_def == -1 else rest[:next_def]
ns = {{}}
exec(chunk, ns)
sanitize = ns["sanitize_secret"]
assert sanitize("\ufeff\ufeffpassword\n") == "password"
assert sanitize("  secret  ") == "secret"
assert sanitize("\ufeff  \ufeffcode") == "code"
assert sanitize("") == ""
print("ok")
"#,
        path = script_path
    )
}

#[test]
fn powershell_encoding_snippets_use_utf8_without_bom() {
    for snippet in [SET_UTF8_NO_BOM, SET_UTF8_NO_BOM_VOIDED] {
        assert!(snippet.contains(UTF8_NO_BOM_CTOR));
        assert!(snippet.contains("$OutputEncoding"));
        assert!(
            !snippet.contains("[System.Text.Encoding]::UTF8"),
            "Encoding.UTF8 includes a BOM preamble on .NET Framework"
        );
    }
}

#[test]
fn isolated_env_forces_python_utf8_on_windows() {
    let isolated = get_isolated_env();
    let python_utf8 = isolated
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("PYTHONUTF8"))
        .map(|(_, value)| value.as_str());

    #[cfg(windows)]
    assert_eq!(python_utf8, Some("1"));

    #[cfg(not(windows))]
    assert!(python_utf8.is_none() || python_utf8 == Some("1"));
}

#[test]
fn telegram_setup_strips_leading_utf8_bom() {
    let script = bundled_skill_script("telegram-cli", "setup.py");
    let output = python_cmd()
        .arg("-c")
        .arg(load_sanitize_secret(&script))
        .output()
        .expect("python available");
    assert!(
        output.status.success(),
        "sanitize_secret self-test failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn x_cli_setup_strips_leading_utf8_bom() {
    let script = bundled_skill_script("x-cli", "setup.py");
    let output = python_cmd()
        .arg("-c")
        .arg(load_sanitize_secret(&script))
        .output()
        .expect("python available");
    assert!(
        output.status.success(),
        "sanitize_secret self-test failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn email_setup_strips_leading_utf8_bom() {
    let script = bundled_skill_script("email-integration", "setup_account.py");
    let output = python_cmd()
        .arg("-c")
        .arg(load_sanitize_secret(&script))
        .output()
        .expect("python available");
    assert!(
        output.status.success(),
        "sanitize_secret self-test failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
