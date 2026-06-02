## 2024-05-18 - Prevent Unresolved Symlink Target File Write/Read Path Traversal
**Vulnerability:** `SecurityValidator` allows creating or writing to files via unresolved symlinks (where the symlink itself is the target path, not its parent directory).
**Learning:** `canonicalize()` fails if a symlink target doesn't exist, leading the fallback logic to check parent directories. Since the parent directory (`test_dir`) exists and is canonical, validation incorrectly passes, but the file creation actually follows the dangling symlink outside the allowed base directory.
**Prevention:** Check if the exact target path is an unresolved (broken) symlink using `std::fs::symlink_metadata` before recursively traversing up and canonicalizing parent directories.
## 2024-05-20 - Path Traversal in File Export Package Name
**Vulnerability:** Path traversal in `FileExportService::create_zip_export` where `package_name` is directly interpolated into a path passed to `temp_dir.path().join()`.
**Learning:** `Path::join` allows directory traversal if the right-hand side contains relative components like `..`, potentially escaping intended directories (e.g., `temp_dir`). This can be exploited to overwrite arbitrary files when creating zip exports.
**Prevention:** Sanitize variables from external input used in path constructions, even for temporary filenames. Replace non-alphanumeric characters with safe alternatives like underscores.
## 2024-05-18 - Path Traversal in MCP Workspace Export
**Vulnerability:** The MCP tool `export_operations.rs` constructed the output `zip_filename` using the unsanitized `name_param` from user input, allowing potential path traversal (e.g. `../../../malicious_name.zip`).
**Learning:** Even if the input is parsed via JSON, if it's used directly in path constructions without sanitization, it poses a path traversal risk.
**Prevention:** Always sanitize user-provided filename inputs (e.g., using `sanitize_package_name` or replacing non-alphanumeric chars) before incorporating them into file paths.
