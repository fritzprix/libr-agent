//! Shared PowerShell exit-code rule for isolated and persistent shells (#2805).
//!
//! Success follows the process exit code only — never command text or stdout/stderr.
//! Immediately after the user command (before any `Write-Output`):
//!
//! 1. Treat the command as failed when `$?` is false **or** `$Error.Count` grew
//!    (needed because persistent shells run the user text via `Invoke-Expression`,
//!    which leaves `$?` true after non-terminating cmdlet errors).
//! 2. If failed: use a non-zero `$LASTEXITCODE` when set, otherwise `1` (a stale
//!    `0` from an earlier native in a `;` chain must not mask a later cmdlet
//!    failure).
//! 3. If ok: use `$LASTEXITCODE` when set (including `0`), otherwise `0` (a later
//!    successful native in a `;` chain wins — document that pitfall).
//!
//! Call [`ERROR_COUNT_BEFORE`] once before the user command, then
//! [`CAPTURE_AND_ASSIGN_EXIT`] immediately after.
//!
//! Snippets are single-line so persistent PowerShell (interactive stdin) does
//! not wait on a multi-line `if` block.

/// Snapshot `$Error.Count` before the user command (isolated try body / before
/// persistent `Invoke-Expression`).
pub const ERROR_COUNT_BEFORE: &str = "$__libr_err_before = $Error.Count";

/// Capture success / `$LASTEXITCODE` then assign `$__libr_exit`.
pub const CAPTURE_AND_ASSIGN_EXIT: &str = "\
$__libr_ok = ($? -eq $true) -and ($Error.Count -eq $__libr_err_before); \
$__libr_lec = $LASTEXITCODE; \
if (-not $__libr_ok) { if ($null -ne $__libr_lec -and $__libr_lec -ne 0) { $__libr_exit = $__libr_lec } else { $__libr_exit = 1 } } \
else { if ($null -ne $__libr_lec) { $__libr_exit = $__libr_lec } else { $__libr_exit = 0 } }";

/// Short contract for tool descriptions (isolated and persistent).
pub const TOOL_DESCRIPTION_EXIT_RULE: &str = "\
Exit code (isolated and persistent share this rule): after your command, success \
is the exit code only — never flipped by matching stdout/stderr or the command text. \
A cmdlet non-terminating error (`Get-Item` missing) is exit 1 even when a prior \
native left `$LASTEXITCODE` at 0. A later successful native in a `;` chain wins \
(`cmd /c exit 2; cmd /c exit 0` → 0). `2>&1` on a successful native can add error \
records and report 1 — avoid merging stderr when you care about the code. \
`| Select-Object -First` may stop an upstream native early; trust the reported exit code.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_snippet_reads_ok_and_lec_before_assigning() {
        assert!(ERROR_COUNT_BEFORE.contains("$Error.Count"));
        assert!(CAPTURE_AND_ASSIGN_EXIT.contains("$__libr_err_before"));
        assert!(CAPTURE_AND_ASSIGN_EXIT.contains("$__libr_lec = $LASTEXITCODE"));
        assert!(CAPTURE_AND_ASSIGN_EXIT.contains("$__libr_exit = 1"));
        assert!(
            !CAPTURE_AND_ASSIGN_EXIT.contains('\n'),
            "persistent stdin needs a one-line assign"
        );
    }
}
