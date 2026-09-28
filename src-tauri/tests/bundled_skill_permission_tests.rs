//! Bundled skill credential file permission hardening.

use std::path::PathBuf;
use std::process::Command;

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

fn load_harden_private_file(script_path: &PathBuf) -> String {
    format!(
        r#"
from pathlib import Path
import tempfile
import os
import stat

source = Path({path:?}).read_text(encoding="utf-8")
start = source.index("def harden_private_file")
rest = source[start:]
next_def = rest.find("\ndef ", 1)
chunk = rest if next_def == -1 else rest[:next_def]
ns = {{"os": os, "OSError": OSError, "Path": Path}}
exec(chunk, ns)
harden = ns["harden_private_file"]

with tempfile.TemporaryDirectory() as tmp:
    target = Path(tmp) / "secret.json"
    target.write_text('{{"ok": true}}', encoding="utf-8")
    os.chmod(target, 0o644)
    harden(target)
    mode = stat.S_IMODE(target.stat().st_mode)
    if os.name != "nt":
        assert mode == 0o600, f"expected 0o600, got {{oct(mode)}}"

# Missing path must not raise
harden(Path(tempfile.gettempdir()) / "libragent-missing-harden-target.json")
print("ok")
"#,
        path = script_path
    )
}

fn assert_harden_helper(skill: &str, script: &str) {
    let script_path = bundled_skill_script(skill, script);
    assert!(
        script_path.exists(),
        "missing bundled skill script: {}",
        script_path.display()
    );
    let output = python_cmd()
        .arg("-c")
        .arg(load_harden_private_file(&script_path))
        .output()
        .expect("python available");
    assert!(
        output.status.success(),
        "harden_private_file self-test failed for {skill}/{script}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn x_cli_setup_hardens_private_files() {
    assert_harden_helper("x-cli", "setup.py");
}

#[test]
fn x_cli_check_config_hardens_private_files() {
    assert_harden_helper("x-cli", "check_config.py");
}

#[test]
fn telegram_cli_setup_hardens_private_files() {
    assert_harden_helper("telegram-cli", "setup.py");
}

#[test]
fn ig_cli_setup_hardens_private_files() {
    assert_harden_helper("ig-cli", "setup.py");
}

#[test]
fn ig_cli_check_config_hardens_private_files() {
    assert_harden_helper("ig-cli", "check_config.py");
}

#[test]
fn ig_cli_dispatcher_hardens_private_files() {
    assert_harden_helper("ig-cli", "ig_cli.py");
}

#[test]
fn ig_cli_check_config_missing_and_ok_paths() {
    let script = bundled_skill_script("ig-cli", "check_config.py");
    let output = python_cmd()
        .arg("-c")
        .arg(format!(
            r#"
import json
import os
import stat
import tempfile
from pathlib import Path
from unittest import mock

script = Path({path:?})
source = script.read_text(encoding="utf-8")
ns = {{"__name__": "ig_check_config_test"}}
exec(compile(source, str(script), "exec"), ns)
main = ns["main"]

with tempfile.TemporaryDirectory() as tmp:
    home = Path(tmp)
    config = home / ".libragent" / "ig_config.json"
    session = home / ".libragent" / "ig_session.json"
    config.parent.mkdir(parents=True)

    with mock.patch.object(ns["Path"], "home", return_value=home):
        # Refresh module-level paths that were bound at import/exec time.
        ns["CONFIG_PATH"] = config
        ns["SESSION_PATH"] = session

        code = main()
        assert code == 1

        config.write_text(json.dumps({{"username": "demo"}}), encoding="utf-8")
        session.write_text(json.dumps({{"authorization_data": {{"ds_user_id": "1"}}}}), encoding="utf-8")
        os.chmod(config, 0o644)
        os.chmod(session, 0o644)

        code = main()
        assert code == 0
        if os.name != "nt":
            assert stat.S_IMODE(config.stat().st_mode) == 0o600
            assert stat.S_IMODE(session.stat().st_mode) == 0o600

print("ok")
"#,
            path = script
        ))
        .output()
        .expect("python available");

    assert!(
        output.status.success(),
        "ig-cli check_config path test failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
