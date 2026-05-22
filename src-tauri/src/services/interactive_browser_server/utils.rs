/// Validates URL and returns a normalized version.
/// Supports: http://, https://, about:
///
/// # Arguments
/// * `url` - The URL to validate
///
/// # Returns
/// A `Result` containing the normalized URL on success, or an error string on failure.
pub fn validate_and_normalize_url(url: &str) -> Result<String, String> {
    let parsed_result = url::Url::parse(url);

    match parsed_result {
        Ok(parsed) => {
            // Determine if we should allow based on scheme
            match parsed.scheme() {
                "http" | "https" => Ok(url.to_string()),
                "about" => {
                    // Replace about:blank with a deterministic HTML document so the
                    // sidecar-backed browser session always lands on a readable page.
                    Ok("data:text/html,<html><body><h1>Agent Ready</h1></body></html>".to_string())
                }
                scheme => Err(format!(
                    "Unsupported URL scheme '{}'. Allowed: http://, https://, about:",
                    scheme
                )),
            }
        }
        Err(_) => {
            // Try prepending https://
            let with_proto = format!("https://{}", url);
            if let Ok(_parsed) = url::Url::parse(&with_proto) {
                return Ok(with_proto);
            }

            Err(format!("Invalid URL format: {}", url))
        }
    }
}
