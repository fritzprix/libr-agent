## 2024-05-18 - Prevent Unresolved Symlink Target File Write/Read Path Traversal
**Vulnerability:** `SecurityValidator` allows creating or writing to files via unresolved symlinks (where the symlink itself is the target path, not its parent directory).
**Learning:** `canonicalize()` fails if a symlink target doesn't exist, leading the fallback logic to check parent directories. Since the parent directory (`test_dir`) exists and is canonical, validation incorrectly passes, but the file creation actually follows the dangling symlink outside the allowed base directory.
**Prevention:** Check if the exact target path is an unresolved (broken) symlink using `std::fs::symlink_metadata` before recursively traversing up and canonicalizing parent directories.
## 2024-05-20 - Path Traversal in File Export Package Name
**Vulnerability:** Path traversal in `FileExportService::create_zip_export` where `package_name` is directly interpolated into a path passed to `temp_dir.path().join()`.
**Learning:** `Path::join` allows directory traversal if the right-hand side contains relative components like `..`, potentially escaping intended directories (e.g., `temp_dir`). This can be exploited to overwrite arbitrary files when creating zip exports.
**Prevention:** Sanitize variables from external input used in path constructions, even for temporary filenames. Replace non-alphanumeric characters with safe alternatives like underscores.
## 2024-05-31 - Path Traversal in MCP Workspace Export Package Name
**Vulnerability:** Path traversal vulnerability in `export_operations.rs` where the `name` parameter from the MCP request is directly used to construct the ZIP export filename (`{package_name}_{timestamp}.zip`).
**Learning:** External user input, even when used only for a portion of a filename, can contain path traversal characters (like `../`) that alter the final resolved path when used with `Path::join`, potentially allowing files to be written outside the intended directory.
**Prevention:** Always sanitize user-provided filename inputs (e.g., using `sanitize_package_name` to remove or replace non-alphanumeric characters) before interpolating them into paths.
