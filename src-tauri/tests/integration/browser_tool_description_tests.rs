use tauri_mcp_agent_lib::mcp::builtin::browser::BrowserServer;
use tauri_mcp_agent_lib::mcp::schema::{JSONSchema, JSONSchemaType, SchemaProperties};
use tauri_mcp_agent_lib::mcp::types::MCPContent;

fn browser_tool_description(tool_name: &str) -> String {
    BrowserServer::tools_static()
        .into_iter()
        .find(|tool| tool.name == tool_name)
        .unwrap_or_else(|| panic!("browser tool not found: {tool_name}"))
        .description
}

fn browser_tool(tool_name: &str) -> tauri_mcp_agent_lib::mcp::MCPTool {
    BrowserServer::tools_static()
        .into_iter()
        .find(|tool| tool.name == tool_name)
        .unwrap_or_else(|| panic!("browser tool not found: {tool_name}"))
}

fn object_properties<'a>(schema: &'a JSONSchema, context: &str) -> &'a SchemaProperties {
    match &schema.schema_type {
        JSONSchemaType::Object {
            properties: Some(properties),
            ..
        } => properties,
        other => panic!("{context}: expected object schema, got {other:?}"),
    }
}

#[test]
fn navigate_to_url_description_warns_about_stateful_overwrites() {
    let description = browser_tool_description("navigateToUrl");

    assert!(
        description.contains("single active browser session"),
        "navigateToUrl should explain that browser navigation is stateful"
    );
    assert!(
        description.contains("overwrite"),
        "navigateToUrl should warn that repeated navigation overwrites prior page state"
    );
    assert!(
        description.contains(
            "browser__getPageContent({}) or browser__listInteractable before another browser__navigateToUrl"
        ),
        "navigateToUrl should require a read step before another navigation"
    );
}

#[test]
fn create_session_description_explains_single_stateful_session() {
    let description = browser_tool_description("createSession");

    assert!(
        description.contains("one active browser session")
            || description.contains("single active browser session"),
        "createSession should explain the single-session model"
    );
    assert!(
        description.contains("Do not keep userChrome and sidecar open at the same time")
            || description.contains("no dual-open")
            || description.contains("Do not assume two browser sessions"),
        "createSession must forbid parallel userChrome+sidecar sessions"
    );
    assert!(
        description.contains("closes it and starts a fresh one"),
        "createSession must state that a new create replaces the previous session"
    );
    assert!(
        description.contains("sticky agent"),
        "createSession should document sticky agent login persistence"
    );
    assert!(
        description.contains("shares one cookie jar")
            || description.contains("Shares one cookie jar"),
        "createSession should document shared sticky cookie jar across sessions"
    );
    assert!(
        description.contains("userChrome"),
        "createSession should document everyday Chrome via userChrome"
    );
    assert!(
        description.contains("Explicit choice")
            || description.contains("no silent fallback")
            || description.contains("does NOT open sidecar"),
        "createSession must forbid silent sidecar fallback from userChrome"
    );
    assert!(
        !description.to_lowercase().contains("use_profile"),
        "createSession must not mention removed use_profile"
    );
    assert!(
        !description.to_lowercase().contains("saved-login")
            && !description.to_lowercase().contains("saved login"),
        "createSession must not mention saved-login import path"
    );
}

#[test]
fn create_session_schema_exposes_browser_target_not_use_profile() {
    let tool = browser_tool("createSession");
    let properties = object_properties(&tool.input_schema, "createSession");
    assert!(
        !properties.contains_key("use_profile"),
        "createSession schema must not expose use_profile"
    );
    assert!(
        properties.contains_key("browser"),
        "createSession schema must expose browser=sidecar|userChrome"
    );
    let browser = properties.get("browser").expect("browser property missing");
    let values = browser
        .enum_values
        .as_ref()
        .expect("browser enum_values missing");
    let as_str: Vec<&str> = values.iter().filter_map(|v| v.as_str()).collect();
    assert!(as_str.contains(&"sidecar"));
    assert!(as_str.contains(&"userChrome"));
}

#[test]
fn get_console_logs_description_marks_sidecar_only() {
    let description = browser_tool_description("getConsoleLogs");
    assert!(
        description.contains("sidecar") && description.contains("userChrome"),
        "getConsoleLogs must document sidecar-only vs userChrome unsupported"
    );
    assert!(
        description.to_lowercase().contains("not available")
            || description.contains("unsupported")
            || description.contains("Not available"),
        "getConsoleLogs must state userChrome is unavailable"
    );
}

#[test]
fn evaluate_js_description_notes_console_logs_sidecar_only() {
    let description = browser_tool_description("evaluateJS");
    assert!(
        description.contains("userChrome"),
        "evaluateJS should mention userChrome support"
    );
    assert!(
        description.contains("getConsoleLogs") && description.contains("sidecar"),
        "evaluateJS should steer console logs to sidecar-only"
    );
}

#[test]
fn evaluate_js_description_requires_expression_or_iife() {
    let description = browser_tool_description("evaluateJS");
    assert!(
        description.contains("IIFE") || description.contains("final expression"),
        "evaluateJS must steer agents away from top-level return"
    );
    assert!(
        description.to_lowercase().contains("top-level return") || description.contains("null"),
        "evaluateJS must warn that top-level return often yields null"
    );
}

