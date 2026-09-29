//! Content source parsing (HTTP URL vs local file path).

use url::Url;

/// Describes where the content bytes come from.
pub(super) enum ContentSource {
    /// An HTTP/HTTPS URL to be fetched with reqwest.
    Http(String),
    /// An absolute or resolved local file path.
    LocalFile(std::path::PathBuf),
}

/// Parse the `url` argument into a `ContentSource`.
///
/// Accepts:
/// - `https://…` / `http://…`  → `Http`
/// - `file:///path`             → `LocalFile`
/// - Any other string           → treated as a local path → `LocalFile`
pub(super) fn parse_source(url: &str) -> Result<ContentSource, String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        Ok(ContentSource::Http(url.to_string()))
    } else if url.starts_with("file://") {
        let parsed = Url::parse(url).map_err(|e| format!("Invalid file URL format: {e}"))?;
        let path = parsed
            .to_file_path()
            .map_err(|_| "URL cannot be converted to a local file path".to_string())?;
        Ok(ContentSource::LocalFile(path))
    } else {
        Ok(ContentSource::LocalFile(std::path::PathBuf::from(url)))
    }
}
