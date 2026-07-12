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
    format!("readFile, writeFile, listDirectory, {PRIMARY_EDIT_TOOL}")
}

pub fn read_file_tool_hint() -> &'static str {
    if LINE_ANCHORS_ENABLED {
        "Use showLineAnchors=true when you need anchors for editFile."
    } else {
        "Read files before strReplace so old_string matches the on-disk content exactly."
    }
}

#[cfg(all(
    feature = "workspace-edit-file",
    not(feature = "workspace-str-replace")
))]
pub fn read_file_show_line_anchors_schema_hint() -> &'static str {
    "Optional: include opaque edit anchors for each line in the form '42:a31f2c|...'. For editFile, pass only the 6-character anchor (for example 'a31f2c'), not '42:a31f2c' or the trailing '|...'."
}

#[cfg(all(
    feature = "workspace-edit-file",
    not(feature = "workspace-str-replace")
))]
pub fn search_show_line_anchors_schema_hint() -> &'static str {
    "Include edit anchors in results for use with editFile (default: false). Anchored lines look like '42:a31f2c|...'; for edit tools, pass only the 6-character anchor (for example 'a31f2c')."
}

pub fn read_file_primary_next_action(show_line_anchors: bool) -> String {
    if LINE_ANCHORS_ENABLED {
        if show_line_anchors {
            "Use editFile with only the 6-character startAnchor in edits[]; for ranges, also copy only the 6-character endAnchor from the final line".to_string()
        } else {
            "If you plan to use editFile next, rerun with showLineAnchors=true to get anchors"
                .to_string()
        }
    } else {
        "Copy the exact text block you want to change into strReplace.old_string".to_string()
    }
}

pub fn read_file_secondary_next_action() -> &'static str {
    if LINE_ANCHORS_ENABLED {
        "Use editFile with op='insert_after', startLine, and startAnchor in edits[] to insert below an existing line"
    } else {
        "Use strReplace when you already know the exact old_string to replace"
    }
}

pub fn search_inline_match_footer(show_hashes: bool) -> String {
    if LINE_ANCHORS_ENABLED {
        if show_hashes {
            "Use the returned anchors with editFile. For range replacement/deletion, also copy endAnchor from the exact end line.\n".to_string()
        } else {
            "If you plan to use editFile next, run again with `showLineAnchors: true` to get anchors.\n"
                .to_string()
        }
    } else {
        let _ = show_hashes;
        "Use readFile on a match, then strReplace with the exact old_string copied from that output.\n"
            .to_string()
    }
}

pub fn search_directory_next_step(show_hashes: bool) -> &'static str {
    if LINE_ANCHORS_ENABLED {
        if show_hashes {
            "Use the returned anchors with editFile; add endAnchor for range replacement/deletion"
        } else {
            "Run with `showLineAnchors: true` to get anchors for targeted editing tools"
        }
    } else {
        let _ = show_hashes;
        "Use readFile on a hit, then strReplace with old_string copied verbatim from the file"
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
    "\n\nFor edit tools, pass only the 6-character anchor (example: `792c6f`). Do not pass `1:792c6f` or `|{content}`."
}