#[test]
fn get_page_content_description_marks_read_after_navigation_workflow() {
    let description = browser_tool_description("getPageContent");

    assert!(
        description.contains("This is the normal next step after `browser__navigateToUrl`"),
        "getPageContent should be described as the immediate follow-up to navigation"
    );
    assert!(
        description.contains("If the cache is empty, the first call extracts automatically"),
        "getPageContent should document empty-cache extract fallback for page:1"
    );
}

#[test]
fn get_page_content_page_schema_requires_positive_integer() {
    let tool = browser_tool("getPageContent");
    let properties = object_properties(&tool.input_schema, "getPageContent");
    let page_schema = properties
        .get("page")
        .expect("getPageContent should expose page");

    match &page_schema.schema_type {
        JSONSchemaType::Integer {
            minimum, maximum, ..
        } => {
            assert_eq!(minimum, &Some(1));
            assert_eq!(maximum, &None);
        }
        other => panic!("expected integer page schema, got {other:?}"),
    }
}

#[test]
fn take_screenshot_schema_exposes_optional_full_page_flag() {
    let tool = browser_tool("takeScreenshot");
    let properties = object_properties(&tool.input_schema, "takeScreenshot");
    let full_page = properties
        .get("fullPage")
        .expect("takeScreenshot should expose fullPage");

    assert!(
        matches!(&full_page.schema_type, JSONSchemaType::Boolean),
        "fullPage should be a boolean"
    );
    if let JSONSchemaType::Object { required, .. } = &tool.input_schema.schema_type {
        assert!(
            !required
                .as_ref()
                .is_some_and(|required| required.iter().any(|name| name == "fullPage")),
            "fullPage should be optional"
        );
    }
    assert!(
        tool.description.contains("PNG image"),
        "takeScreenshot should describe its image output"
    );
}

#[test]
fn screenshot_image_content_serializes_as_standard_mcp_image() {
    let content = MCPContent::Image {
        data: Some("cG5n".to_string()),
        uri: None,
        mime_type: "image/png".to_string(),
    };
    let serialized = serde_json::to_value(content).expect("image content should serialize");

    assert_eq!(serialized["type"], "image");
    assert_eq!(serialized["data"], "cG5n");
    assert_eq!(serialized["mimeType"], "image/png");
}

#[test]
fn list_interactable_description_explains_selector_discovery_role() {
    let description = browser_tool_description("listInteractable");

    assert!(
        description.contains("before `browser__clickElement` or `browser__inputText`"),
        "listInteractable should explain that it is the selector discovery step"
    );
    assert!(
        description.contains("instead of guessing"),
        "listInteractable should explicitly discourage guessed selectors"
    );
    assert!(
        description.contains("[name=") || description.contains("name="),
        "listInteractable should document name-aware selectors"
    );
}

#[test]
fn close_session_description_mentions_state_reset() {
    let description = browser_tool_description("closeSession");

    assert!(
        description.contains("clear the stored session state"),
        "closeSession should explain that it resets stored browser session state"
    );
    assert!(
        description.contains("starting over with `browser__createSession`"),
        "closeSession should point agents toward the recovery path"
    );
}

#[test]
fn fetch_description_marks_stateless_alternative() {
    let description = browser_tool_description("fetchUrl");

    assert!(
        description.contains("Stateless one-off fetch"),
        "fetch should be framed as the stateless alternative"
    );
    assert!(
        description.contains("instead of chaining multiple `browser__navigateToUrl` calls"),
        "fetch should explicitly discourage repeated navigation for independent lookups"
    );
    assert!(
        description.contains("does not create or reuse the visible stateful browser workflow"),
        "fetch should distinguish itself from the stateful browser session workflow"
    );
}

#[test]
fn browser_public_surface_exposes_explicit_libragent_names() {
    let tool_names: Vec<String> = BrowserServer::tools_static()
        .into_iter()
        .map(|tool| tool.name)
        .collect();

    assert!(
        tool_names.contains(&"navigateToUrl".to_string()),
        "browser public surface should expose navigateToUrl"
    );
    assert!(
        tool_names.contains(&"getPageContent".to_string()),
        "browser public surface should expose getPageContent"
    );
    assert!(
        tool_names.contains(&"takeScreenshot".to_string()),
        "browser public surface should expose takeScreenshot"
    );
    assert!(
        !tool_names.contains(&"goto".to_string()),
        "browser public surface should not expose goto"
    );
    assert!(
        !tool_names.contains(&"content".to_string()),
        "browser public surface should not expose content"
    );
    assert!(
        !tool_names.contains(&"extractWebContent".to_string()),
        "browser public surface should not expose extractWebContent"
    );
    assert!(
        !tool_names.contains(&"readWebContent".to_string()),
        "browser public surface should not expose readWebContent"
    );
    assert!(
        !tool_names.contains(&"click".to_string()),
        "browser public surface should not expose click alias"
    );
    assert!(
        !tool_names.contains(&"fill".to_string()),
        "browser public surface should not expose fill alias"
    );
    assert!(
        !tool_names.contains(&"scroll".to_string()),
        "browser public surface should not expose scroll alias"
    );
    assert!(
        !tool_names.contains(&"back".to_string()),
        "browser public surface should not expose back alias"
    );
    assert!(
        !tool_names.contains(&"forward".to_string()),
        "browser public surface should not expose forward alias"
    );
}
