use crate::mcp::builtin::tool_description::tool_description;
use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Create a new browser session
pub fn create_session_tool() -> MCPTool {
    MCPTool {
        name: "createSession".to_string(),
        title: Some("Create Browser Session".to_string()),
        description: tool_description(
            "Create or replace the active browser session for this agent. One agent has one active browser session/page at a time.",
            &[],
            &[
                "Call createSession before other browser tools if no active session exists.",
                "If a session already exists, createSession closes it and starts a fresh one.",
                "If url is omitted, the session opens https://www.google.com.",
            ],
            &[
                "Navigate with browser__navigateToUrl.",
                "Read page content with browser__getPageContent.",
            ],
        )
        .to_string(),
        input_schema: object_prop(
            vec![(
                "url".to_string(),
                string_prop(
                    None,
                    None,
                    Some("Initial URL to open in the new active session."),
                ),
            )],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Navigate to a specific URL
pub fn navigate_to_url_tool() -> MCPTool {
    MCPTool {
        name: "navigateToUrl".to_string(),
        title: Some("Navigate to URL".to_string()),
        description: "Navigate the single active browser session to a specific URL.

Behavior:
- Requires an active session created by `createSession`
- There is only one active browser session per agent — navigateToUrl will overwrite the current page
- Replaces the current page and invalidates previously extracted page content
- Returns navigation status plus the live page title and URL, not full page content

⚠️ Error Handling:
- 403/401: Page blocks automated access - abandon and search elsewhere
- 404: Page not found - check URL or search homepage
- Timeout: Page too complex or blocking - try different URL

💡 Suggested follow-ups:
- browser__getPageContent({}) or browser__listInteractable before another browser__navigateToUrl
- browser__listInteractable to inspect actionable elements"
            .to_string(),
        input_schema: object_prop(
            vec![(
                "url".to_string(),
                string_prop_required(
                    "URL to navigate to (must start with http://, https://, or about:)",
                ),
            )],
            vec!["url".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Navigate back in browser history
pub fn navigate_back_tool() -> MCPTool {
    MCPTool {
        name: "navigateBack".to_string(),
        title: Some("Navigate Back".to_string()),
        description: tool_description(
            "Navigate back in browser history to the previous page.",
            &["Active browser session from browser__createSession."],
            &["Requires prior navigation history in the active session."],
            &[
                "Extract content with browser__getPageContent.",
                "Inspect elements with browser__listInteractable.",
            ],
        ),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

/// Navigate forward in browser history
pub fn navigate_forward_tool() -> MCPTool {
    MCPTool {
        name: "navigateForward".to_string(),
        title: Some("Navigate Forward".to_string()),
        description: tool_description(
            "Navigate forward in browser history to the next page.",
            &["Active browser session from browser__createSession."],
            &["Requires having navigated back previously."],
            &[
                "Extract content with browser__getPageContent.",
                "Inspect elements with browser__listInteractable.",
            ],
        ),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

/// Get the current URL
pub fn get_current_url_tool() -> MCPTool {
    MCPTool {
        name: "getCurrentUrl".to_string(),
        title: Some("Get Current URL".to_string()),
        description: tool_description(
            "Get the current URL of the active browser page.",
            &["Active browser session from browser__createSession."],
            &[],
            &[
                "Extract page content with browser__getPageContent.",
                "Navigate elsewhere with browser__navigateToUrl.",
            ],
        ),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

/// Get the page title
pub fn get_page_title_tool() -> MCPTool {
    MCPTool {
        name: "getPageTitle".to_string(),
        title: Some("Get Page Title".to_string()),
        description: tool_description(
            "Get the title of the current active browser page.",
            &["Active browser session from browser__createSession."],
            &[],
            &[
                "Read page content with browser__getPageContent.",
                "Verify navigation succeeded after browser__navigateToUrl.",
            ],
        ),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

/// Get page content — fresh extraction or cached page read
pub fn get_page_content_tool() -> MCPTool {
    MCPTool {
        name: "getPageContent".to_string(),
        title: Some("Get Page Content".to_string()),
        description: "Get content from the active browser session page as markdown.

This is the normal next step after `navigateToUrl`.
- No `page` arg: extract fresh content from the current page.
- With `page`: read a specific page number from the most recently extracted cache.

Pagination is cache-based, not scroll-based.
If the response says `[Page 1/N]`, continue with `getPageContent({ \"page\": 2 })`.

⚠️ Navigation (navigateToUrl, navigateBack, navigateForward) clears the content cache.
Call `getPageContent({})` again after any navigation."
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "page".to_string(),
                    integer_prop(
                        Some(1),
                        None,
                        Some("Page number to read from cache (minimum 1). Omit to extract fresh content."),
                    ),
                ),
                (
                    "autoMerge".to_string(),
                    boolean_prop(Some("Whether to attempt merging all pages into one response.")),
                ),
                (
                    "saveRawHtml".to_string(),
                    boolean_prop(Some("Whether to save raw HTML to a file for debugging.")),
                ),
            ],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Click an element on the page
pub fn click_element_tool() -> MCPTool {
    MCPTool {
        name: "clickElement".to_string(),
        title: Some("Click Element".to_string()),
        description: tool_description(
            "Click an element in the active browser session using a CSS selector.",
            &["Active browser session from browser__createSession."],
            &[
                "Call browser__listInteractable or browser__getPageContent before this tool to extract a real selector from the current page.",
                "Pass the selector exactly as extracted. Do not guess selectors.",
            ],
            &[
                "Use browser__getPageContent to verify the page state after the click.",
                "Use browser__listInteractable again if the page revealed new elements.",
            ],
        ),
        input_schema: object_prop(
            vec![(
                "selector".to_string(),
                string_prop_required("CSS selector of the element to click (must match an element visible in listInteractable or getPageContent)"),
            )],
            vec!["selector".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Input text into an element
pub fn input_text_tool() -> MCPTool {
    MCPTool {
        name: "inputText".to_string(),
        title: Some("Input Text".to_string()),
        description: tool_description(
            "Enter text into an input element in the active browser session.",
            &["Active browser session from browser__createSession."],
            &[
                "Call browser__listInteractable or browser__getPageContent before this tool to extract a valid selector.",
                "Use a selector that targets an input or textarea element.",
            ],
            &[
                "Use browser__getPageContent to confirm the form state if the page reflects the input.",
                "Use browser__clickElement if the next step is submitting or revealing related controls.",
            ],
        ),
        input_schema: object_prop(
            vec![
                (
                    "selector".to_string(),
                    string_prop_required("CSS selector of the input element"),
                ),
                ("text".to_string(), string_prop_required("Text to input")),
            ],
            vec!["selector".to_string(), "text".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Scroll the page to a specific position
pub fn scroll_page_tool() -> MCPTool {
    MCPTool {
        name: "scrollPage".to_string(),
        title: Some("Scroll Page".to_string()),
        description: tool_description(
            "Scroll the active browser page to a specific position.",
            &["Active browser session from browser__createSession."],
            &[
                "Use this to reveal off-screen or lazy-loaded elements in the live page.",
                "Do not use scrolling to advance cached browser__getPageContent pages. When content extraction returns [Page 1/N], read the next cached page with browser__getPageContent instead.",
            ],
            &[
                "Use browser__listInteractable after scrolling to inspect newly visible elements.",
                "Use browser__getPageContent after scrolling if the page loaded more text content.",
            ],
        ),
        input_schema: object_prop(
            vec![
                (
                    "x".to_string(),
                    number_prop(None, None, Some("X coordinate to scroll to")),
                ),
                (
                    "y".to_string(),
                    number_prop(None, None, Some("Y coordinate to scroll to")),
                ),
            ],
            vec!["x".to_string(), "y".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// List interactable elements on the page
pub fn list_interactable_tool() -> MCPTool {
    MCPTool {
        name: "listInteractable".to_string(),
        title: Some("List Interactable Elements".to_string()),
        description: "List interactable elements on the page.

Use this before `clickElement` or `inputText` to discover valid CSS selectors instead of guessing.
Prefer this over getPageContent when you only need to find elements for interaction."
            .to_string(),
        input_schema: object_prop(
            vec![
                (
                    "filterType".to_string(),
                    enum_prop(
                        vec!["semantic_clickable", "semantic_input", "all_focusable"],
                        "semantic_clickable",
                        Some("Filter type:\n- semantic_clickable: Buttons, links, and clickable elements\n- semantic_input: Inputs, textareas, and form fields\n- all_focusable: Everything that can receive focus"),
                    ),
                ),
                (
                    "scope".to_string(),
                    enum_prop(
                        vec!["viewport", "all"],
                        "viewport",
                        Some("Scope of listing."),
                    ),
                ),
            ],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Close the browser session
pub fn close_session_tool() -> MCPTool {
    MCPTool {
        name: "closeSession".to_string(),
        title: Some("Close Browser Session".to_string()),
        description: "Explicitly close the browser session and clear the stored session state.

Good practice after finishing a task to free resources.
starting over with `createSession` after closing is the recommended recovery path if the session enters a broken state."
            .to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

/// Fetch URL content directly (headless browser or file download)
pub fn fetch_tool() -> MCPTool {
    let mut props = crate::mcp::schema::SchemaProperties::new();

    props.insert(
        "url".to_string(),
        string_prop_required("URL to fetch (must start with http:// or https://)"),
    );

    props.insert(
        "savePath".to_string(),
        string_prop(
            None,
            None,
            Some("Relative path to save the file to if it's not a web page (e.g., 'downloads/document.pdf')")
        ),
    );

    MCPTool {
        name: "fetchUrl".to_string(),
        title: Some("Fetch URL".to_string()),
        description: tool_description(
            "Stateless one-off fetch: fetch a single URL without affecting the active browser session.",
            &[],
            &[
                "Use this instead of chaining multiple `navigateToUrl` calls when you only need the content of a single, independent URL.",
                "This does not create or reuse the visible stateful browser workflow.",
                "HTML or text responses are returned as markdown in the tool result.",
                "Non-HTML responses require savePath so the file can be downloaded into the workspace.",
            ],
            &[
                "Process the returned markdown directly when you only need page content.",
                "Use workspace tools on the saved file when savePath downloaded a non-HTML resource.",
            ],
        ),
        input_schema: object_schema(props, vec!["url".to_string()]),
        output_schema: None,
        annotations: None,
    }
}

/// Returns all browser tools (canonical LibrAgent names, no Playwright/short aliases)
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        create_session_tool(),
        close_session_tool(),
        // Navigation
        navigate_to_url_tool(),
        navigate_back_tool(),
        navigate_forward_tool(),
        get_current_url_tool(),
        get_page_title_tool(),
        // Interaction
        click_element_tool(),
        input_text_tool(),
        scroll_page_tool(),
        // Content
        get_page_content_tool(),
        fetch_tool(),
        // Discovery
        list_interactable_tool(),
        evaluate_js_tool(),
        get_console_logs_tool(),
    ]
}

/// Execute JavaScript code in the active browser session
pub fn evaluate_js_tool() -> MCPTool {
    MCPTool {
        name: "evaluateJS".to_string(),
        title: Some("Evaluate JavaScript".to_string()),
        description: tool_description(
            "Execute JavaScript in the active browser session and return the serialized result.",
            &["Active browser session from browser__createSession."],
            &[
                "Use this for page inspection, debugging, or controlled DOM manipulation in the current page.",
                "Return plain values when possible. For complex objects, serialize them in the script with JSON.stringify(...).",
            ],
            &[
                "Use browser__getConsoleLogs to inspect page-side errors after script execution.",
                "Use browser__getPageContent to verify page state after DOM changes.",
            ],
        ),
        input_schema: object_prop(
            vec![(
                "script".to_string(),
                string_prop_required("JavaScript code to execute"),
            )],
            vec!["script".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Get browser console logs
pub fn get_console_logs_tool() -> MCPTool {
    MCPTool {
        name: "getConsoleLogs".to_string(),
        title: Some("Get Console Logs".to_string()),
        description: tool_description(
            "Read recent browser console output from the active browser session.",
            &["Active browser session from browser__createSession."],
            &[
                "Use this after navigation, form submission, or browser__evaluateJS when you need runtime logs from the page.",
                "Adjust maxEntries when you need a broader or narrower log window.",
            ],
            &[
                "Use browser__evaluateJS to inspect page state related to the logged messages.",
                "Use browser__getPageContent when you need the rendered page context alongside the logs.",
            ],
        ),
        input_schema: object_prop(
            vec![(
                "maxEntries".to_string(),
                integer_prop(
                    Some(100),
                    Some(1000),
                    Some("Maximum number of log entries to return (default 100, max 1000)"),
                ),
            )],
            vec![],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}
