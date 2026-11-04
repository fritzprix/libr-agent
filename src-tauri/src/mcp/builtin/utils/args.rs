//! Read tool arguments that models sometimes send with the wrong JSON type.

use serde_json::Value;

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::types::{MCPContent, MCPResult};

/// A boolean tool argument. `warning` is set when the model sent the value as
/// a string ("true"/"false") instead of a JSON boolean.
#[derive(Debug, PartialEq, Eq)]
pub struct BoolArg {
    pub value: bool,
    pub warning: Option<String>,
}

impl BoolArg {
    /// Append the string-boolean warning, if any, to a tool result.
    pub fn annotate(&self, mut result: MCPResult) -> MCPResult {
        if let Some(warning) = &self.warning {
            result
                .content
                .get_or_insert_with(Vec::new)
                .push(MCPContent::Text {
                    text: warning.clone(),
                });
        }
        result
    }
}

/// Read boolean argument `name`. A missing or null value gives `default`.
/// The strings "true"/"false" (trimmed, any case) are accepted with a warning;
/// any other value is an invalid-input error that names the argument.
pub fn read_bool_arg(
    args: &Value,
    name: &str,
    default: bool,
    tool_group: ToolGroup,
) -> Result<BoolArg, MCPResult> {
    let parsed = match args.get(name) {
        None | Some(Value::Null) => Some((default, None)),
        Some(Value::Bool(value)) => Some((*value, None)),
        Some(Value::String(text)) => {
            let value = match text.trim().to_ascii_lowercase().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            };
            value.map(|value| {
                let warning = format!(
                    "Warning: {name} was the string \"{text}\"; pass a JSON boolean (true/false)."
                );
                (value, Some(warning))
            })
        }
        Some(_) => None,
    };

    parsed
        .map(|(value, warning)| BoolArg { value, warning })
        .ok_or_else(|| {
            guided_error(
                ErrorCategory::InvalidInput,
                format!("{name} must be a JSON boolean (true or false)"),
                tool_group,
            )
            .guidance(vec![format!("Pass \"{name}\": true or \"{name}\": false.")])
            .to_mcp_result()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read(args: Value) -> Result<BoolArg, MCPResult> {
        read_bool_arg(&args, "flag", false, ToolGroup::Workspace)
    }

    fn result_text(result: &MCPResult) -> String {
        serde_json::to_string(result).expect("result serializes")
    }

    #[test]
    fn json_booleans_are_used_without_a_warning() {
        assert_eq!(
            read(json!({ "flag": true })).expect("valid"),
            BoolArg {
                value: true,
                warning: None
            }
        );
        assert_eq!(
            read(json!({ "flag": false })).expect("valid"),
            BoolArg {
                value: false,
                warning: None
            }
        );
    }

    #[test]
    fn a_missing_value_uses_the_default() {
        assert!(!read(json!({})).expect("valid").value);
        let with_default =
            read_bool_arg(&json!({}), "flag", true, ToolGroup::Workspace).expect("valid");
        assert_eq!(
            with_default,
            BoolArg {
                value: true,
                warning: None
            }
        );
    }

    #[test]
    fn string_booleans_are_accepted_with_a_warning_in_the_result() {
        let arg = read(json!({ "flag": "true" })).expect("valid");
        assert!(arg.value);
        let result = arg.annotate(MCPResult::success("ran"));
        let text = result_text(&result);
        assert!(text.contains("ran"), "{text}");
        assert!(
            text.contains(
                r#"Warning: flag was the string \"true\"; pass a JSON boolean (true/false)."#
            ),
            "{text}"
        );

        let padded = read(json!({ "flag": "TRUE " })).expect("valid");
        assert!(padded.value);
        assert!(padded.warning.is_some());
    }

    #[test]
    fn other_values_are_errors_that_name_the_argument() {
        for value in [json!("yes"), json!(1)] {
            let error = read(json!({ "flag": value })).expect_err("invalid");
            assert_eq!(error.is_error, Some(true));
            assert!(result_text(&error).contains("flag must be a JSON boolean"));
        }
    }

    #[test]
    fn annotate_without_a_warning_keeps_the_result() {
        let arg = read(json!({ "flag": true })).expect("valid");
        let result = arg.annotate(MCPResult::success("ran"));
        assert_eq!(result.content.map(|content| content.len()), Some(1));
    }
}
