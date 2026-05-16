## 2024-05-16 - Path Traversal via Absolute Paths
**Vulnerability:** The `SecurityValidator` allowed arbitrary path access by bypassing relative traversal checks if an absolute path was directly provided.
**Learning:** `clean_path.is_absolute()` bypasses `base_dir.join()` making absolute inputs directly become the target path without checking containment.
**Prevention:** Always verify that the final canonicalized or resolved absolute path explicitly `.starts_with()` the intended base directory.
