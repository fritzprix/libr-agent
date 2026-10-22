use super::files::save_downloaded_file;
use super::markdown::{convert_to_markdown, extract_html_title};
use crate::browser_sidecar::BrowserAutomationClient;
use crate::mcp::builtin::browser::{handle_browser_op_error, BrowserServer};
use crate::mcp::builtin::error_guidance::{
    guided_error, missing_param_error, ErrorCategory, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::{json, Value};
use tokio::task;
use uuid::Uuid;

pub async fn fetch_url(
    server: &BrowserServer,
    args: Value,
    session_id: Option<String>,
) -> Result<MCPResult, String> {
    let url = match args.get("url").and_then(|v| v.as_str()) {
        Some(u) => u.trim().to_string(),
        Option::None => return Ok(missing_param_error("url", ToolGroup::Browser)),
    };

    let scheme = url.split_once(':').map(|(s, _)| s.to_ascii_lowercase());
    if scheme.as_deref() == Some("file") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            "Local file URLs are not supported by browser__fetchUrl. Use http:// or https:// URLs only",
            ToolGroup::Browser,
        )
        .guidance(vec![
            "Use workspace tools (listDirectory/readFile) for local files.".to_string(),
            "If the file is a session attachment, use attachments__listAttachments / readAttachment."
                .to_string(),
        ])
        .to_mcp_result());
    }

    if scheme.as_deref() != Some("http") && scheme.as_deref() != Some("https") {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            format!(
                "Invalid URL format: '{}'. browser__fetchUrl requires http:// or https://",
                url
            ),
            ToolGroup::Browser,
        )
        .to_mcp_result());
    }

    let save_path = args
        .get("savePath")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // 1. Fast HTTP probe for content-type / static HTML (avoids headless for simple pages).
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| e.to_string())?;

    let get_res = client.get(&url).send().await;
    let mut is_html = true;
    let mut static_html: Option<String> = None;

    if let Ok(res) = get_res {
        if res.status().is_success() {
            if let Some(content_type) = res.headers().get(reqwest::header::CONTENT_TYPE) {
                if let Ok(ct_str) = content_type.to_str() {
                    let ct = ct_str.to_lowercase();
                    if !ct.contains("text/html")
                        && !ct.contains("text/plain")
                        && !ct.contains("application/xhtml+xml")
                    {
                        is_html = false;
                    }
                }
            }

            if !is_html {
                if let Some(path) = save_path.as_ref() {
                    let bytes = match res.bytes().await {
                        Ok(b) => b,
                        Err(e) => {
                            return Ok(handle_browser_op_error(
                                "Download File",
                                e.to_string(),
                                vec![],
                            ));
                        }
                    };
                    return save_downloaded_file(&url, path, session_id.as_deref(), &bytes).await;
                }
            } else {
                match res.text().await {
                    Ok(html) => static_html = Some(html),
                    Err(e) => {
                        warn_fetch_http_body(&url, &e);
                    }
                }
            }
        }
    }

    if !is_html && save_path.is_none() {
        return Ok(guided_error(
            ErrorCategory::InvalidInput,
            "The URL points to a file, not a web page. A 'savePath' is required to download it.",
            ToolGroup::Browser,
        )
        .guidance(vec![
            "Provide a 'savePath' parameter to save the file.".to_string(),
            "If you meant to read it, download it first, then use workspace tools to read the file.".to_string()
        ])
        .to_mcp_result());
    }

    // 2. Prefer static HTML when it already yields usable markdown.
    if let Some(html) = static_html.as_ref() {
        let html_clone = html.clone();
        let markdown_content = task::spawn_blocking(move || {
            if html_clone.len() > 10 * 1024 * 1024 {
                return "**Error: Page content too large to process (exceeds 10MB limit).**"
                    .to_string();
            }
            convert_to_markdown(&html_clone)
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?;

        if is_usable_static_fetch(html, &markdown_content) {
            let title = extract_html_title(html).unwrap_or_default();
            return Ok(build_fetch_success(
                &url,
                &title,
                &markdown_content,
                "http",
                None,
            ));
        }
    }

    // 3. Headless fallback for JS-rendered / bot-gated pages.
    match fetch_url_via_headless(server, &url).await {
        Ok((title, markdown_content, nav_note)) => Ok(build_fetch_success(
            &url,
            &title,
            &markdown_content,
            "headless",
            nav_note.as_deref(),
        )),
        Err(headless_error) => {
            // Soft fallback: return imperfect static HTML rather than failing entirely.
            if let Some(html) = static_html {
                let html_clone = html.clone();
                let markdown_content =
                    task::spawn_blocking(move || convert_to_markdown(&html_clone))
                        .await
                        .map_err(|e| format!("Task join error: {}", e))?;
                if !markdown_content.trim().is_empty() {
                    let title = extract_html_title(&html).unwrap_or_default();
                    let note = format!(
                        "Headless fetch failed ({headless_error}); returned static HTTP HTML which may be incomplete."
                    );
                    return Ok(build_fetch_success(
                        &url,
                        &title,
                        &markdown_content,
                        "http_fallback",
                        Some(&note),
                    ));
                }
            }
            Ok(handle_browser_op_error(
                "Fetch URL",
                headless_error,
                vec!["Check URL", "Try a different site"],
            ))
        }
    }
}

fn warn_fetch_http_body(url: &str, error: &reqwest::Error) {
    log::warn!("fetchUrl HTTP body read failed for {url}: {error}");
}

/// Heuristic: static HTML is good enough when markdown is substantial and not a bot wall.
fn is_usable_static_fetch(html: &str, markdown: &str) -> bool {
    let md = markdown.trim();
    if md.len() < 400 {
        return false;
    }

    let lower = html.to_lowercase();
    const BLOCKERS: &[&str] = &[
        "cf-browser-verification",
        "just a moment...",
        "checking your browser",
        "enable javascript to continue",
        "please enable cookies",
        "_incapsula_",
        "attention required! | cloudflare",
    ];
    if BLOCKERS.iter().any(|marker| lower.contains(marker)) && md.len() < 2_000 {
        return false;
    }

    true
}

fn build_fetch_success(
    url: &str,
    title: &str,
    markdown_content: &str,
    method: &str,
    note: Option<&str>,
) -> MCPResult {
    let mut result_text = format!(
        "Content fetched from {}\n\nPage Title: {}\n\n{}",
        url, title, markdown_content
    );

    if let Some(note) = note {
        result_text.push_str("\n\n(");
        result_text.push_str(note);
        result_text.push(')');
    }

    if markdown_content.trim().is_empty() {
        result_text.push_str(
            "\n\n(Empty Content) The fetched page returned no readable markdown. The page may be blank, script-rendered, or blocked. Try createSession/navigateToUrl for interactive inspection, or retry with a different URL.",
        );
    }

    let structured_data = json!({
         "url": url,
         "title": title,
         "content_length": markdown_content.len(),
         "content_empty": markdown_content.trim().is_empty(),
         "fetch_method": method
    });

    let hint = SuccessHint::new(
        result_text,
        vec![
            "Use browser__createSession and browser__navigateToUrl if you need interactive inspection of this page"
                .to_string(),
            "Use workspace tools only if you downloaded a file instead of page content".to_string(),
        ],
    );
    hint.to_mcp_result_with_data(Some(structured_data))
}

async fn fetch_url_via_headless(
    server: &BrowserServer,
    url: &str,
) -> Result<(String, String, Option<String>), String> {
    // Temporary headless sidecar so fetch does not conflict with a visible interactive runtime.
    let service = server.get_browser_service()?;
    let action_timeout = service.action_timeout();
    let fetch_client = BrowserAutomationClient::new(action_timeout);
    let fetch_session_id = format!("fetch-{}", Uuid::new_v4().simple());

    let create_state = match fetch_client
        .create_session(&fetch_session_id, url, Some("Fetch Tool Session"), false)
        .await
    {
        Ok(state) => state,
        Err(e) => {
            fetch_client.shutdown().await;
            return Err(e);
        }
    };
    let nav_note = create_state.navigation_message.clone();

    let raw_html = match fetch_client
        .evaluate(
            &fetch_session_id,
            "document.documentElement ? document.documentElement.outerHTML : (document.body ? document.body.outerHTML : \"\")",
        )
        .await
    {
        Ok(html) => html,
        Err(e) => {
            let _ = fetch_client.close_session(&fetch_session_id).await;
            fetch_client.shutdown().await;
            return Err(e);
        }
    };

    let page_title = fetch_client
        .evaluate(&fetch_session_id, "document.title")
        .await
        .unwrap_or_else(|_| create_state.title.clone().unwrap_or_default());

    let _ = fetch_client.close_session(&fetch_session_id).await;
    fetch_client.shutdown().await;

    let raw_html_clone = raw_html.clone();
    let markdown_content = task::spawn_blocking(move || {
        if raw_html_clone.len() > 10 * 1024 * 1024 {
            return "**Error: Page content too large to process (exceeds 10MB limit).**"
                .to_string();
        }
        convert_to_markdown(&raw_html_clone)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    let title = page_title.trim_matches('"').to_string();
    Ok((title, markdown_content, nav_note))
}
