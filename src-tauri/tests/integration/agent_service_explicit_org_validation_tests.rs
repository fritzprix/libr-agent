use tauri_mcp_agent_lib::services::agent_service::normalize_explicit_org;

#[test]
fn normalize_explicit_org_rejects_partial_values() {
    let result = normalize_explicit_org(
        Some("org-1".to_string()),
        Some("Alpha Org".to_string()),
        None,
    );

    assert_eq!(
        result,
        Err(
            "Explicit org metadata must include orgId, orgName, and orgRootSessionId together"
                .to_string()
        )
    );
}

#[test]
fn normalize_explicit_org_rejects_empty_values() {
    let result = normalize_explicit_org(
        Some("  ".to_string()),
        Some("Alpha Org".to_string()),
        Some("root-1".to_string()),
    );

    assert_eq!(
        result,
        Err(
            "Explicit org metadata must include non-empty orgId, orgName, and orgRootSessionId together"
                .to_string()
        )
    );
}

#[test]
fn normalize_explicit_org_trims_valid_values() {
    let result = normalize_explicit_org(
        Some(" org-1 ".to_string()),
        Some(" Alpha Org ".to_string()),
        Some(" root-1 ".to_string()),
    );

    assert_eq!(
        result,
        Ok(Some((
            "org-1".to_string(),
            "Alpha Org".to_string(),
            "root-1".to_string(),
        )))
    );
}
