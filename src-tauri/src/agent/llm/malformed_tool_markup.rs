//! Detects a final assistant turn that is only text-formatted tool-call markup.
//!
//! Some models occasionally answer with `<function=NAME><parameter=…>…</function>`
//! as plain text instead of a structured tool call. The markup is never parsed or
//! executed; it is only recognized so the turn can be discarded and retried.

/// Name of the tool the markup tries to call, when `text` is essentially one
/// `<function=NAME>…</function>` block (optionally wrapped in `<tool_call>` tags)
/// outside code fences. Prose that merely mentions the markup does not match.
pub(crate) fn malformed_tool_markup_name(text: &str) -> Option<String> {
    let outside_fences = strip_code_fences(text);
    let mut rest = outside_fences.trim();
    if let Some(stripped) = rest.strip_prefix("<tool_call>") {
        rest = stripped.trim_start();
    }
    let after_open = rest.strip_prefix("<function=")?;
    let (name, body) = after_open.split_once('>')?;
    let name = name.trim();
    if name.is_empty() || name.contains(|c: char| c.is_whitespace() || c == '<') {
        return None;
    }
    let body = body.trim_end();
    if body.ends_with("</function>") || body.ends_with("</tool_call>") {
        Some(name.to_string())
    } else {
        None
    }
}

/// Removes fenced code blocks (an unterminated fence swallows the rest).
fn strip_code_fences(text: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    for (index, part) in text.split("```").enumerate() {
        if index > 0 {
            in_fence = !in_fence;
        }
        if !in_fence {
            out.push_str(part);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shapes of the three occurrences found in the Harbor jobs (#2252): the
    // traces have a closing `</tool_call>` but no opening `<tool_call>`.
    const T05: &str = "<function=scheduled_task__scheduleCallback>\n<parameter=delaySeconds>\n600\n</parameter>\n<parameter=message>\nstretch\n</parameter>\n</function>\n</tool_call>";
    const T14: &str = "<function=workspace__listDirectory>\n<parameter=path>\n/app/docs\n</parameter>\n</tool_call>";
    const T16_CHILD: &str = "<function=workspace__writeFile>\n<parameter=path>\nteam/plan.md\n</parameter>\n<parameter=content>\n# Plan\n</parameter>\n</function>";

    #[test]
    fn detects_the_observed_trace_shapes() {
        assert_eq!(
            malformed_tool_markup_name(T05).as_deref(),
            Some("scheduled_task__scheduleCallback")
        );
        assert_eq!(
            malformed_tool_markup_name(T14).as_deref(),
            Some("workspace__listDirectory")
        );
        assert_eq!(
            malformed_tool_markup_name(T16_CHILD).as_deref(),
            Some("workspace__writeFile")
        );
    }

    #[test]
    fn accepts_an_optional_tool_call_wrapper_and_surrounding_whitespace() {
        let wrapped = format!("\n  <tool_call>\n{T16_CHILD}\n</tool_call>\n");
        assert_eq!(
            malformed_tool_markup_name(&wrapped).as_deref(),
            Some("workspace__writeFile")
        );
    }

    #[test]
    fn markup_inside_a_code_fence_is_not_detected() {
        let fenced = format!("Here is the format:\n```\n{T14}\n```");
        assert_eq!(malformed_tool_markup_name(&fenced), None);
        let unterminated = format!("```xml\n{T14}");
        assert_eq!(malformed_tool_markup_name(&unterminated), None);
        let tilde_free = format!("```\n{T05}\n```\n");
        assert_eq!(malformed_tool_markup_name(&tilde_free), None);
    }

    #[test]
    fn markup_mentioned_in_prose_is_not_detected() {
        for text in [
            "The model sometimes writes <function=workspace__listDirectory> as text.",
            "Use `<function=NAME>` and `</function>` only inside tool calls.",
            &format!("I will list the folder. {T14}"),
            &format!("{T14}\nThat is the call I meant to make."),
            "<function=workspace__listDirectory> without any closing tag",
            "<function=>nothing</function>",
            "plain answer",
            "",
        ] {
            assert_eq!(malformed_tool_markup_name(text), None, "{text}");
        }
    }

    #[test]
    fn fenced_example_next_to_a_real_block_still_detects_the_block() {
        let text = format!("```\nexample\n```\n{T14}");
        assert_eq!(
            malformed_tool_markup_name(&text).as_deref(),
            Some("workspace__listDirectory")
        );
    }
}
