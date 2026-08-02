//! Markdown → PDF via the `markdown2pdf` crate (proven out-of-the-box engine).
//!
//! Uses the bundled `github` theme plus platform fonts so Hangul / emoji
//! render instead of tofu boxes (Helvetica alone cannot cover them).

use markdown2pdf::config::ConfigSource;
use markdown2pdf::fonts::{FontConfig, FontSource};
use std::path::PathBuf;

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
    use super::{body_font_candidates, build_markdown_pdf, first_existing_font};

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
}
