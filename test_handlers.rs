// Test format function
fn sanitize_text(text: &str) -> String {
    text.replace("|", "\\|").replace("\n", " ")
}
