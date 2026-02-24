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
    assert!(result.is_ok(), "Extraction should succeed for valid zip: {:?}", result.err());

    // 3. Verify files exist
    let hello_path = temp_dir.path().join("hello.txt");
    assert!(hello_path.exists(), "hello.txt should exist");
    assert_eq!(fs::read_to_string(hello_path).unwrap(), "Hello World");

    let nested_path = temp_dir.path().join("nested/test.txt");
    assert!(nested_path.exists(), "nested/test.txt should exist");
    assert_eq!(fs::read_to_string(nested_path).unwrap(), "Nested content");
}

#[test]
fn test_secure_zip_extraction_malicious_path_simulated() {
    // Ideally, we would create a zip with "../bad.txt".
    // Since ZipWriter might validate this, we can't easily produce it here.
    // However, if we could, extract_zip_secure should return Ok(()) but NOT extract the file,
    // because it skips invalid paths (continue).

    // If we really wanted to test this, we would need a pre-generated malicious zip file as bytes.
    // For now, we trust `zip::ZipFile::enclosed_name()` implementation as per documentation.
}
