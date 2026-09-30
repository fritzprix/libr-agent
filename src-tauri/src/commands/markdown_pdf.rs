//! Markdown → PDF via the `markdown2pdf` crate (proven out-of-the-box engine).
//!
//! Uses the bundled `github` theme plus platform fonts so Hangul / emoji
//! render instead of tofu boxes (Helvetica alone cannot cover them).
//!
//! Optional PNG embeds use placeholders `libragent-pdf-embed:N` (filled by
//! the frontend Mermaid preprocess) so diagrams land as images without
//! changing the github-theme pipeline.

use base64::{engine::general_purpose, Engine as _};
use markdown2pdf::config::ConfigSource;
use markdown2pdf::fonts::{FontConfig, FontSource};
use serde::Deserialize;
use std::path::PathBuf;

/// PNG (or other `image`-crate-decodable) payload referenced from Markdown
/// via `libragent-pdf-embed:{index}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfEmbeddedImage {
    pub data_base64: String,
}

/// Convert Markdown content to PDF bytes using the github theme + Unicode fonts.
pub fn build_markdown_pdf(markdown: &str) -> Result<Vec<u8>, String> {
    let font_config = build_unicode_font_config();
    markdown2pdf::parse_into_bytes(
        markdown.to_string(),
        ConfigSource::Theme("github"),
        Some(&font_config),
    )
    .map_err(|e| format!("markdown2pdf failed: {e}"))
}

/// Like [`build_markdown_pdf`], but materializes `embedded_images` into a
/// temp directory and rewrites `libragent-pdf-embed:N` markers to absolute
/// file paths before rendering. Temp files live until the PDF is built.
pub fn build_markdown_pdf_with_embeds(
    markdown: &str,
    embedded_images: &[PdfEmbeddedImage],
) -> Result<Vec<u8>, String> {
    if embedded_images.is_empty() {
        return build_markdown_pdf(markdown);
    }

    let temp_dir = tempfile::tempdir().map_err(|e| format!("temp dir for PDF embeds: {e}"))?;
    let mut path_by_index: Vec<(usize, String)> = Vec::with_capacity(embedded_images.len());

    for (index, image) in embedded_images.iter().enumerate() {
        let bytes = general_purpose::STANDARD
            .decode(image.data_base64.trim())
            .map_err(|e| format!("invalid PDF embed base64 at index {index}: {e}"))?;
        if bytes.is_empty() {
            return Err(format!("empty PDF embed at index {index}"));
        }

        let file_path = temp_dir.path().join(format!("embed-{index}.png"));
        std::fs::write(&file_path, &bytes)
            .map_err(|e| format!("failed to write PDF embed {index}: {e}"))?;

        // Forward slashes keep markdown image URLs portable on Windows.
        let path_for_md = file_path.to_string_lossy().replace('\\', "/");
        path_by_index.push((index, path_for_md));
    }

    let rewritten = rewrite_pdf_embed_markers(markdown, &path_by_index);
    let pdf = build_markdown_pdf(&rewritten)?;
    // Keep temp_dir alive until after render (images are read during parse).
    drop(temp_dir);
    Ok(pdf)
}

/// Replace `libragent-pdf-embed:N` with file paths.
///
/// Indices are applied **highest-first** so `…:1` cannot corrupt `…:10`
/// (plain `.replace` treats the shorter marker as a prefix of the longer one).
pub(crate) fn rewrite_pdf_embed_markers(
    markdown: &str,
    path_by_index: &[(usize, String)],
) -> String {
    let mut indices: Vec<usize> = path_by_index.iter().map(|(index, _)| *index).collect();
    indices.sort_unstable_by(|a, b| b.cmp(a));

    let mut rewritten = markdown.to_string();
    for index in indices {
        let Some((_, path)) = path_by_index.iter().find(|(i, _)| *i == index) else {
            continue;
        };
        let marker = format!("libragent-pdf-embed:{index}");
        rewritten = rewritten.replace(&marker, path);
    }
    rewritten
}

fn build_unicode_font_config() -> FontConfig {
    let mut config = FontConfig::new().with_subsetting(true);

    if let Some(path) = first_existing_font(&body_font_candidates()) {
        config = config.with_default_font_source(FontSource::file(path));
    } else {
        // Prefer a Unicode-capable system face when path lookup fails.
        config = config.with_default_font(default_body_font_name());
    }

    if let Some(path) = first_existing_font(&mono_font_candidates()) {
        config = config.with_code_font_source(FontSource::file(path));
    } else {
        config = config.with_code_font(default_mono_font_name());
    }

    for path in fallback_font_candidates()
        .into_iter()
        .filter(|p| p.is_file())
    {
        config = config.add_fallback_font_source(FontSource::file(path));
    }

    // Name-based fallbacks for environments where only registry fonts exist.
    for name in fallback_font_names() {
        config = config.add_fallback_font(name);
    }

    config
}

