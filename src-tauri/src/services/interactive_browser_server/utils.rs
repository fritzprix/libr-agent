use reqwest;
use std::time::Duration;

/// Validates URL and returns normalized version.
/// Supports: http://, https://
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
                    // Replace 'about:blank' with a minimal data URI to ensure webview lifecycle triggers correctly
                    // about:blank specifically can fail to trigger 'PageLoad' events on some WebKit/WebView2 backends
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

/// Checks the HTTP status of a URL using reqwest.
pub async fn check_url_status(url: &str) -> Result<u16, String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 LibrAgent Browser")
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    // We use GET instead of HEAD to be more robust against servers that block HEAD or return 405.
    // reqwest does not download the body unless we consume the stream, so it's efficient.
    let response = client.get(url).send().await.map_err(|e| e.to_string())?;
    Ok(response.status().as_u16())
}
