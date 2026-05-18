use tauri_mcp_agent_lib::mcp::builtin::utils::{SecurityError, SecurityValidator};
use tempfile::tempdir;

#[test]
fn test_path_validation_allows_general_absolute_paths_but_blocks_sensitive_ones() {
    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_with_base_dir(temp_dir.path().to_path_buf());
    let outside_dir = tempdir().expect("outside dir");
    let outside_file = outside_dir.path().join("safe.txt");

    assert!(validator.validate_path("test.txt").is_ok());
    assert!(validator.validate_path("./test.txt").is_ok());
    assert!(validator.validate_path("subdir/test.txt").is_ok());
    assert!(validator
        .validate_path("attachments/docker_조사....md")
        .is_ok());
    assert_eq!(
        validator
            .validate_path_for_read(&outside_file.to_string_lossy())
            .expect("general absolute path should be allowed"),
        outside_file
    );

    assert!(matches!(
        validator.validate_path("../test.txt"),
        Err(SecurityError::PathTraversal(_))
    ));
    assert!(matches!(
        validator.validate_path("./subdir/../../../etc/passwd"),
        Err(SecurityError::PathTraversal(_))
    ));
    assert!(matches!(
        validator.validate_path("subdir\\..\\..\\Windows"),
        Err(SecurityError::PathTraversal(_))
    ));

    let project_env_path = outside_dir.path().join(".env.production");
    assert_eq!(
        validator
            .validate_path(&project_env_path.to_string_lossy())
            .expect("project-local .env outside home should remain readable"),
        project_env_path
    );

    if let Some(home) = dirs::home_dir() {
        let home_env_path = home.join(".env.production");
        assert!(matches!(
            validator.validate_path(&home_env_path.to_string_lossy()),
            Err(SecurityError::AccessDenied(_))
        ));

        let ssh_path = home.join(".ssh").join("config");
        assert!(matches!(
            validator.validate_path(&ssh_path.to_string_lossy()),
            Err(SecurityError::AccessDenied(_))
        ));

        let kube_path = home.join(".kube").join("config");
        assert!(matches!(
            validator.validate_path(&kube_path.to_string_lossy()),
            Err(SecurityError::AccessDenied(_))
        ));
    }

    #[cfg(unix)]
    {
        assert!(matches!(
            validator.validate_path("/root/.ssh/id_rsa"),
            Err(SecurityError::AccessDenied(_))
        ));
        assert!(matches!(
            validator.validate_path("/home/otheruser/.aws/credentials"),
            Err(SecurityError::AccessDenied(_))
        ));
        assert!(matches!(
            validator.validate_path("/Users/otheruser/.gnupg/pubring.kbx"),
            Err(SecurityError::AccessDenied(_))
        ));
        assert!(matches!(
            validator.validate_path("/home/otheruser/.kube/config"),
            Err(SecurityError::AccessDenied(_))
        ));
        assert!(matches!(
            validator.validate_path("/home/otheruser/.local/share/keyrings/login.keyring"),
            Err(SecurityError::AccessDenied(_))
        ));
        assert!(matches!(
            validator.validate_path("/etc/shadow"),
            Err(SecurityError::AccessDenied(_))
        ));
        assert!(matches!(
            validator.validate_path("/etc/sudoers"),
            Err(SecurityError::AccessDenied(_))
        ));
    }
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
    assert_eq!(
        SecurityValidator::extract_filename("C:\\Users\\user\\Downloads\\test.pdf"),
        Some("test.pdf".to_string())
    );
    assert_eq!(
        SecurityValidator::extract_filename("/home/user/downloads/test.pdf"),
        Some("test.pdf".to_string())
    );
    assert_eq!(
        SecurityValidator::extract_filename("C:/Users/user\\Downloads\\test.pdf"),
        Some("test.pdf".to_string())
    );
    assert_eq!(
        SecurityValidator::extract_filename("test.pdf"),
        Some("test.pdf".to_string())
    );
    assert_eq!(
        SecurityValidator::extract_filename(""),
        Some("".to_string())
    );
    assert_eq!(
        SecurityValidator::extract_filename("C:\\Users\\"),
        Some("".to_string())
    );
}

#[test]
#[cfg(unix)]
fn test_symlink_to_general_external_file_is_allowed() {
    use std::os::unix::fs::symlink;

    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_with_base_dir(temp_dir.path().to_path_buf());

    let outside_dir = tempdir().expect("outside dir");
    let external_file = outside_dir.path().join("external.txt");
    std::fs::write(&external_file, "safe").expect("write external file");

    let link_path = temp_dir.path().join("safe_link");
    symlink(&external_file, &link_path).expect("create symlink");

    let expected_link_path = temp_dir
        .path()
        .canonicalize()
        .expect("canonical temp dir")
        .join("safe_link");

    assert_eq!(
        validator
            .validate_path("safe_link")
            .expect("symlink to general file should be allowed"),
        expected_link_path
    );
}

#[test]
#[cfg(unix)]
fn test_symlink_to_sensitive_target_is_blocked() {
    use std::os::unix::fs::symlink;

    let Some(home) = dirs::home_dir() else {
        return;
    };
    let ssh_dir = home.join(".ssh");
    if !ssh_dir.exists() {
        return;
    }

    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_with_base_dir(temp_dir.path().to_path_buf());
    let link_path = temp_dir.path().join("ssh_link");
    symlink(&ssh_dir, &link_path).expect("create symlink");

    assert!(matches!(
        validator.validate_path("ssh_link/config"),
        Err(SecurityError::AccessDenied(_))
    ));
}

#[test]
#[cfg(unix)]
fn test_dangling_symlink_parent_is_rejected() {
    use std::os::unix::fs::symlink;

    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_with_base_dir(temp_dir.path().to_path_buf());

    let outside_dir = tempdir().expect("outside dir");
    let missing_target_dir = outside_dir.path().join("missing-target");

    let link_path = temp_dir.path().join("dangling_link_dir");
    symlink(&missing_target_dir, &link_path).expect("create symlink");

    let result = validator.validate_path("dangling_link_dir/new_file.txt");
    assert!(matches!(result, Err(SecurityError::PathTraversal(_))));
}

#[test]
fn test_scoped_validator_blocks_absolute_paths_outside_base_dir() {
    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_scoped_with_base_dir(temp_dir.path().to_path_buf());
    let outside_dir = tempdir().expect("outside dir");
    let outside_file = outside_dir.path().join("safe.txt");

    assert!(matches!(
        validator.validate_path_for_read(&outside_file.to_string_lossy()),
        Err(SecurityError::PathTraversal(_))
    ));
    assert!(matches!(
        validator.validate_path_for_write(&outside_file.to_string_lossy()),
        Err(SecurityError::PathTraversal(_))
    ));
}

#[test]
#[cfg(unix)]
fn test_scoped_validator_blocks_symlink_to_general_external_file() {
    use std::os::unix::fs::symlink;

    let temp_dir = tempdir().expect("temp dir");
    let validator = SecurityValidator::new_scoped_with_base_dir(temp_dir.path().to_path_buf());

    let outside_dir = tempdir().expect("outside dir");
    let external_file = outside_dir.path().join("external.txt");
    std::fs::write(&external_file, "safe").expect("write external file");

    let link_path = temp_dir.path().join("safe_link");
    symlink(&external_file, &link_path).expect("create symlink");

    assert!(matches!(
        validator.validate_path_for_read("safe_link"),
        Err(SecurityError::PathTraversal(_))
    ));
    assert!(matches!(
        validator.validate_path_for_write("safe_link"),
        Err(SecurityError::PathTraversal(_))
    ));
}
