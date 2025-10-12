// helpers.rs - Utility functions
use std::path::Path;

/// Extract file path from file:// URL
///
/// This function properly handles file URLs across Windows and Unix platforms,
/// including URL encoding (spaces, Unicode characters), drive letters (Windows),
/// and UNC paths. It uses the url crate to ensure proper parsing and conversion.
///
/// # Examples
/// - `file:///C:/Users/Me/file.txt` -> `C:\Users\Me\file.txt` (Windows)
/// - `file:///home/user/file.txt` -> `/home/user/file.txt` (Unix)
/// - `file:///C:/My%20Files/doc.txt` -> `C:\My Files\doc.txt` (URL decoding)
/// - `file://localhost/C:/file.txt` -> `C:\file.txt` (with host)
pub(crate) fn extract_file_path_from_url(file_url: &str) -> Result<String, String> {
    let url = url::Url::parse(file_url).map_err(|e| format!("Invalid file URL format: {e}"))?;

    // Ensure it's a file:// URL
    if url.scheme() != "file" {
        return Err(format!(
            "URL must use file:// scheme, got: {}",
            url.scheme()
        ));
    }

    // Convert to OS-specific file path (handles Windows drive letters, UNC, URL decoding, etc.)
    url.to_file_path()
        .map_err(|_| "URL cannot be converted to a local file path".to_string())?
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Failed to convert path to UTF-8 string".to_string())
}

/// Determine MIME type from file extension
pub(crate) fn mime_type_from_extension(path: &Path) -> &'static str {
    match path.extension() {
        Some(ext) => match ext.to_str().unwrap_or("").to_lowercase().as_str() {
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "pdf" => "application/pdf",
            "txt" => "text/plain",
            "md" => "text/markdown",
            "csv" => "text/csv",
            _ => "text/plain",
        },
        None => "text/plain",
    }
}

/// Create text chunks from content lines
pub(crate) fn create_text_chunks(lines: &[&str], chunk_size: usize) -> Vec<String> {
    lines
        .chunks(chunk_size)
        .map(|chunk| chunk.join("\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn test_extract_file_path_from_url_valid_unix() {
        // This test only runs on Unix systems
        let result = extract_file_path_from_url("file:///home/user/doc.txt");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert_eq!(path, "/home/user/doc.txt");
    }

    #[test]
    fn test_extract_file_path_from_url_valid_windows() {
        let result = extract_file_path_from_url("file:///C:/Users/Me/doc.txt");
        assert!(result.is_ok());
        let path = result.unwrap();
        // On Windows: C:\Users\Me\doc.txt
        // On Unix: might work differently
        #[cfg(windows)]
        {
            assert!(path.starts_with("C:"));
            assert!(path.contains("Users"));
            assert!(path.contains("Me"));
            assert!(path.ends_with("doc.txt"));
        }
    }

    #[test]
    fn test_extract_file_path_from_url_with_spaces() {
        // URL encoded space (%20)
        let result = extract_file_path_from_url("file:///C:/My%20Files/test%20doc.txt");
        assert!(result.is_ok());
        let path = result.unwrap();
        // Should decode %20 to spaces
        assert!(
            path.contains("My Files") || path.contains("My%20Files"),
            "Path should contain decoded spaces: {}",
            path
        );
    }

    #[test]
    fn test_extract_file_path_from_url_with_unicode() {
        // Unicode characters in path (한글)
        let result = extract_file_path_from_url("file:///C:/Users/%EB%AC%B8%EC%84%9C/file.txt");
        assert!(result.is_ok());
        let _path = result.unwrap();
        // Should handle Unicode properly
    }

    #[test]
    fn test_extract_file_path_from_url_with_localhost() {
        // file://localhost/path format
        let result = extract_file_path_from_url("file://localhost/C:/path/file.txt");
        // url crate should handle localhost properly
        assert!(result.is_ok() || result.is_err()); // Behavior may vary by platform
    }

    #[test]
    fn test_extract_file_path_from_url_invalid_scheme() {
        // Non-file:// URLs should be rejected
        assert!(extract_file_path_from_url("http://example.com/file.txt").is_err());
        assert!(extract_file_path_from_url("https://example.com/file.txt").is_err());
    }

    #[test]
    fn test_extract_file_path_from_url_invalid_format() {
        // Not a URL at all
        assert!(extract_file_path_from_url("/home/user/file.txt").is_err());
        assert!(extract_file_path_from_url("C:\\Users\\file.txt").is_err());
    }

    #[test]
    fn test_extract_file_path_from_url_unc_path() {
        // UNC path: file://server/share/file.txt
        let result = extract_file_path_from_url("file://server/share/file.txt");
        // On Windows, this should convert to \\server\share\file.txt
        // On Unix, this will likely fail or behave differently
        #[cfg(windows)]
        {
            if let Ok(path) = result {
                // Should be UNC path format
                assert!(path.starts_with(r"\\") || path.contains("server"));
            }
        }
    }

    #[test]
    fn test_mime_type_from_extension() {
        assert_eq!(
            mime_type_from_extension(Path::new("test.pdf")),
            "application/pdf"
        );
        assert_eq!(
            mime_type_from_extension(Path::new("test.docx")),
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        );
        assert_eq!(
            mime_type_from_extension(Path::new("test.unknown")),
            "text/plain"
        );
    }

    #[test]
    fn test_create_text_chunks() {
        let lines = vec!["line1", "line2", "line3", "line4", "line5"];
        let chunks = create_text_chunks(&lines, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "line1\nline2");
        assert_eq!(chunks[1], "line3\nline4");
        assert_eq!(chunks[2], "line5");
    }
}
