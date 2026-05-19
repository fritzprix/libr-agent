use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Create a new browser session
pub fn create_session_tool() -> MCPTool {
    MCPTool {
        name: "createSession".to_string(),
        title: None,
        description: "Create or replace the active browser session for this agent.

One agent has one active browser session/page at a time.

Behavior:
1. Call createSession before browser tools if no active session exists
2. Other browser tools automatically use the active session stored by the backend
3. If a session already exists, createSession closes it and starts a fresh one
4. Session automatically closes if the agent terminates

If `url` is omitted, the session opens https://www.google.com.

Returns a success message confirming the active session is ready."
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
        title: None,
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

Next Steps:
- use `getPageContent({})` or listInteractable before another `navigateToUrl`
- Use listInteractable to inspect actionable elements"
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
        title: None,
        description: "Navigate back in browser history to the previous page.".to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

/// Navigate forward in browser history
pub fn navigate_forward_tool() -> MCPTool {
    MCPTool {
        name: "navigateForward".to_string(),
        title: None,
        description: "Navigate forward in browser history to the next page.".to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

/// Get the current URL
pub fn get_current_url_tool() -> MCPTool {
    MCPTool {
        name: "getCurrentUrl".to_string(),
        title: None,
        description: "Get the current URL of the page.".to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

/// Get the page title
pub fn get_page_title_tool() -> MCPTool {
    MCPTool {
        name: "getPageTitle".to_string(),
        title: None,
        description: "Get the title of the current page.".to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

/// Get page content — fresh extraction or cached page read
pub fn get_page_content_tool() -> MCPTool {
    MCPTool {
        name: "getPageContent".to_string(),
        title: None,
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
        title: None,
        description: "Click an element on the page using a CSS selector.

⚠️ PREREQUISITE:
- Call listInteractable OR `getPageContent` FIRST to find valid selectors on the page.
- Do NOT guess selectors."
            .to_string(),
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
        title: None,
        description: "Input text into an element on the page.

⚠️ PREREQUISITE:
- Call listInteractable OR `getPageContent` FIRST to find valid selectors.
- Verify the element is an input or textarea."
            .to_string(),
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
        title: None,
        description: "Scroll the page to a specific position.

Use this for interaction or lazy-loaded pages. It does not advance cached `getPageContent` pages.
If `getPageContent({})` returned `[Page 1/N]`, use `getPageContent({ \"page\": 2 })` instead of scrolling."
            .to_string(),
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
        title: None,
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
        title: None,
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
    let mut props = std::collections::HashMap::new();

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
        name: "fetch".to_string(),
        title: Some("Fetch Content".to_string()),
        description: "Stateless one-off fetch: extract markdown content from a URL or download a file without creating or reusing the visible stateful browser workflow.

Use this instead of chaining multiple `navigateToUrl` calls when you only need the content of a single, independent URL.
does not create or reuse the visible stateful browser workflow — does not affect the active session.

WORKFLOW:
1. Provide the URL to fetch.
2. If it's a web page, the content will be extracted and returned as markdown.
3. If it's a file (PDF, image, etc.) and savePath is provided, it will be downloaded.

NEXT STEPS:
- Process the returned markdown content.
- If a file was saved, use workspace tools to interact with it."
            .to_string(),
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
    ]
}