fn first_existing_font(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}

#[cfg(windows)]
fn windows_fonts_dir() -> Option<PathBuf> {
    std::env::var_os("WINDIR").map(|windir| PathBuf::from(windir).join("Fonts"))
}

fn body_font_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(windows)]
    if let Some(fonts) = windows_fonts_dir() {
        paths.extend([
            fonts.join("malgun.ttf"),
            fonts.join("malgunsl.ttf"),
            fonts.join("YuGothM.ttc"),
            fonts.join("meiryo.ttc"),
            fonts.join("msyh.ttc"),
            fonts.join("arial.ttf"),
        ]);
    }

    #[cfg(target_os = "macos")]
    {
        paths.extend([
            PathBuf::from("/System/Library/Fonts/AppleSDGothicNeo.ttc"),
            PathBuf::from("/Library/Fonts/AppleGothic.ttf"),
            PathBuf::from("/System/Library/Fonts/Supplemental/Arial Unicode.ttf"),
            PathBuf::from("/Library/Fonts/Arial Unicode.ttf"),
            PathBuf::from("/System/Library/Fonts/Supplemental/AppleGothic.ttf"),
        ]);
    }

    #[cfg(target_os = "linux")]
    {
        paths.extend([
            PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
            PathBuf::from("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc"),
            PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansKR-Regular.otf"),
            PathBuf::from("/usr/share/fonts/truetype/noto/NotoSansKR-Regular.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/nanum/NanumGothic.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        ]);
    }

    paths
}

fn mono_font_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(windows)]
    if let Some(fonts) = windows_fonts_dir() {
        paths.extend([
            fonts.join("consola.ttf"),
            fonts.join("CascadiaMono.ttf"),
            fonts.join("cour.ttf"),
            fonts.join("malgun.ttf"),
        ]);
    }

    #[cfg(target_os = "macos")]
    {
        paths.extend([
            PathBuf::from("/System/Library/Fonts/Menlo.ttc"),
            PathBuf::from("/System/Library/Fonts/SFNSMono.ttf"),
            PathBuf::from("/System/Library/Fonts/AppleSDGothicNeo.ttc"),
        ]);
    }

    #[cfg(target_os = "linux")]
    {
        paths.extend([
            PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/noto/NotoSansKR-Regular.ttf"),
        ]);
    }

    paths
}

fn fallback_font_candidates() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(windows)]
    if let Some(fonts) = windows_fonts_dir() {
        paths.extend([
            // Hangul
            fonts.join("malgun.ttf"),
            fonts.join("malgunbd.ttf"),
            // Symbols / emoji-ish coverage (B&W symbol font is safer than COLR emoji)
            fonts.join("seguisym.ttf"),
            fonts.join("seguiemj.ttf"),
            fonts.join("segmdl2.ttf"),
        ]);
    }

    #[cfg(target_os = "macos")]
    {
        paths.extend([
            PathBuf::from("/System/Library/Fonts/AppleSDGothicNeo.ttc"),
            PathBuf::from("/System/Library/Fonts/Supplemental/Arial Unicode.ttf"),
            PathBuf::from("/System/Library/Fonts/Apple Color Emoji.ttc"),
            PathBuf::from("/System/Library/Fonts/Symbol.ttf"),
        ]);
    }

    #[cfg(target_os = "linux")]
    {
        paths.extend([
            PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
            PathBuf::from("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc"),
            PathBuf::from("/usr/share/fonts/truetype/noto/NotoSansKR-Regular.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/ancient-scripts/Symbola_hint.ttf"),
            PathBuf::from("/usr/share/fonts/truetype/ancient-scripts/Symbola.ttf"),
        ]);
    }

    paths
}

fn default_body_font_name() -> &'static str {
    #[cfg(windows)]
    {
        "Malgun Gothic"
    }
    #[cfg(target_os = "macos")]
    {
        "Apple SD Gothic Neo"
    }
    #[cfg(target_os = "linux")]
    {
        "Noto Sans CJK KR"
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        "Helvetica"
    }
}

fn default_mono_font_name() -> &'static str {
    #[cfg(windows)]
    {
        "Consolas"
    }
    #[cfg(target_os = "macos")]
    {
        "Menlo"
    }
    #[cfg(target_os = "linux")]
    {
        "DejaVu Sans Mono"
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        "Courier"
    }
}

