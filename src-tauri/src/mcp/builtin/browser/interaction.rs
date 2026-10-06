use crate::mcp::builtin::browser::BrowserServer;
use crate::mcp::builtin::error_guidance::{
    guided_error, invalid_input_error, missing_param_error, operation_failed_error, ErrorCategory,
    SuccessHint, ToolGroup,
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

    let browser_session_id = match browser_session_id {
        Some(id) => id,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                "No active browser session found for this agent",
                ToolGroup::Browser,
            )
            .guidance(vec![
                "Use browser__createSession FIRST to start a browser session".to_string(),
                "Wait for browser__createSession to return a success message before clicking elements"
                    .to_string(),
            ])
            .to_mcp_result());
        }
    };

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

    match service
        .click_element(&browser_session_id, selector)
        .await
    {
        Ok(res) => match classify_dom_action_result(&res, "Clicked element") {
            DomActionOutcome::Success => {
                create_rich_response(&service, &browser_session_id, "Clicked element").await
            }
            DomActionOutcome::NotFound => {
                let suggestions = suggest_selectors(&service, &browser_session_id).await;
                Ok(operation_failed_error(
                    "Click element",
                    &format!("Element with selector '{}' not found{}", selector, suggestions),
                    vec![
                        "Verify the selector is correct CSS syntax".to_string(),
                        "The element might be lazy-loaded. Use `scrollPage` to load more content down the page.".to_string(),
                        "Use browser__listInteractable to find valid selectors".to_string(),
                    ],
                    ToolGroup::Browser,
                ))
            }
            DomActionOutcome::NotVisible => Ok(operation_failed_error(
                "Click element",
                &format!("Element with selector '{}' is not visible", selector),
                vec![
                    "The element exists but is hidden. Use `browser__getPageContent({})` to analyze the page structure and find a parent container or toggle button.".to_string(),
                    "The element might be lazy-loaded or off-screen. Use `scrollPage` to potentially trigger its visibility.".to_string(),
                    "Use `browser__listInteractable` to find visible elements that might reveal this target.".to_string(),
                ],
                ToolGroup::Browser,
            )),
            DomActionOutcome::Other(message) => Ok(operation_failed_error(
                "Click element",
                &format!(
                    "Unexpected click result for selector '{}': {}",
                    selector, message
                ),
                vec![
                    "Verify the selector is correct CSS syntax".to_string(),
                    "Use browser__listInteractable to find valid selectors".to_string(),
                    "On userChrome, reload the Chrome extension after updating LibrAgent".to_string(),
                ],
                ToolGroup::Browser,
            )),
        },
        Err(e) => Ok(operation_failed_error(
            "Click element",
            &e,
            vec![
                "Verify the selector is correct CSS syntax".to_string(),
                "Try using `scrollPage` to reveal lazy-loaded elements".to_string(),
                "Use browser__listInteractable to find valid selectors".to_string(),
                "On userChrome, reload the Chrome extension after updating LibrAgent (Unsupported method: clickElement means a stale service worker)".to_string(),
            ],
            ToolGroup::Browser,
        )),
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

    let browser_session_id = match browser_session_id {
        Some(id) => id,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                "No active browser session found for this agent",
                ToolGroup::Browser,
            )
            .guidance(vec![
                "Use browser__createSession FIRST to start a browser session".to_string(),
                "Wait for browser__createSession to return a success message before inputting text"
                    .to_string(),
            ])
            .to_mcp_result());
        }
    };

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

    match service
        .input_text(&browser_session_id, selector, text)
        .await
    {
        Ok(res) => match classify_dom_action_result(&res, "Input successful") {
            DomActionOutcome::Success => {
                create_rich_response(&service, &browser_session_id, "Input successful").await
            }
            DomActionOutcome::NotFound => {
                let suggestions = suggest_selectors(&service, &browser_session_id).await;
                Ok(operation_failed_error(
                    "Input text",
                    &format!("Element with selector '{}' not found{}", selector, suggestions),
                    vec![
                        "Verify the selector targets an input/textarea element".to_string(),
                        "The element might be lazy-loaded. Use `scrollPage` to load more content down the page.".to_string(),
                        "Use browser__listInteractable to find valid selectors".to_string(),
                    ],
                    ToolGroup::Browser,
                ))
            }
            DomActionOutcome::NotVisible => Ok(operation_failed_error(
                "Input text",
                &format!("Element with selector '{}' is not visible", selector),
                vec![
                    "The input is hidden. Use `browser__getPageContent({})` to find the form section or toggle that contains it.".to_string(),
                    "The element might be lazy-loaded or off-screen. Use `scrollPage` to potentially trigger its visibility.".to_string(),
                    "Use `browser__clickElement` on the parent container or toggle to reveal the input.".to_string(),
                ],
                ToolGroup::Browser,
            )),
            DomActionOutcome::Other(message) => Ok(operation_failed_error(
                "Input text",
                &format!(
                    "Unexpected input result for selector '{}': {}",
                    selector, message
                ),
                vec![
                    "Verify the selector targets an input/textarea/contenteditable element"
                        .to_string(),
                    "Use browser__listInteractable with filterType='semantic_input'".to_string(),
                    "On userChrome, reload the Chrome extension after updating LibrAgent".to_string(),
                ],
                ToolGroup::Browser,
            )),
        },
        Err(e) => Ok(operation_failed_error(
            "Input text",
            &e,
            vec![
                "Verify the selector targets an input/textarea element".to_string(),
                "Try using `scrollPage` to reveal lazy-loaded elements".to_string(),
                "Use browser__listInteractable with filterType='semantic_input'".to_string(),
                "On userChrome, reload the Chrome extension after updating LibrAgent (Unsupported method: inputText means a stale service worker)".to_string(),
            ],
            ToolGroup::Browser,
        )),
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

    let browser_session_id = match browser_session_id {
        Some(id) => id,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                "No active browser session found for this agent",
                ToolGroup::Browser,
            )
            .guidance(vec![
                "Use browser__createSession FIRST to start a browser session".to_string(),
                "Wait for browser__createSession to return a success message before scrolling"
                    .to_string(),
            ])
            .to_mcp_result());
        }
    };

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

    create_rich_response(&service, &browser_session_id, &result).await
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

    let browser_session_id = match browser_session_id {
        Some(id) => id,
        None => {
            return Ok(guided_error(
                ErrorCategory::ResourceNotFound,
                "No active browser session found for this agent",
                ToolGroup::Browser,
            )
            .guidance(vec![
                "Use browser__createSession FIRST to start a browser session".to_string(),
                "Wait for browser__createSession to return a success message before listing elements"
                    .to_string(),
            ])
            .to_mcp_result());
        }
    };

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
                    "Try `browser__getPageContent({})` first to see page structure".to_string(),
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
                    "If the page is stale or broken, use `browser__navigateToUrl` with a URL to replace the current page in this session.".to_string(),
                    "Use `browser__getPageContent({})` to verify page structure".to_string(),
                ],
                ToolGroup::Browser,
            ))
        }
    };

    let hint = SuccessHint::new(
        formatted_text,
        vec![
            "Use `browser__clickElement` with the selector.".to_string(),
            "If the target is off-screen, use `scrollPage` to bring it into the viewport."
                .to_string(),
            "Use `browser__getPageContent({})` to see the full page structure regardless of scroll position."
                .to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}

enum DomActionOutcome {
    Success,
    NotFound,
    NotVisible,
    Other(String),
}

/// Require an exact success/error token so `"null"` and echoed phrases are not false successes.
fn classify_dom_action_result(result: &str, success_token: &str) -> DomActionOutcome {
    let token = result.trim().trim_matches('"');
    if token == success_token {
        return DomActionOutcome::Success;
    }
    if token == "Element not found" {
        return DomActionOutcome::NotFound;
    }
    if token == "Element not visible" {
        return DomActionOutcome::NotVisible;
    }
    DomActionOutcome::Other(if token.is_empty() {
        "(empty)".to_string()
    } else {
        token.to_string()
    })
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

            function escapeAttr(value) {{
                if (typeof CSS !== 'undefined' && CSS.escape) return CSS.escape(value);
                return String(value).replace(/["\\\\]/g, '\\\\$&');
            }}

            function isUnique(sel) {{
                try {{ return document.querySelectorAll(sel).length === 1; }}
                catch (_) {{ return false; }}
            }}

            // Prefer stable, agent-usable CSS: #id → [name] → [name][value] → [type] →
            // aria/placeholder → scoped nth-of-type. Never emit a known-non-unique selector
            // mid-chain (continue searching); keep a best-effort fallback for the end.
            function getUniqueSelector(el) {{
                if (el.id) {{
                    const byId = '#' + escapeAttr(el.id);
                    if (isUnique(byId)) return byId;
                }}

                const tag = el.tagName.toLowerCase();
                const name = el.getAttribute('name');
                const type = el.getAttribute('type');
                const value = el.getAttribute('value');
                let fallback = tag;

                if (name) {{
                    const byName = tag + '[name="' + escapeAttr(name) + '"]';
                    if (isUnique(byName)) return byName;
                    fallback = byName;
                    if (value !== null && value !== '') {{
                        const byNameValue = byName + '[value="' + escapeAttr(value) + '"]';
                        if (isUnique(byNameValue)) return byNameValue;
                        fallback = byNameValue;
                    }}
                }}

                if (type) {{
                    const byType = tag + '[type="' + escapeAttr(type) + '"]';
                    if (isUnique(byType)) return byType;
                }}

                const aria = el.getAttribute('aria-label');
                if (aria) {{
                    if (name) {{
                        const byNameAria = tag + '[name="' + escapeAttr(name) + '"][aria-label="' + escapeAttr(aria) + '"]';
                        if (isUnique(byNameAria)) return byNameAria;
                    }}
                    const byAria = tag + '[aria-label="' + escapeAttr(aria) + '"]';
                    if (isUnique(byAria)) return byAria;
                }}

                const placeholder = el.getAttribute('placeholder');
                if (placeholder) {{
                    if (name) {{
                        const byNamePh = tag + '[name="' + escapeAttr(name) + '"][placeholder="' + escapeAttr(placeholder) + '"]';
                        if (isUnique(byNamePh)) return byNamePh;
                    }}
                    const byPh = tag + '[placeholder="' + escapeAttr(placeholder) + '"]';
                    if (isUnique(byPh)) return byPh;
                }}

                // :nth-of-type counts same-tag siblings (not attribute matches).
                const parent = el.parentElement;
                if (parent) {{
                    const siblings = Array.from(parent.children).filter(
                        (c) => c.tagName === el.tagName
                    );
                    const nth = siblings.indexOf(el) + 1;
                    if (nth > 0) {{
                        let base = tag;
                        if (type) base += '[type="' + escapeAttr(type) + '"]';
                        const relative = base + ':nth-of-type(' + nth + ')';
                        if (parent.id) {{
                            const withParent = '#' + escapeAttr(parent.id) + ' > ' + relative;
                            if (isUnique(withParent)) return withParent;
                        }}
                        const ancestor = el.closest('[id]');
                        if (ancestor && ancestor !== el) {{
                            const scoped = '#' + escapeAttr(ancestor.id) + ' ' + relative;
                            if (isUnique(scoped)) return scoped;
                        }}
                        if (isUnique(relative)) return relative;
                    }}
                }}

                return fallback;
            }}

            const visible = candidates.filter(isVisible).slice(0, 50).map((el, idx) => {{
                return {{
                    index: idx,
                    tag: el.tagName.toLowerCase(),
                    text: (el.textContent || '').trim().substring(0, 50),
                    attributes: {{
                        href: el.getAttribute('href'),
                        type: el.getAttribute('type'),
                        name: el.getAttribute('name'),
                        value: el.getAttribute('value'),
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

    let trimmed = json_result.trim();
    // userChrome evaluate often returns literal "null" under CSP; treat as empty, not a hard parse error.
    if trimmed.is_empty() || trimmed == "null" || trimmed == "undefined" {
        return Ok(
            "Interactable discovery unavailable: page CSP likely blocked evaluate on userChrome \
(or the script returned no data). Use known selectors with browser__clickElement / \
browser__inputText, or create a session with browser=\"sidecar\"."
                .to_string(),
        );
    }

    let elements: Vec<Element> = serde_json::from_str(trimmed)
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
    output.push_str("💡 Tip: Selectors can be used to interact with these elements.");

    Ok(output)
}

/// Post-action rich response: captures page title + URL to confirm result
pub async fn create_rich_response(
    service: &crate::services::InteractiveBrowserServer,
    session_id: &str,
    action_result: &str,
) -> Result<MCPResult, String> {
    // Prefer tabs/CDP page state — userChrome `evaluate` is unreliable under strict page CSP.
    let (title, url) = match service.get_page_state(session_id).await {
        Ok(state) => (
            state
                .title
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| "Unknown Title".to_string()),
            if state.url.trim().is_empty() {
                "Unknown URL".to_string()
            } else {
                state.url
            },
        ),
        Err(_) => ("Unknown Title".to_string(), "Unknown URL".to_string()),
    };
    let summary = format!(
        "Status: Success\nAction: {}\n\n--- Page State ---\nTitle: {}\nURL: {}\n",
        action_result, title, url
    );
    let hint = SuccessHint::new(
        summary,
        vec![
            "Use `browser__getPageContent({})` to read the full page content.".to_string(),
            "Use `browser__clickElement` to interact with elements.".to_string(),
            "Use `inputText` to type into forms.".to_string(),
        ],
    );
    Ok(hint.to_mcp_result())
}

/// Suggest alternative selectors when element is not found
async fn suggest_selectors(
    service: &crate::services::InteractiveBrowserServer,
    session_id: &str,
) -> String {
    let script = r#"(function() {
        const candidates = Array.from(document.querySelectorAll('button, input, a[href], [role="button"]'));
        function isVisible(el) {
            const style = window.getComputedStyle(el);
            return style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0';
        }
        return candidates.filter(isVisible).slice(0, 5).map(el => {
            let id = el.id ? '#' + el.id : '';
            let cls = el.classList.length > 0 ? '.' + el.classList[0] : '';
            let text = (el.innerText || el.value || '').substring(0, 20).replace(/\s+/g, ' ').trim();
            return `${el.tagName.toLowerCase()}${id}${cls} (text: "${text}")`;
        });
    })()"#;
    match service.execute_script(session_id, script).await {
        Ok(json_str) => match serde_json::from_str::<Vec<String>>(&json_str) {
            Ok(suggestions) if !suggestions.is_empty() => {
                format!(
                    "\nDid you mean one of these?\n- {}",
                    suggestions.join("\n- ")
                )
            }
            _ => String::new(),
        },
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_script_builds_name_aware_unique_selectors() {
        let script = get_filter_script("semantic_input", "viewport");
        assert!(
            script.contains("getAttribute('name')"),
            "selector builder must read name attributes"
        );
        assert!(
            script.contains("[name=\""),
            "selector builder must emit name-based CSS"
        );
        assert!(
            script.contains("nth-of-type"),
            "selector builder must fall back to nth-of-type"
        );
        assert!(
            script.contains("name: el.getAttribute('name')"),
            "listed attributes must include name for agent readability"
        );
        assert!(
            script.contains("let fallback"),
            "non-unique name must continue the chain via fallback, not early-return"
        );
        assert!(
            script.contains("closest('[id]')"),
            "nth-of-type must try an id-bearing ancestor before unscoped relative"
        );
        assert!(
            script.contains("if (isUnique(relative)) return relative"),
            "unscoped nth-of-type must only be emitted when unique"
        );
        for line in script.lines() {
            let trimmed = line.trim();
            if trimmed == "return byName;" || trimmed == "return byNameValue;" {
                panic!("unconditional mid-chain return of non-unique name selector: {trimmed}");
            }
            if trimmed.contains("return byName;") {
                assert!(
                    trimmed.contains("isUnique(byName)"),
                    "return byName must be gated by isUnique: {trimmed}"
                );
            }
            if trimmed.contains("return byNameValue;") {
                assert!(
                    trimmed.contains("isUnique(byNameValue)"),
                    "return byNameValue must be gated by isUnique: {trimmed}"
                );
            }
        }
    }

    #[test]
    fn format_interactive_elements_preserves_name_selectors() {
        let json = r#"[
            {"index":0,"tag":"input","text":"","attributes":{"name":"custname","type":"text"},"selector":"input[name=\"custname\"]"},
            {"index":1,"tag":"input","text":"","attributes":{"name":"size","type":"radio","value":"small"},"selector":"input[name=\"size\"][value=\"small\"]"}
        ]"#;
        let formatted =
            format_interactive_elements(json, "semantic_input", "viewport").expect("format");
        assert!(formatted.contains("Selector: input[name=\"custname\"]"));
        assert!(formatted.contains("Selector: input[name=\"size\"][value=\"small\"]"));
        assert!(formatted.contains("name=\"custname\""));
    }

    #[test]
    fn classify_dom_action_result_requires_exact_tokens() {
        assert!(matches!(
            classify_dom_action_result("Clicked element", "Clicked element"),
            DomActionOutcome::Success
        ));
        assert!(matches!(
            classify_dom_action_result("\"Input successful\"", "Input successful"),
            DomActionOutcome::Success
        ));
        assert!(matches!(
            classify_dom_action_result("Element not found", "Clicked element"),
            DomActionOutcome::NotFound
        ));
        assert!(matches!(
            classify_dom_action_result("Element not visible", "Input successful"),
            DomActionOutcome::NotVisible
        ));
        // Substring / echoed phrases must not count as success.
        assert!(matches!(
            classify_dom_action_result("Failed while Clicked element", "Clicked element"),
            DomActionOutcome::Other(_)
        ));
        assert!(matches!(
            classify_dom_action_result("null", "Input successful"),
            DomActionOutcome::Other(_)
        ));
        assert!(matches!(
            classify_dom_action_result("", "Clicked element"),
            DomActionOutcome::Other(_)
        ));
    }

    #[test]
    fn format_interactive_elements_null_explains_csp_limit() {
        let formatted =
            format_interactive_elements("null", "semantic_clickable", "viewport").expect("format");
        assert!(formatted.contains("Interactable discovery unavailable"));
        assert!(formatted.contains("userChrome"));
        assert!(!formatted.contains("No semantic clickable elements found"));
    }
}
