## 2024-05-18 - Prevent Unresolved Symlink Target File Write/Read Path Traversal
**Vulnerability:** `SecurityValidator` allows creating or writing to files via unresolved symlinks (where the symlink itself is the target path, not its parent directory).
**Learning:** `canonicalize()` fails if a symlink target doesn't exist, leading the fallback logic to check parent directories. Since the parent directory (`test_dir`) exists and is canonical, validation incorrectly passes, but the file creation actually follows the dangling symlink outside the allowed base directory.
**Prevention:** Check if the exact target path is an unresolved (broken) symlink using `std::fs::symlink_metadata` before recursively traversing up and canonicalizing parent directories.
## 2024-05-20 - Path Traversal in File Export Package Name
**Vulnerability:** Path traversal in `FileExportService::create_zip_export` where `package_name` is directly interpolated into a path passed to `temp_dir.path().join()`.
**Learning:** `Path::join` allows directory traversal if the right-hand side contains relative components like `..`, potentially escaping intended directories (e.g., `temp_dir`). This can be exploited to overwrite arbitrary files when creating zip exports.
**Prevention:** Sanitize variables from external input used in path constructions, even for temporary filenames. Replace non-alphanumeric characters with safe alternatives like underscores.
## 2026-05-24 - Path Traversal in ZIP Export
**Vulnerability:** The `handle_export` function accepted a user-provided `name` parameter and directly interpolated it into a file path (`exports_dir.join("packages").join(&zip_filename)`) without sanitization, allowing path traversal (e.g., `../../../`) to write ZIP files outside the intended `.libragent/exports` directory.
**Learning:** Path traversal vulnerabilities can occur when constructing destination paths using user-provided strings, even when the extension or suffix is hardcoded (like `.zip`).
**Prevention:** Always sanitize user-provided strings (e.g., `sanitize_package_name` to replace non-alphanumeric characters with underscores) before using them as filenames or path components.
