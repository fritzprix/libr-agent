//! Bundled skill timeout helpers, path guards, and shared Twikit patches.

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

#[test]
fn x_cli_twikit_patches_module_loads() {
    let script = bundled_skill_script("x-cli", "twikit_patches.py");
    let output = python_cmd()
        .arg("-c")
        .arg(format!(
            r#"
import sys
from pathlib import Path
sys.path.insert(0, str(Path({path:?}).parent))
from twikit_patches import apply_twikit_patches
apply_twikit_patches()
apply_twikit_patches()  # idempotent
print("ok")
"#,
            path = script
        ))
        .output()
        .expect("python available");
    assert!(
        output.status.success(),
        "twikit_patches load failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn x_cli_await_op_times_out() {
    let script = bundled_skill_script("x-cli", "x_cli.py");
    let output = python_cmd()
        .arg("-c")
        .arg(format!(
            r#"
import asyncio
import sys
from pathlib import Path

source = Path({path:?}).read_text(encoding="utf-8")
# Extract await_op + OP_TIMEOUT_SEC without executing full module side effects.
start = source.index("OP_TIMEOUT_SEC = ")
chunk = source[start:]
end = chunk.index("\ndef get_client")
ns = {{"asyncio": asyncio}}
exec(chunk[:end], ns)
ns["OP_TIMEOUT_SEC"] = 0.05

async def slow():
    await asyncio.sleep(1.0)
    return "nope"

async def main():
    try:
        await ns["await_op"](slow())
        raise SystemExit("expected TimeoutError")
    except TimeoutError as e:
        assert "timed out" in str(e).lower()
        print("ok")

asyncio.run(main())
"#,
            path = script
        ))
        .output()
        .expect("python available");
    assert!(
        output.status.success(),
        "await_op timeout test failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ig_cli_run_sync_times_out() {
    let script = bundled_skill_script("ig-cli", "ig_cli.py");
    let output = python_cmd()
        .arg("-c")
        .arg(format!(
            r#"
import time
from concurrent.futures import ThreadPoolExecutor
from concurrent.futures import TimeoutError as FuturesTimeout
from pathlib import Path

source = Path({path:?}).read_text(encoding="utf-8")
start = source.index("def run_sync")
rest = source[start:]
end = rest.index("\ndef validate_media_path")
# Strip return type annotations that need typing generics
chunk = rest[:end].replace("fn: Callable[..., T], *args: Any, timeout: float = OP_TIMEOUT_SEC, **kwargs: Any) -> T", "fn, *args, timeout=OP_TIMEOUT_SEC, **kwargs)")
ns = {{
    "ThreadPoolExecutor": ThreadPoolExecutor,
    "FuturesTimeout": FuturesTimeout,
    "OP_TIMEOUT_SEC": 0.05,
}}
exec(chunk, ns)

def slow():
    time.sleep(1.0)
    return "nope"

try:
    ns["run_sync"](slow, timeout=0.05)
    raise SystemExit("expected TimeoutError")
except TimeoutError as e:
    assert "timed out" in str(e).lower()
    print("ok")
"#,
            path = script
        ))
        .output()
        .expect("python available");
    assert!(
        output.status.success(),
        "run_sync timeout test failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ig_cli_validate_media_path_rejects_config_dir() {
    let script = bundled_skill_script("ig-cli", "ig_cli.py");
    let output = python_cmd()
        .arg("-c")
        .arg(format!(
            r#"
import json
import sys
import tempfile
from pathlib import Path

source = Path({path:?}).read_text(encoding="utf-8")
start = source.index("def validate_media_path")
rest = source[start:]
end = rest.index("\ndef save_session")
ns = {{}}
# Minimal deps for validate_media_path
ns["Path"] = Path
ns["Any"] = object
ns["MAX_MEDIA_BYTES"] = 100 * 1024 * 1024

def emit_error(payload, code=1):
    raise SystemExit(json.dumps({{"code": code, **payload}}))

ns["emit_error"] = emit_error

with tempfile.TemporaryDirectory() as tmp:
    home = Path(tmp)
    config = home / ".libragent" / "ig_config.json"
    session = home / ".libragent" / "ig_session.json"
    config.parent.mkdir(parents=True)
    config.write_text("{{}}", encoding="utf-8")
    session.write_text("{{}}", encoding="utf-8")
    ns["CONFIG_PATH"] = config
    ns["SESSION_PATH"] = session
    exec(rest[:end], ns)

    # Allowed outside config dir
    allowed = home / "photo.jpg"
    allowed.write_bytes(b"abc")
    got = ns["validate_media_path"](str(allowed))
    assert Path(got) == allowed.resolve() or Path(got) == allowed

    # Denied inside ~/.libragent
    denied = config.parent / "secret.jpg"
    denied.write_bytes(b"abc")
    try:
        ns["validate_media_path"](str(denied))
        raise SystemExit("expected denial")
    except SystemExit as e:
        payload = json.loads(str(e))
        assert payload["code"] == 3
        assert "Access denied" in payload["message"]
    print("ok")
"#,
            path = script
        ))
        .output()
        .expect("python available");
    assert!(
        output.status.success(),
        "validate_media_path test failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn telegram_run_op_times_out() {
    let script = bundled_skill_script("telegram-cli", "telegram_cli.py");
    let output = python_cmd()
        .arg("-c")
        .arg(format!(
            r#"
import asyncio
from pathlib import Path
from types import SimpleNamespace

source = Path({path:?}).read_text(encoding="utf-8")
start = source.index("def run_op")
rest = source[start:]
end = rest.index("\ndef emit_result")
ns = {{"asyncio": asyncio, "OP_TIMEOUT_SEC": 0.05}}
exec(rest[:end], ns)

async def slow():
    await asyncio.sleep(1.0)
    return "nope"

loop = asyncio.new_event_loop()
client = SimpleNamespace(loop=loop)
try:
    ns["run_op"](client, slow(), timeout=0.05)
    raise SystemExit("expected TimeoutError")
except TimeoutError as e:
    assert "timed out" in str(e).lower()
    print("ok")
finally:
    loop.close()
"#,
            path = script
        ))
        .output()
        .expect("python available");
    assert!(
        output.status.success(),
        "telegram run_op timeout test failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
