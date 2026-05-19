## 2024-05-18 - Prevent Unresolved Symlink Target File Write/Read Path Traversal
**Vulnerability:** `SecurityValidator` allows creating or writing to files via unresolved symlinks (where the symlink itself is the target path, not its parent directory).
**Learning:** `canonicalize()` fails if a symlink target doesn't exist, leading the fallback logic to check parent directories. Since the parent directory (`test_dir`) exists and is canonical, validation incorrectly passes, but the file creation actually follows the dangling symlink outside the allowed base directory.
**Prevention:** Check if the exact target path is an unresolved (broken) symlink using `std::fs::symlink_metadata` before recursively traversing up and canonicalizing parent directories.
