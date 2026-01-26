use crate::mcp::builtin::browser::BrowserServer;
use crate::mcp::builtin::error_guidance::{
    invalid_input_error, missing_param_error, operation_failed_error, SuccessHint, ToolGroup,
};
use crate::mcp::types::MCPResult;
use serde_json::Value;

pub async fn click_element(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

    let selector = match args.get("selector").and_then(|v| v.as_str()) {
        Some(s) => s,
        Option::None => return Ok(missing_param_error("selector", ToolGroup::Browser)),
    };

    // Proactive CSS selector validation
    if selector.trim().is_empty() {
        return Ok(invalid_input_error(
            "CSS selector cannot be empty",
            ToolGroup::Browser,
        ));
    }

    let script = get_click_script(selector)?;
    match service.execute_script(&browser_session_id, &script).await {
        Ok(res) => {
            if res.contains("Element not found") {
                return Ok(operation_failed_error(
                    "Click element",
                    &format!("Element with selector '{}' not found", selector),
                    vec![
                        "Verify the selector is correct CSS syntax".to_string(),
                        "The element might be lazy-loaded. Use `scrollPage` to load more content down the page.".to_string(),
                        "Use listInteractable to find valid selectors".to_string(),
                    ],
                    ToolGroup::Browser,
                ));
            }
            if res.contains("Element not visible") {
                return Ok(operation_failed_error(
                    "Click element",
                    &format!("Element with selector '{}' is not visible", selector),
                    vec![
                        "The element exists but is hidden. Use `extractWebContent` to analyze the page structure and find a parent container or toggle button.".to_string(),
                        "The element might be lazy-loaded or off-screen. Use `scrollPage` to potentially trigger its visibility.".to_string(),
                        "Use `listInteractable` to find visible elements that might reveal this target.".to_string(),
                    ],
                    ToolGroup::Browser,
                ));
            }

            // ✅ Success: Return hints for next actions
            let hint = SuccessHint::new(
                res,
                vec![
                    "Use extractWebContent to see page changes after click".to_string(),
                    "Use getCurrentUrl to check if navigation occurred".to_string(),
                ],
            );
            Ok(hint.to_mcp_result())
        }
        Err(e) => {
            // ❌ Error: Only provide recovery guidance, no success hints
            Ok(operation_failed_error(
                "Click element",
                &e,
                vec![
                    "Verify the selector is correct CSS syntax".to_string(),
                    "Try using scrollPage to reveal lazy-loaded elements".to_string(),
                    "Use listInteractable to find valid selectors".to_string(),
                ],
                ToolGroup::Browser,
            ))
        }
    }
}

pub async fn input_text(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

    let selector = match args.get("selector").and_then(|v| v.as_str()) {
        Some(s) => s,
        Option::None => return Ok(missing_param_error("selector", ToolGroup::Browser)),
    };
    let text = match args.get("text").and_then(|v| v.as_str()) {
        Some(t) => t,
        Option::None => return Ok(missing_param_error("text", ToolGroup::Browser)),
    };

    // Proactive CSS selector validation
    if selector.trim().is_empty() {
        return Ok(invalid_input_error(
            "CSS selector cannot be empty",
            ToolGroup::Browser,
        ));
    }

    let selector_json =
        serde_json::to_string(selector).map_err(|e| format!("Serialization error: {}", e))?;
    let text_json =
        serde_json::to_string(text).map_err(|e| format!("Serialization error: {}", e))?;

    let script = format!(
        r#"(function() {{
            const el = document.querySelector({});
            if (!el) return 'Element not found';
            
            const style = window.getComputedStyle(el);
            if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {{
                return 'Element not visible';
            }}

            el.value = {};
            el.dispatchEvent(new Event('input', {{bubbles: true}}));
            el.dispatchEvent(new Event('change', {{bubbles: true}}));
            return 'Input successful';
        }})()"#,
        selector_json,
        text_json
    );

    match service.execute_script(&browser_session_id, &script).await {
        Ok(res) => {
            if res.contains("Element not found") {
                return Ok(operation_failed_error(
                    "Input text",
                    &format!("Element with selector '{}' not found", selector),
                    vec![
                        "Verify the selector targets an input/textarea element".to_string(),
                        "The element might be lazy-loaded. Use `scrollPage` to load more content down the page.".to_string(),
                        "Use listInteractable to find valid selectors".to_string(),
                    ],
                    ToolGroup::Browser,
                ));
            }
            if res.contains("Element not visible") {
                return Ok(operation_failed_error(
                    "Input text",
                    &format!("Element with selector '{}' is not visible", selector),
                    vec![
                        "The input is hidden. Use `extractWebContent` to find the form section or toggle that contains it.".to_string(),
                        "The element might be lazy-loaded or off-screen. Use `scrollPage` to potentially trigger its visibility.".to_string(),
                        "Use `clickElement` on the parent container or toggle to reveal the input.".to_string(),
                    ],
                    ToolGroup::Browser,
                ));
            }

            // ✅ Success: Return hints for next actions
            let hint = SuccessHint::new(
                res,
                vec![
                    "Use clickElement to submit the form or click buttons".to_string(),
                    "Use extractWebContent to verify input changes".to_string(),
                ],
            );
            Ok(hint.to_mcp_result())
        }
        Err(e) => {
            // ❌ Error: Only provide recovery guidance, no success hints
            Ok(operation_failed_error(
                "Input text",
                &e,
                vec![
                    "Verify the selector targets an input/textarea element".to_string(),
                    "Try using scrollPage to reveal lazy-loaded elements".to_string(),
                    "Use listInteractable with filterType='semantic_input'".to_string(),
                ],
                ToolGroup::Browser,
            ))
        }
    }
}

