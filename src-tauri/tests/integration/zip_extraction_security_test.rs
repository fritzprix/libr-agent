use std::fs;
use tauri_mcp_agent_lib::utils::fs::extract_zip_secure;
use zip::write::FileOptions;
use zip::ZipWriter;

#[test]
fn test_secure_zip_extraction_valid() {
    // 1. Create a valid zip archive in memory
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        // Add a simple file
        zip.start_file("hello.txt", options).unwrap();
        use std::io::Write;
        zip.write_all(b"Hello World").unwrap();

        // Add a nested file
        zip.start_file("nested/test.txt", options).unwrap();
        zip.write_all(b"Nested content").unwrap();

        zip.finish().unwrap();
    }

    // Reset cursor position
    buffer.set_position(0);

    // 2. Extract securely
    let temp_dir = tempfile::tempdir().unwrap();
    let mut archive = zip::ZipArchive::new(buffer).unwrap();

    let result = extract_zip_secure(&mut archive, temp_dir.path());
    assert!(
        result.is_ok(),
        "Extraction should succeed for valid zip: {:?}",
        result.err()
    );

    // 3. Verify files exist
    let hello_path = temp_dir.path().join("hello.txt");
    assert!(hello_path.exists(), "hello.txt should exist");
    assert_eq!(fs::read_to_string(hello_path).unwrap(), "Hello World");

    let nested_path = temp_dir.path().join("nested/test.txt");
    assert!(nested_path.exists(), "nested/test.txt should exist");
    assert_eq!(fs::read_to_string(nested_path).unwrap(), "Nested content");
}

#[test]
fn test_secure_zip_extraction_malicious_path_ignored() {
    // Create a zip archive containing a path traversal entry ("../evil.txt").
    // ZipWriter does NOT sanitize paths on write, so this successfully produces
    // a zip with a malicious entry that real-world attackers could craft.
    let mut buffer = std::io::Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        use std::io::Write;

        // Valid file inside the archive root
        zip.start_file("good.txt", options).unwrap();
        zip.write_all(b"Good content").unwrap();

        // Malicious traversal entry — must NOT escape the extraction directory
        zip.start_file("../evil.txt", options).unwrap();
        zip.write_all(b"EVIL").unwrap();

        zip.finish().unwrap();
    }

    buffer.set_position(0);

    let temp_dir = tempfile::tempdir().unwrap();
    let temp_dir_path = temp_dir.path().to_path_buf();
    let parent_dir = temp_dir_path
        .parent()
        .expect("temp dir should have a parent")
        .to_path_buf();
    let outside_path = parent_dir.join("evil.txt");

    // Remove any pre-existing file to avoid false positives
    if outside_path.exists() {
        fs::remove_file(&outside_path).unwrap();
    }

    let mut archive = zip::ZipArchive::new(buffer).unwrap();
    let result = extract_zip_secure(&mut archive, &temp_dir_path);
    assert!(
        result.is_ok(),
        "Extraction should succeed even with malicious entry: {:?}",
        result.err()
    );

    // Verify the valid file was extracted
    let good_path = temp_dir_path.join("good.txt");
    assert!(
        good_path.exists(),
        "good.txt should exist inside extraction directory"
    );
    assert_eq!(fs::read_to_string(&good_path).unwrap(), "Good content");

    // Verify the malicious path did NOT create a file outside the extraction directory
    assert!(
        !outside_path.exists(),
        "Malicious entry must not create a file outside the extraction directory: {:?}",
        outside_path
    );

    // Also ensure the malicious entry was not silently flattened into the extraction root
    let inside_evil = temp_dir_path.join("evil.txt");
    assert!(
        !inside_evil.exists(),
        "Malicious entry should be skipped entirely, not extracted as evil.txt in the root"
    );
}
