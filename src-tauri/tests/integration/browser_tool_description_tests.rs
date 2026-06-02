use std::collections::HashMap;
use tauri_mcp_agent_lib::mcp::builtin::browser::BrowserServer;
use tauri_mcp_agent_lib::mcp::schema::{JSONSchema, JSONSchemaType};

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

fn object_properties<'a>(schema: &'a JSONSchema, context: &str) -> &'a HashMap<String, JSONSchema> {
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
            "use `getPageContent({})` or listInteractable before another `navigateToUrl`"
        ),
        "navigateToUrl should require a read step before another navigation"
    );
}

#[test]
fn create_session_description_explains_single_stateful_session() {
    let description = browser_tool_description("createSession");

    assert!(
        description.contains("One agent has one active browser session/page"),
        "createSession should explain the single-session model"
    );
}

#[test]
fn get_page_content_description_marks_read_after_navigation_workflow() {
    let description = browser_tool_description("getPageContent");

    assert!(
        description.contains("This is the normal next step after `navigateToUrl`"),
        "getPageContent should be described as the immediate follow-up to navigation"
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
fn list_interactable_description_explains_selector_discovery_role() {
    let description = browser_tool_description("listInteractable");

    assert!(
        description.contains("before `clickElement` or `inputText`"),
        "listInteractable should explain that it is the selector discovery step"
    );
    assert!(
        description.contains("instead of guessing"),
        "listInteractable should explicitly discourage guessed selectors"
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
        description.contains("starting over with `createSession`"),
        "closeSession should point agents toward the recovery path"
    );
}

#[test]
fn fetch_description_marks_stateless_alternative() {
    let description = browser_tool_description("fetch");

    assert!(
        description.contains("Stateless one-off fetch"),
        "fetch should be framed as the stateless alternative"
    );
    assert!(
        description.contains("instead of chaining multiple `navigateToUrl` calls"),
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