pub async fn scroll_page(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

    let x = match args.get("x").and_then(|v| v.as_f64()) {
        Some(x_val) => x_val,
        Option::None => return Ok(missing_param_error("x", ToolGroup::Browser)),
    };
    let y = match args.get("y").and_then(|v| v.as_f64()) {
        Some(y_val) => y_val,
        Option::None => return Ok(missing_param_error("y", ToolGroup::Browser)),
    };

    let script = format!("window.scrollTo({}, {}); 'Scrolled'", x, y);
    let result = match service.execute_script(&browser_session_id, &script).await {
        Ok(res) => res,
        Err(e) => {
            return Ok(operation_failed_error(
                "Scroll page",
                &e,
                vec![
                    "Verify the browser session is active".to_string(),
                    "Check if the page has scrollable content".to_string(),
                ],
                ToolGroup::Browser,
            ))
        }
    };

    let hint = SuccessHint::new(
        result,
        vec![
            "Use `listInteractable` to find elements in the new viewport position.".to_string(),
            "If the page uses lazy loading (infinite scroll), use `extractWebContent` to capture newly loaded content.".to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}

pub async fn list_interactable(server: &BrowserServer, args: Value) -> Result<MCPResult, String> {
    let service = server.get_browser_service()?;

    // Get browser session ID from server instance
    let browser_session_id = {
        let guard = server
            .browser_session_id
            .read()
            .map_err(|e| e.to_string())?;
        guard.clone()
    };

    let browser_session_id = browser_session_id
        .ok_or_else(|| "No active browser session. Call createSession first.".to_string())?;

    let filter_type = args
        .get("filterType")
        .and_then(|v| v.as_str())
        .unwrap_or("semantic_clickable");
    let scope = args
        .get("scope")
        .and_then(|v| v.as_str())
        .unwrap_or("viewport");

    // Proactive filterType validation
    let valid_filters = ["semantic_clickable", "semantic_input", "all_focusable"];
    if !valid_filters.contains(&filter_type) {
        return Ok(invalid_input_error(
            &format!(
                "Invalid filterType: '{}'. Must be one of: {}",
                filter_type,
                valid_filters.join(", ")
            ),
            ToolGroup::Browser,
        ));
    }

    let script = get_filter_script(filter_type, scope);
    let result_json = match service.execute_script(&browser_session_id, &script).await {
        Ok(res) => res,
        Err(e) => {
            return Ok(operation_failed_error(
                "List interactable elements",
                &e,
                vec![
                    "Verify the browser session is active".to_string(),
                    "Ensure the page has fully loaded".to_string(),
                    "Try extractWebContent first to see page structure".to_string(),
                ],
                ToolGroup::Browser,
            ))
        }
    };

    // Parse and format results
    let formatted_text = match format_interactive_elements(&result_json, filter_type, scope) {
        Ok(text) => text,
        Err(e) => {
            return Ok(operation_failed_error(
                "Format interactable elements",
                &e,
                vec![
                    "The page may have returned unexpected data".to_string(),
                    "Try refreshing the page with navigateToUrl".to_string(),
                    "Use extractWebContent to verify page structure".to_string(),
                ],
                ToolGroup::Browser,
            ))
        }
    };

    let hint = SuccessHint::new(
        formatted_text,
        vec![
            "Use `clickElement` with the selector or index.".to_string(),
            "If the target is off-screen, use `scrollPage` to bring it into the viewport."
                .to_string(),
            "Use `extractWebContent` to see the full page structure regardless of scroll position."
                .to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}

/// Helper to inline the clickElement script
fn get_click_script(selector: &str) -> Result<String, String> {
    let selector_json =
        serde_json::to_string(selector).map_err(|e| format!("Serialization error: {}", e))?;

    Ok(format!(
        r#"(function() {{
            const el = document.querySelector({});
            if (!el) return 'Element not found';
            
            const style = window.getComputedStyle(el);
            if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') {{
                return 'Element not visible';
            }}

            el.scrollIntoView({{block: 'center'}});
            el.focus();
            el.click();
            return 'Clicked element';
        }})()"#,
        selector_json
    ))
}

