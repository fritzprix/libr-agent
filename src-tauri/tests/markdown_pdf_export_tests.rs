//! Markdown → PDF export via markdown2pdf (github theme + Unicode fonts).

use tauri_mcp_agent_lib::commands::markdown_pdf::{
    build_markdown_pdf, build_markdown_pdf_with_embeds, PdfEmbeddedImage,
};

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

#[test]
fn build_markdown_pdf_typesets_latex_math() {
    let plain = "## Answer\n\nPlain prose only.\n";
    let with_math =
        "## Answer\n\nEnergy $E=mc^2$ and:\n\n$$\nx = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}\n$$\n";
    let plain_bytes = build_markdown_pdf(plain).expect("plain");
    let math_bytes = build_markdown_pdf(with_math).expect("math");
    assert!(math_bytes.starts_with(b"%PDF-"));
    assert!(
        math_bytes.len() > plain_bytes.len(),
        "math PDF should be larger than plain (got {} vs {})",
        math_bytes.len(),
        plain_bytes.len()
    );
}

#[test]
fn build_markdown_pdf_embeds_png_from_marker() {
    const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
    let md = "## Chart\n\n![diagram](libragent-pdf-embed:0)\n";
    let bytes = build_markdown_pdf_with_embeds(
        md,
        &[PdfEmbeddedImage {
            data_base64: PNG_1X1.to_string(),
        }],
    )
    .expect("embed pdf");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 300);
}

#[test]
fn build_markdown_pdf_embeds_eleven_pngs_without_marker_prefix_collision() {
    // Regression: ascending `.replace("…:1")` corrupts `…:10` → `…/embed-1.png0`.
    const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
    let mut md = String::from("## Many charts\n\n");
    let mut images = Vec::with_capacity(11);
    for index in 0..11 {
        md.push_str(&format!("![d{index}](libragent-pdf-embed:{index})\n\n"));
        images.push(PdfEmbeddedImage {
            data_base64: PNG_1X1.to_string(),
        });
    }
    let bytes = build_markdown_pdf_with_embeds(&md, &images).expect("11 embeds");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 500);
}
