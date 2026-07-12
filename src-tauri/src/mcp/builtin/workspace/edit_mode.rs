//! Compile-time selection between line+anchor edits and string replacement.

#[cfg(all(feature = "workspace-edit-file", feature = "workspace-str-replace"))]
compile_error!("Enable only one of `workspace-edit-file` or `workspace-str-replace`");

#[cfg(not(any(feature = "workspace-edit-file", feature = "workspace-str-replace")))]
compile_error!("Enable either `workspace-edit-file` or `workspace-str-replace`");

#[cfg(feature = "workspace-edit-file")]
pub const PRIMARY_EDIT_TOOL: &str = "editFile";

#[cfg(feature = "workspace-str-replace")]
pub const PRIMARY_EDIT_TOOL: &str = "strReplace";