/// Helper to inline the listInteractable filter script
fn get_filter_script(filter_type: &str, scope: &str) -> String {
    let filter_selector = match filter_type {
        "semantic_input" => "input:not([type=\"hidden\"]):not([disabled]), select:not([disabled]), textarea:not([disabled]), [contenteditable=\"true\"]",
        "all_focusable" => "a, button, input, select, textarea, [tabindex]:not([tabindex=\"-1\"]), [contenteditable]",
        _ => "a[href], button:not([disabled]), [role=\"button\"]:not([disabled]), [onclick], [role=\"link\"]" // default semantic_clickable
    };

    let scope_check = if scope == "viewport" {
        r#"
        const rect = el.getBoundingClientRect();
        if (rect.width === 0 || rect.height === 0) return false;
        const inViewport = (
            rect.top < window.innerHeight &&
            rect.bottom > 0 &&
            rect.left < window.innerWidth &&
            rect.right > 0
        );
        if (!inViewport) return false;
        "#
    } else {
        r#"
         const rect = el.getBoundingClientRect();
         if (rect.width === 0 || rect.height === 0) return false;
         "#
    };

    format!(
        r#"(function() {{
            const selector = "{}";
            const candidates = Array.from(document.querySelectorAll(selector));
            
            function isVisible(el) {{
                const style = window.getComputedStyle(el);
                if (style.display === 'none' || style.visibility === 'hidden' || style.opacity === '0') return false;
                {}
                return true;
            }}

            function getUniqueSelector(el) {{
                if (el.id) return '#' + CSS.escape(el.id);
                // Minimal fallback
                return el.tagName.toLowerCase(); 
            }}

            const visible = candidates.filter(isVisible).slice(0, 50).map((el, idx) => {{
                return {{
                    index: idx,
                    tag: el.tagName.toLowerCase(),
                    text: (el.textContent || '').trim().substring(0, 50),
                    attributes: {{
                        href: el.getAttribute('href'),
                        type: el.getAttribute('type'),
                        placeholder: el.getAttribute('placeholder'),
                        "aria-label": el.getAttribute('aria-label')
                    }},
                    selector: getUniqueSelector(el)
                }};
            }});

            return JSON.stringify(visible);
        }})()"#,
        filter_selector.replace("\"", "\\\""),
        scope_check
    )
}

/// Format interactive elements list to match TypeScript output format
fn format_interactive_elements(
    json_result: &str,
    filter_type: &str,
    scope: &str,
) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Element {
        index: usize,
        tag: String,
        text: String,
        attributes: serde_json::Map<String, Value>,
        selector: String,
    }

    let elements: Vec<Element> = serde_json::from_str(json_result)
        .map_err(|e| format!("Failed to parse elements JSON: {}", e))?;

    if elements.is_empty() {
        let filter_label = filter_type.replace('_', " ");
        let scope_label = if scope == "viewport" {
            "current viewport"
        } else {
            "page"
        };
        return Ok(format!(
            "No {} elements found in {}.",
            filter_label, scope_label
        ));
    }

    // Header with metadata
    let filter_label = filter_type.replace('_', " ");
    let scope_label = if scope == "viewport" {
        "viewport"
    } else {
        "page"
    };
    let mut output = format!(
        "Found {} {} element(s) in {}:\n\n",
        elements.len(),
        filter_label,
        scope_label
    );

    // Format each element
    for el in &elements {
        // Format attributes
        let attrs: Vec<String> = el
            .attributes
            .iter()
            .filter(|(_, v)| !v.is_null())
            .map(|(k, v)| {
                if let Some(s) = v.as_str() {
                    format!("{}=\"{}\"", k, s)
                } else {
                    String::new()
                }
            })
            .filter(|s| !s.is_empty())
            .collect();

        let attr_str = if !attrs.is_empty() {
            format!(" {}", attrs.join(" "))
        } else {
            String::new()
        };

        let text_str = if !el.text.is_empty() {
            format!(" \"{}\"", el.text)
        } else {
            String::new()
        };

        output.push_str(&format!(
            "[{}] <{}{}>{}\n",
            el.index, el.tag, attr_str, text_str
        ));
        output.push_str(&format!("    Selector: {}\n\n", el.selector));
    }

    // Footer with usage hint
    output.push_str("💡 Use the selector or index to interact with these elements.");

    Ok(output)
}
