//! PowerShell UTF-8 encoding snippets shared by Windows shell spawn paths.
//!
//! .NET Framework's `[System.Text.Encoding]::UTF8` includes a BOM preamble
//! (`U+FEFF`, bytes `EF BB BF`). PowerShell 5.1 uses `$OutputEncoding` when
//! piping objects to native processes, so assigning that encoding prepends one
//! or more BOM characters to child stdin.
//!
//! Python `str.strip()` does **not** remove `U+FEFF`, so secrets such as
//! Telegram 2FA passwords are hashed with a leading BOM and rejected as
//! `PasswordHashInvalidError`.
//!
//! Always assign a BOM-less `UTF8Encoding` to:
//! - `[Console]::InputEncoding`
//! - `[Console]::OutputEncoding`
//! - `$OutputEncoding` (native-command pipe encoding)

/// PowerShell 5.1-compatible constructor for UTF-8 without BOM.
pub const UTF8_NO_BOM_CTOR: &str = "New-Object System.Text.UTF8Encoding $false";

/// One-liner that sets console and pipeline encodings to UTF-8 without BOM.
///
/// Creates `$__libr_utf8` then assigns it to InputEncoding, OutputEncoding,
/// and `$OutputEncoding`.
pub const SET_UTF8_NO_BOM: &str = concat!(
    "$__libr_utf8 = New-Object System.Text.UTF8Encoding $false; ",
    "[Console]::InputEncoding = [Console]::OutputEncoding = $OutputEncoding = $__libr_utf8"
);

/// Same as [`SET_UTF8_NO_BOM`], with `[void]` so persistent-shell spawn setup
/// does not leak assignment output into the first command's stdout.
pub const SET_UTF8_NO_BOM_VOIDED: &str = concat!(
    "$__libr_utf8 = New-Object System.Text.UTF8Encoding $false; ",
    "[void]([Console]::InputEncoding = [Console]::OutputEncoding = $OutputEncoding = $__libr_utf8)"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_no_bom_ctor_disables_preamble() {
        assert!(UTF8_NO_BOM_CTOR.contains("UTF8Encoding"));
        assert!(UTF8_NO_BOM_CTOR.contains("$false"));
        assert!(
            !UTF8_NO_BOM_CTOR.contains("[System.Text.Encoding]::UTF8"),
            "Encoding.UTF8 includes a BOM preamble on .NET Framework"
        );
    }

    #[test]
    fn set_utf8_no_bom_covers_console_and_pipeline() {
        for snippet in [SET_UTF8_NO_BOM, SET_UTF8_NO_BOM_VOIDED] {
            assert!(snippet.contains(UTF8_NO_BOM_CTOR));
            assert!(snippet.contains("[Console]::InputEncoding"));
            assert!(snippet.contains("[Console]::OutputEncoding"));
            assert!(
                snippet.contains("$OutputEncoding"),
                "$OutputEncoding controls native-command stdin encoding"
            );
            assert!(
                !snippet.contains("[System.Text.Encoding]::UTF8"),
                "must not assign BOM-ful Encoding.UTF8 to console/pipeline"
            );
        }
    }
}
