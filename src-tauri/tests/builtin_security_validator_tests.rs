use tauri_mcp_agent_lib::mcp::builtin::utils::{SecurityError, SecurityValidator};
use tempfile::tempdir;

#[test]
fn test_path_validation() {
    let temp_dir = tempdir().unwrap();
    let validator = SecurityValidator::new_with_base_dir(temp_dir.path().to_path_buf());

    // Valid paths
    assert!(validator.validate_path("test.txt").is_ok());
    assert!(validator.validate_path("./test.txt").is_ok());
    assert!(validator.validate_path("subdir/test.txt").is_ok());
    assert!(validator
        .validate_path("attachments/docker_조사....md")
        .is_ok());

    // Absolute paths for read operations should be allowed
    assert!(validator
        .validate_path_for_read("/tmp/some-file.txt")
        .is_ok());

    // Invalid paths (directory traversal)
    assert!(validator.validate_path("../test.txt").is_err());
    assert!(validator.validate_path("../../etc/passwd").is_err());

    // Invalid paths (absolute paths)
    assert!(validator.validate_path("/etc/passwd").is_err());
    assert!(validator.validate_path("/Users/test/file.txt").is_err());
    assert!(validator.validate_path("/tmp/outside.txt").is_err());

    // Invalid paths (Windows drive letters)
    assert!(validator.validate_path("C:\\Windows\\System32").is_err());
    assert!(validator.validate_path("D:\\secret.txt").is_err());

    // Invalid paths (complex traversal attempts)
    assert!(validator
        .validate_path("./subdir/../../../etc/passwd")
        .is_err());

    // Windows-style separators are normalized, but parent traversal must still be blocked.
    assert!(validator.validate_path("subdir\\..\\..\\Windows").is_err());
}

#[test]
fn test_normalize_path_separators() {
    let windows_path = "C:\\Users\\user\\file.txt";
    let normalized = SecurityValidator::normalize_path_separators(windows_path);
    assert_eq!(normalized, "C:/Users/user/file.txt");

    let mixed_path = "C:/Users\\user/file.txt";
    let normalized = SecurityValidator::normalize_path_separators(mixed_path);
    assert_eq!(normalized, "C:/Users/user/file.txt");

    let unix_path = "/home/user/file.txt";
    let normalized = SecurityValidator::normalize_path_separators(unix_path);
    assert_eq!(normalized, "/home/user/file.txt");
}

#[test]
fn test_extract_filename() {
    // Windows paths
    let path = "C:\\Users\\user\\Downloads\\test.pdf";
    let filename = SecurityValidator::extract_filename(path);
    assert_eq!(filename, Some("test.pdf".to_string()));

    // Unix paths
    let path = "/home/user/downloads/test.pdf";
    let filename = SecurityValidator::extract_filename(path);
    assert_eq!(filename, Some("test.pdf".to_string()));

    // Mixed separators
    let path = "C:/Users/user\\Downloads\\test.pdf";
    let filename = SecurityValidator::extract_filename(path);
    assert_eq!(filename, Some("test.pdf".to_string()));

    // Edge cases
    let path = "test.pdf";
    let filename = SecurityValidator::extract_filename(path);
    assert_eq!(filename, Some("test.pdf".to_string()));

    let path = "";
    let filename = SecurityValidator::extract_filename(path);
    assert_eq!(filename, Some("".to_string()));

    let path = "C:\\Users\\";
    let filename = SecurityValidator::extract_filename(path);
    assert_eq!(filename, Some("".to_string()));
}

#[test]
#[cfg(unix)]
fn test_symlink_traversal() {
    use std::os::unix::fs::symlink;
    let temp_dir = tempdir().unwrap();

    let validator = SecurityValidator::new_with_base_dir(temp_dir.path().to_path_buf());

    // Create a secret file outside
    let outside_dir = tempdir().unwrap();
    let secret_file = outside_dir.path().join("mcp_secret.txt");
    std::fs::write(&secret_file, "secret").unwrap();

    // Create a symlink inside base_dir pointing to secret file
    let link_path = temp_dir.path().join("bad_link");
    symlink(&secret_file, &link_path).unwrap();

    // Try to access via symlink
    let result = validator.validate_path("bad_link");

    assert!(result.is_err(), "Symlink traversal should be blocked");
    if let Err(SecurityError::PathTraversal(msg)) = result {
        assert!(msg.contains("resolves outside allowed directory"));
    } else {
        panic!("Expected PathTraversal error");
    }
}

#[test]
#[cfg(unix)]
fn test_symlink_base_dir_traversal() {
    use std::os::unix::fs::symlink;

    let temp_root = tempdir().unwrap();

    // real base directory and a symlink used as the validator's base_dir
    let real_base = temp_root.path().join("real_base");
    std::fs::create_dir_all(&real_base).unwrap();
    let base_dir_link = temp_root.path().join("base_link");
    symlink(&real_base, &base_dir_link).unwrap();

    // Use the symlinked path as base_dir; new_with_base_dir should canonicalize it
    let validator = SecurityValidator::new_with_base_dir(base_dir_link.clone());

    // secret file outside the real base
    let outside_dir = tempdir().unwrap();
    let secret_file = outside_dir.path().join("mcp_secret_base_dir.txt");
    std::fs::write(&secret_file, "secret").unwrap();

    // symlink inside the real base that points to the secret file outside
    let escaping_link = real_base.join("escaping_link");
    symlink(&secret_file, &escaping_link).unwrap();

    let result = validator.validate_path("escaping_link");

    assert!(
        result.is_err(),
        "Symlink traversal via symlinked base_dir should be blocked"
    );
    if let Err(SecurityError::PathTraversal(msg)) = result {
        assert!(msg.contains("resolves outside allowed directory"));
    } else {
        panic!("Expected PathTraversal error");
    }
}

#[test]
#[cfg(unix)]
fn test_symlink_traversal_nonexistent_file() {
    use std::os::unix::fs::symlink;

    let temp_dir = tempdir().unwrap();
    let validator = SecurityValidator::new_with_base_dir(temp_dir.path().to_path_buf());

    // Create an outside directory
    let outside_dir = tempdir().unwrap();

    // Create a symlink inside base_dir pointing to outside directory
    let link_path = temp_dir.path().join("bad_link_dir");
    symlink(outside_dir.path(), &link_path).unwrap();

    // Try to access a nonexistent file via the symlink
    let result = validator.validate_path("bad_link_dir/nonexistent_file.txt");

    assert!(
        result.is_err(),
        "Symlink traversal to nonexistent file should be blocked"
    );
    if let Err(SecurityError::PathTraversal(msg)) = result {
        assert!(msg.contains("resolves outside allowed directory"));
    } else {
        panic!("Expected PathTraversal error");
    }
}

#[test]
#[cfg(unix)]
fn test_dangling_symlink_parent_is_rejected() {
    use std::os::unix::fs::symlink;

    let temp_dir = tempdir().unwrap();
    let validator = SecurityValidator::new_with_base_dir(temp_dir.path().to_path_buf());

    let outside_dir = tempdir().unwrap();
    let missing_target_dir = outside_dir.path().join("missing-target");

    let link_path = temp_dir.path().join("dangling_link_dir");
    symlink(&missing_target_dir, &link_path).unwrap();

    let result = validator.validate_path("dangling_link_dir/new_file.txt");

    assert!(
        result.is_err(),
        "Dangling symlink parents should be rejected during path validation"
    );
    if let Err(SecurityError::PathTraversal(msg)) = result {
        assert!(msg.contains("unresolved symlink parent"));
    } else {
        panic!("Expected PathTraversal error");
    }
}
