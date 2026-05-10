use tauri_mcp_agent_lib::mcp::presets::get_recommended_servers;

#[test]
fn all_embedded_presets_declare_a_category() {
    let presets = get_recommended_servers();

    assert!(!presets.is_empty(), "expected embedded presets to exist");
    assert!(
        presets
            .iter()
            .all(|preset| preset.category.as_deref().is_some()),
        "every preset should declare a category"
    );
}

#[test]
fn category_assignments_match_expected_presets() {
    let presets = get_recommended_servers();

    let github = presets
        .iter()
        .find(|preset| preset.name == "github")
        .expect("github preset should exist");
    assert_eq!(github.category.as_deref(), Some("devtools"));

    let openai = presets
        .iter()
        .find(|preset| preset.name == "openai")
        .expect("openai preset should exist");
    assert_eq!(openai.category.as_deref(), Some("ai"));

    let yahoo_finance = presets
        .iter()
        .find(|preset| preset.name == "yahoo-finance")
        .expect("yahoo-finance preset should exist");
    assert_eq!(yahoo_finance.category.as_deref(), Some("data"));
}
