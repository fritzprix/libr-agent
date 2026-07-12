// File operations module - split for maintainability
// Original file_operations.rs was 2022 lines - now split into focused modules

#[cfg(feature = "workspace-edit-file")]
pub mod edit_line;
#[cfg(feature = "workspace-str-replace")]
pub mod str_replace;
pub mod import;
pub mod list_dir;
pub mod read;
pub mod search;
pub mod utils;
pub mod write;
