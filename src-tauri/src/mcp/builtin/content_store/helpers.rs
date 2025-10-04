// helpers.rs - Utility functions
use std::path::Path;

/// Extract file path from file:// URL
pub(crate) fn extract_file_path_from_url(file_url: &str) -> Result<String, String> {
    if let Some(path) = file_url.strip_prefix("file://") {
        Ok(path.to_string())
    } else {
        Err(format!("Invalid file URL format: {file_url}"))
    }
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
    fn test_extract_file_path_from_url_valid() {
        assert_eq!(
            extract_file_path_from_url("file:///home/user/doc.txt"),
            Ok("/home/user/doc.txt".to_string())
        );
    }

    #[test]
    fn test_extract_file_path_from_url_invalid() {
        assert!(extract_file_path_from_url("http://example.com/file.txt").is_err());
        assert!(extract_file_path_from_url("/home/user/file.txt").is_err());
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
