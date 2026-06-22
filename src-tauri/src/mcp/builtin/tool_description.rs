//! Builds consistent agent-facing MCP tool descriptions.

/// Format a tool description using the standard template:
/// summary, optional prerequisites, critical workflow, and next steps.
pub fn tool_description(
    summary: &str,
    prerequisites: &[&str],
    workflow: &[&str],
    next_steps: &[&str],
) -> String {
    let mut out = summary.trim().to_string();

    if !prerequisites.is_empty() {
        out.push_str("\n\nPrerequisites:");
        for item in prerequisites {
            out.push_str("\n- ");
            out.push_str(item);
        }
    }

    if !workflow.is_empty() {
        out.push_str("\n\n⚠️ CRITICAL WORKFLOW:");
        for (index, step) in workflow.iter().enumerate() {
            out.push_str(&format!("\n{}. {}", index + 1, step));
        }
    }

    if !next_steps.is_empty() {
        out.push_str("\n\n💡 Next Steps:");
        for step in next_steps {
            out.push_str("\n- ");
            out.push_str(step);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omits_empty_sections() {
        let desc = tool_description("Do the thing.", &[], &[], &[]);
        assert_eq!(desc, "Do the thing.");
    }

    #[test]
    fn includes_all_sections() {
        let desc = tool_description(
            "Summary.",
            &["Need X"],
            &["Do A", "Do B"],
            &["Try foo__bar"],
        );
        assert!(desc.contains("Prerequisites:\n- Need X"));
        assert!(desc.contains("⚠️ CRITICAL WORKFLOW:\n1. Do A\n2. Do B"));
        assert!(desc.contains("💡 Next Steps:\n- Try foo__bar"));
    }
}
