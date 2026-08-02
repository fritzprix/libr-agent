//! Markdown → PDF export via markdown2pdf (github theme + Unicode fonts).

use tauri_mcp_agent_lib::commands::markdown_pdf::build_markdown_pdf;

#[test]
fn build_markdown_pdf_renders_github_themed_pdf() {
    let md = r#"## Answer

**Bold** intro with a list:

1. First item
2. Second item

```rust
fn main() {}
```

> A quote line
"#;
    let bytes = build_markdown_pdf(md).expect("pdf bytes");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 500);
}

#[test]
fn build_markdown_pdf_handles_hangul_and_emoji() {
    let md = "## 안녕하세요 👋\n\n한글 메시지와 emoji ✅\n";
    let bytes = build_markdown_pdf(md).expect("unicode pdf");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 200);
}
