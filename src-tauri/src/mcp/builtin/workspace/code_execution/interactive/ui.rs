/// Build UIResource DTO for shell input form
/// Returns JSON string with execution_id, prompt, and input type
pub fn build_shell_input_ui(
    execution_id: &str,
    prompt: &str,
    input_type: &str,
    nonce: &str,
) -> String {
    let safe_input_type = if input_type.eq_ignore_ascii_case("password") {
        "password"
    } else {
        "text"
    };

    let dto = serde_json::json!({
        "type": "shell_input",
        "execution_id": execution_id,
        "prompt": prompt,
        "input_type": safe_input_type,
        "nonce": nonce
    });

    serde_json::to_string(&dto).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::build_shell_input_ui;
    use serde_json::Value;

    #[test]
    fn test_build_shell_input_ui_whitelists_input_type() {
        let json_str = build_shell_input_ui("exec-1", "Prompt", "password", "nonce-1");
        let dto: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(
            dto["input_type"],
            "password",
            "password should be preserved as allowed input type"
        );

        let json_with_invalid_type = build_shell_input_ui(
            "exec-2",
            "Prompt",
            r#"text" autofocus onfocus="alert(1)""#,
            "nonce-2",
        );
        let invalid_dto: Value = serde_json::from_str(&json_with_invalid_type).unwrap();
        assert_eq!(
            invalid_dto["input_type"],
            "text",
            "invalid input_type should be downgraded to text"
        );
        assert!(
            !json_with_invalid_type.contains("autofocus onfocus"),
            "injected attributes must not appear in generated JSON"
        );
    }

    #[test]
    fn test_build_shell_input_ui_uses_static_placeholder() {
        let json_str = build_shell_input_ui("exec-3", "Prompt", "password", "nonce-3");
        let dto: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(dto["type"], "shell_input");
        assert_eq!(dto["execution_id"], "exec-3");
        assert_eq!(dto["prompt"], "Prompt");
        assert_eq!(dto["nonce"], "nonce-3");
    }
}