fn fallback_font_names() -> Vec<&'static str> {
    #[cfg(windows)]
    {
        vec![
            "Malgun Gothic",
            "Segoe UI Symbol",
            "Segoe UI Emoji",
            "Yu Gothic",
            "Microsoft YaHei",
        ]
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            "Apple SD Gothic Neo",
            "Arial Unicode MS",
            "Apple Color Emoji",
            "Hiragino Sans",
        ]
    }
    #[cfg(target_os = "linux")]
    {
        vec![
            "Noto Sans CJK KR",
            "Noto Sans KR",
            "NanumGothic",
            "Noto Color Emoji",
            "Symbola",
            "DejaVu Sans",
        ]
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        body_font_candidates, build_markdown_pdf, build_markdown_pdf_with_embeds,
        first_existing_font, rewrite_pdf_embed_markers, PdfEmbeddedImage,
    };

    #[test]
    fn build_markdown_pdf_creates_pdf_header() {
        let md = "## Answer\n\n**Bold** text with a list:\n\n1. One\n2. Two\n\n```\ncode\n```\n";
        let bytes = build_markdown_pdf(md).expect("pdf");
        let header = String::from_utf8_lossy(&bytes[..8]);
        assert!(header.starts_with("%PDF-"));
        assert!(bytes.len() > 200);
    }

    #[test]
    fn build_markdown_pdf_accepts_hangul_and_emoji_input() {
        let md = "## 안녕하세요 👋\n\n한글과 emoji가 포함됩니다.\n";
        let bytes = build_markdown_pdf(md).expect("pdf with unicode");
        assert!(bytes.starts_with(b"%PDF-"));
        // When a CJK-capable font is present, embedding makes the PDF larger
        // than a Latin-only Helvetica document.
        if first_existing_font(&body_font_candidates()).is_some() {
            assert!(
                bytes.len() > 2_000,
                "expected embedded Unicode font subset, got {} bytes",
                bytes.len()
            );
        }
    }

    #[test]
    fn build_markdown_pdf_typesets_inline_and_display_math() {
        let plain = "## Answer\n\nNo formulas here.\n";
        let with_math =
            "## Answer\n\nInline $E=mc^2$ and display:\n\n$$\n\\frac{a}{b}+\\sqrt{x}\n$$\n";
        let plain_bytes = build_markdown_pdf(plain).expect("plain pdf");
        let math_bytes = build_markdown_pdf(with_math).expect("math pdf");
        assert!(math_bytes.starts_with(b"%PDF-"));
        // TeX outlines add content streams beyond plain text of similar length.
        assert!(
            math_bytes.len() > plain_bytes.len(),
            "expected math PDF ({}) larger than plain ({})",
            math_bytes.len(),
            plain_bytes.len()
        );
    }

    #[test]
    fn build_markdown_pdf_with_embeds_inlines_png() {
        // 1x1 PNG (red pixel)
        const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let md = "## Diagram\n\n![Mermaid](libragent-pdf-embed:0)\n";
        let plain = build_markdown_pdf("## Diagram\n\nplaceholder\n").expect("plain");
        let with_img = build_markdown_pdf_with_embeds(
            md,
            &[PdfEmbeddedImage {
                data_base64: PNG_1X1.to_string(),
            }],
        )
        .expect("embed pdf");
        assert!(with_img.starts_with(b"%PDF-"));
        assert!(
            with_img.len() > plain.len(),
            "expected embedded image PDF ({}) larger than plain ({})",
            with_img.len(),
            plain.len()
        );
    }

    #[test]
    fn rewrite_pdf_embed_markers_does_not_corrupt_double_digit_indices() {
        // Ascending `.replace` would turn `:10` into `/tmp/embed-1.png0` when
        // substituting `:1`. Highest-first must leave both paths intact.
        let md = "![a](libragent-pdf-embed:1)\n![b](libragent-pdf-embed:10)\n![c](libragent-pdf-embed:2)\n";
        let rewritten = rewrite_pdf_embed_markers(
            md,
            &[
                (1, "/tmp/embed-1.png".to_string()),
                (2, "/tmp/embed-2.png".to_string()),
                (10, "/tmp/embed-10.png".to_string()),
            ],
        );
        assert!(rewritten.contains("/tmp/embed-1.png"));
        assert!(rewritten.contains("/tmp/embed-10.png"));
        assert!(rewritten.contains("/tmp/embed-2.png"));
        assert!(!rewritten.contains("embed-1.png0"));
        assert!(!rewritten.contains("libragent-pdf-embed:"));
    }
}
