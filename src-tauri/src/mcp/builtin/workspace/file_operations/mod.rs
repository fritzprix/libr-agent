// File operations module - split for maintainability
// Original file_operations.rs was 2022 lines - now split into focused modules

#[cfg(feature = "workspace-edit-file")]
pub mod edit_line;
pub mod import;
pub mod list_dir;
pub mod read;
pub mod search;
#[cfg(feature = "workspace-str-replace")]
pub mod str_replace;
pub mod utils;
pub mod write;
