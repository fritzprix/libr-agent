use std::path::PathBuf;
use tauri_mcp_agent_lib::services::skill_service::{
    parse_github_repo_url, skill_storage_directory_name,
};

#[test]
fn github_repo_parser_accepts_query_ref_and_path() {
    let spec =
        parse_github_repo_url("https://github.com/example/skills?ref=feature/foo&path=skills/team")
            .unwrap();

    assert_eq!(spec.owner, "example");
    assert_eq!(spec.repo, "skills");
    assert_eq!(spec.branch.as_deref(), Some("feature/foo"));
    assert_eq!(spec.subpath, Some(PathBuf::from("skills/team")));
}

#[test]
fn github_repo_parser_rejects_ambiguous_tree_subpaths() {
    let error =
        parse_github_repo_url("https://github.com/example/skills/tree/feature/foo/skills/team")
            .unwrap_err();

    assert!(error.contains("Ambiguous GitHub tree URL"));
    assert!(error.contains("?ref=<branch>"));
}

#[test]
fn github_repo_parser_accepts_simple_tree_branch_urls() {
    let spec = parse_github_repo_url("https://github.com/example/skills/tree/main").unwrap();

    assert_eq!(spec.branch.as_deref(), Some("main"));
    assert_eq!(spec.subpath, None);
}

#[test]
fn skill_storage_name_is_safe_and_deterministic() {
    let generated = skill_storage_directory_name(r#" Con:<Skill>?*  "#).unwrap();
    let generated_again = skill_storage_directory_name(r#" Con:<Skill>?*  "#).unwrap();

    assert_eq!(generated, generated_again);
    assert!(generated.starts_with("con-skill-"));
    assert!(generated
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'));
    assert!(!generated.ends_with('.'));
    assert!(!generated.ends_with(' '));
}

#[test]
fn skill_storage_name_allows_display_names_with_path_separators() {
    let generated = skill_storage_directory_name("feature/foo").unwrap();

    assert!(generated.starts_with("feature-foo-"));
}
