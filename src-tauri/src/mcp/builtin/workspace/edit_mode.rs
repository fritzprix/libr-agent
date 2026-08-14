//! Compile-time selection between line+anchor edits and string replacement.
//!
//! Production builds enable exactly one of `workspace-edit-file` or
//! `workspace-str-replace`. When both are enabled (e.g. `cargo clippy --all-features`),
//! `workspace-str-replace` takes precedence.

#[cfg(not(any(feature = "workspace-edit-file", feature = "workspace-str-replace")))]
compile_error!("Enable either `workspace-edit-file` or `workspace-str-replace`");

pub const PRIMARY_EDIT_TOOL: &str = if cfg!(feature = "workspace-str-replace") {
    "strReplace"
} else {
    "editFile"
};

/// Line+anchor metadata (`42:a31f2c|...`) is only active for edit-file-only builds.
pub const LINE_ANCHORS_ENABLED: bool =
    cfg!(feature = "workspace-edit-file") && !cfg!(feature = "workspace-str-replace");

/// Workspace service-context bullet listing file mutation tools.
pub fn workspace_file_tools_context_list() -> String {
    format!(
        "workspace__readFile, workspace__writeFile, workspace__listDirectory, workspace__{PRIMARY_EDIT_TOOL}"
    )
}

pub fn read_file_tool_hint() -> &'static str {
    if LINE_ANCHORS_ENABLED {
        "Use showLineAnchors=true when you need anchors for workspace__editFile."
    } else {
        // Kept in the tool schema (not success next-actions) so edit workflows
        // still see a minimal affordance without padding every read result.
        "When editing afterward, copy the exact on-disk text currently in the file into workspace__strReplace. Do not reuse text from an earlier successful edit."
    }
}

#[cfg(all(
    feature = "workspace-edit-file",
    not(feature = "workspace-str-replace")
))]
pub fn read_file_show_line_anchors_schema_hint() -> &'static str {
    "Optional: include opaque edit anchors for each line in the form '42:a31f2c|...'. For workspace__editFile, pass the full '42:a31f2c' (line + anchor) — omit only the trailing '|...'."
}

#[cfg(all(
    feature = "workspace-edit-file",
    not(feature = "workspace-str-replace")
))]
pub fn search_show_line_anchors_schema_hint() -> &'static str {
    "Include edit anchors in results for use with workspace__editFile (default: false). Anchored lines look like '42:a31f2c|...'; for workspace__editFile, pass '42:a31f2c' (line:anchor prefix only)."
}

pub fn search_inline_match_footer(show_hashes: bool) -> String {
    if LINE_ANCHORS_ENABLED {
        if show_hashes {
            "Copy each line's N:anchor prefix into workspace__editFile start/end (omit '|content'); include end for range edits.\n"
                .to_string()
        } else {
            "For targeted edits, rerun with showLineAnchors=true to get anchors.\n".to_string()
        }
    } else {
        let _ = show_hashes;
        "To edit a match, read the file and copy the exact on-disk text currently in the file into workspace__strReplace.\n"
            .to_string()
    }
}

pub fn write_file_post_write_anchor_heading() -> &'static str {
    if LINE_ANCHORS_ENABLED {
        "\nCurrent anchors:\n"
    } else {
        "\nCurrent content preview:\n"
    }
}

pub fn write_file_anchor_preview_note() -> &'static str {
    if LINE_ANCHORS_ENABLED {
        "*(Note: The lines in the code block below are prefixed with `lineNumber:anchor|` for subsequent editing. These prefixes are metadata and are NOT part of the actual file content.)*\n"
    } else {
        ""
    }
}

pub fn read_file_anchor_prefix_note() -> &'static str {
    "*(Note: The `{lineNumber}:{anchor}|` prefixes in the code block above are metadata added by the tool for edit reference, and are NOT part of the actual file content.)*"
}

pub fn read_file_anchor_output_suffix() -> &'static str {
    "\n\nFor workspace__editFile, pass the full `N:anchor` from each line (e.g. `42:a31f2c`). Omit the trailing `|{content}`."
}
