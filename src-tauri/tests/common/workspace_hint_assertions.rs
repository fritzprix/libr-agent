//! Shared assertions for read-only tool hint suppression.

#[allow(dead_code)]
pub fn assert_no_edit_promotion_next_actions(text: &str) {
    assert!(
        !text.contains("💡 Next:"),
        "success response must not append next-action hints: {text}"
    );
    assert!(
        !text.contains("writeFile for full file replacement"),
        "success response must not promote writeFile: {text}"
    );
    assert!(
        !text.contains("strReplace.old_string"),
        "success response must not expose internal param names: {text}"
    );
}
