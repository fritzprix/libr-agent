# Windows PowerShell Command Execution Fix

## Problem History

### Bug 1: `split_whitespace` Silent Failure

**Affected code:** `create_basic_isolated_command` in `session_isolation/platforms/windows.rs`

Commands starting with `powershell` or `pwsh` were detected and handled with `use_shell_wrapper = false`. The command string was then split by `split_whitespace()` and passed as individual args.

For a command like:

```
powershell -Command "Expand-Archive -Path \"foo.zip\" -DestinationPath \".\""
```

`split_whitespace()` produced `["-Command", "\"Expand-Archive", "-Path", ...]`.  
The leading `"` on `"Expand-Archive` made PowerShell interpret it as a **string literal** — it evaluated and printed the string, exited 0, and did nothing.

**Symptom:** Commands appeared to succeed (exit code 0), stdout/stderr were empty, and no side effects occurred. An agent trying to extract a zip file would get a successful `spawnProcess` response, then `listDirectory` would show nothing new. This produced thousands of tokens of confused re-tries and diagnostic work.

### Bug 2: Base64+Invoke-Expression Triggers Antivirus

**First attempted fix:** Encode the command as Base64 and use `Invoke-Expression` to evaluate it:

```powershell
$cmd = [Convert]::FromBase64String('...'); Invoke-Expression $cmd
```

This is a textbook malware obfuscation pattern. Security software (e.g. Windows Defender, enterprise EDR solutions) blocked the PowerShell process before it could execute. The process exited with `Failed` status and zero output — identical symptoms to a genuine error, but with no diagnostics available.

## Final Solution: Temp `.ps1` File with Self-Cleanup

Commands are written to a temporary `.ps1` file in `<workspace>/tmp/` and executed via a short wrapper:

```powershell
# <workspace>/tmp/cmd_<session_id>_<counter>.ps1  (scanned by AV in plaintext)
$ErrorActionPreference = 'Stop'
[System.Threading.Thread]::CurrentThread.CurrentUICulture = 'en-US'
try {
    Expand-Archive -Path "attachments/foo.zip" -DestinationPath "."
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    [Console]::Error.WriteLine($_.ScriptStackTrace)
    exit 1
}
```

The outer PowerShell process invoked by the agent runs a one-liner wrapper:

```powershell
try { & 'tmp/cmd_abc_0.ps1' } finally { Remove-Item -LiteralPath 'tmp/cmd_abc_0.ps1' -Force -ErrorAction SilentlyContinue }
```

### Why This Works

| Property            | split_whitespace | Base64+IEX | .ps1 file          |
| ------------------- | ---------------- | ---------- | ------------------ |
| Handles quoted args | ❌ breaks on `"` | ✅         | ✅                 |
| AV-friendly         | ✅               | ❌ flagged | ✅ plaintext       |
| No fragmentation    | ❌               | ✅         | ✅                 |
| Temp file cleanup   | n/a              | n/a        | ✅ `finally` block |
| Async-safe I/O      | n/a              | n/a        | ✅ `tokio::fs`     |
| Unique filenames    | n/a              | n/a        | ✅ `AtomicU64`     |

### Implementation Details

- **File naming:** `cmd_<session_id>_<counter>.ps1` where `counter` is a process-lifetime `AtomicU64`. Avoids collisions when multiple commands are spawned concurrently within the same session.
- **Async I/O:** `tokio::fs::create_dir_all` and `tokio::fs::write` — no blocking of the Tokio runtime.
- **Cleanup:** The outer `-Command` wrapper uses `try/finally` so the `.ps1` file is always deleted, even if the script errors or the process is killed mid-run.
- **Single-quote escaping:** Paths containing `'` are escaped as `''` in the wrapper string to prevent PowerShell injection.
- **`-ExecutionPolicy Bypass`:** Required on machines where execution policy would otherwise block unsigned `.ps1` files.

## Regression Tests

`src-tauri/src/session_isolation/platforms/windows.rs` (`mod tests`):

| Test                                                   | Covers                                                        |
| ------------------------------------------------------ | ------------------------------------------------------------- |
| `test_script_content_has_error_handling`               | `$ErrorActionPreference='Stop'`, `try/catch`, `exit 1`        |
| `test_script_content_no_obfuscation`                   | No `Invoke-Expression`, `FromBase64String`, `-EncodedCommand` |
| `test_script_content_preserves_double_quotes`          | Quoted args survive verbatim                                  |
| `test_script_content_preserves_expand_archive_pattern` | Exact command from Bug 1 regression                           |
| `test_script_content_powershell_command_pattern`       | `powershell -Command "..."` pattern                           |
| `test_cleanup_wrapper_calls_script_and_deletes`        | `& 'path'` + `finally { Remove-Item }`                        |
| `test_cleanup_wrapper_escapes_single_quotes_in_path`   | `'` → `''` in wrapper path                                    |
| `test_script_counter_is_monotonic`                     | `SCRIPT_COUNTER` strictly increases                           |
