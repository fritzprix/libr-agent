use crate::mcp::types::MCPTool;
use crate::mcp::utils::schema_builder::*;

/// Create a new browser session
pub fn create_session_tool() -> MCPTool {
    MCPTool {
        name: "createSession".to_string(),
        title: None,
        description: "Create a new browser session for this agent.

⚠️ WORKFLOW:
1. Call createSession FIRST before any other browser operations
2. Use the returned session ID for all subsequent browser tools
3. Session automatically closes if agent terminates

Returns: Session ID (e.g., 'abc123...') - use this ID for all other browser tools"
            .to_string(),
        input_schema: object_prop(
            vec![(
                "url".to_string(),
                string_prop(
                    None,
                    None,
                    Some("Initial URL to open (default: about:blank)"),
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
        description: "Navigate to a specific URL in the browser session.

The browser session is managed automatically by the backend. Simply provide the URL and the system will handle the navigation.

⚠️ Error Handling:
- 403/401: Page blocks automated access - abandon and search elsewhere
- 404: Page not found - check URL or search homepage
- Timeout: Page too complex or blocking - try different URL

Next Steps:
- Use extractWebContent to read page content
- Use listInteractable to see clickable elements".to_string(),
        input_schema: object_prop(
            vec![(
                "url".to_string(),
                string_prop_required("URL to navigate to (must start with http:// or https://)"),
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

/// Extract web content as markdown
pub fn extract_web_content_tool() -> MCPTool {
    MCPTool {
        name: "extractWebContent".to_string(),
        title: None,
        description: "Extract the content of the current page as markdown. Large pages are automatically paginated.

For pages > 3000 tokens, content is split into pages. Use readWebContent(page) to read subsequent pages.".to_string(),
        input_schema: object_prop(
            vec![
                (
                    "autoMerge".to_string(),
                    boolean_prop(Some("Whether to attempt merging all pages into one response (default: true).")),
                ),
                (
                    "saveRawHtml".to_string(),
                    boolean_prop(Some("Whether to save raw HTML to a file for debugging (default: false)")),
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
- Call listInteractable OR extractWebContent FIRST to find valid selectors on the page.
- Do NOT guess selectors.".to_string(),
        input_schema: object_prop(
            vec![(
                "selector".to_string(),
                string_prop_required("CSS selector of the element to click (must match an element visible in listInteractable/extractWebContent)"),
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
- Call listInteractable OR extractWebContent FIRST to find valid selectors.
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
        description: "Scroll the page to a specific position.".to_string(),
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
        description: "List interactable elements on the page.".to_string(),
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
                        Some("Scope of listing (default: viewport)"),
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
        description: "Explicitly close the browser session. Good practice after finishing task to free resources.".to_string(),
        input_schema: object_prop(vec![], vec![], None),
        output_schema: None,
        annotations: None,
    }
}

/// Read a specific page from extracted content
pub fn read_web_content_tool() -> MCPTool {
    MCPTool {
        name: "readWebContent".to_string(),
        title: None,
        description: "Read a specific page from previously extracted content (if paginated)."
            .to_string(),
        input_schema: object_prop(
            vec![(
                "page".to_string(),
                number_prop(None, None, Some("Page number to read (1-based index)")),
            )],
            vec!["page".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

/// Returns all browser tools
pub fn all_tools() -> Vec<MCPTool> {
    vec![
        create_session_tool(),
        navigate_to_url_tool(),
        navigate_back_tool(),
        navigate_forward_tool(),
        get_current_url_tool(),
        get_page_title_tool(),
        extract_web_content_tool(),
        click_element_tool(),
        input_text_tool(),
        scroll_page_tool(),
        list_interactable_tool(),
        close_session_tool(),
        read_web_content_tool(),
    ]
}
