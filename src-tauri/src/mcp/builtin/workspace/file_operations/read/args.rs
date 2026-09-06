//! Argument parsing for workspace__readFile.

use crate::mcp::builtin::error_guidance::{guided_error, ErrorCategory, ToolGroup};
use crate::mcp::builtin::workspace::edit_mode::LINE_ANCHORS_ENABLED;
use crate::mcp::types::MCPResult;
use serde_json::Value;

pub(super) fn parse_show_line_anchors(args: &Value) -> bool {
    if !LINE_ANCHORS_ENABLED {
        return false;
    }

    args.get("showLineAnchors")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub(super) fn parse_offset_parameter(args: &Value) -> Result<Option<isize>, MCPResult> {
    let Some(value) = args.get("offset") else {
        return Ok(None);
    };

    match value {
        Value::Null => Ok(None),
        Value::Number(number) => match number.as_i64() {
            Some(off) => Ok(Some(off as isize)),
            None => Err(guided_error(
                ErrorCategory::InvalidInput,
                "offset must be an integer".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use an integer like {\"offset\": 10} or {\"offset\": -100}".to_string(),
            ])
            .to_mcp_result()),
        },
        _ => Err(guided_error(
            ErrorCategory::InvalidInput,
            "offset must be an integer",
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Use an integer like {\"offset\": 10} or {\"offset\": -100}".to_string(),
        ])
        .to_mcp_result()),
    }
}

pub(super) fn parse_size_parameter(args: &Value) -> Result<Option<isize>, MCPResult> {
    let Some(value) = args.get("size") else {
        return Ok(None);
    };

    match value {
        Value::Null => Ok(None),
        Value::Number(number) => match number.as_i64() {
            Some(sz) => Ok(Some(sz as isize)),
            None => Err(guided_error(
                ErrorCategory::InvalidInput,
                "size must be an integer".to_string(),
                ToolGroup::Workspace,
            )
            .guidance(vec![
                "Use an integer like {\"size\": 50} or {\"size\": -20}".to_string(),
            ])
            .to_mcp_result()),
        },
        _ => Err(guided_error(
            ErrorCategory::InvalidInput,
            "size must be an integer",
            ToolGroup::Workspace,
        )
        .guidance(vec![
            "Use an integer like {\"size\": 50} or {\"size\": -20}".to_string(),
        ])
        .to_mcp_result()),
    }
}
