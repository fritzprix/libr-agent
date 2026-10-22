use once_cell::sync::Lazy;
use regex::Regex;

// Compiled regexes for HTML cleaning
static RE_SCRIPT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<script[^>]*>.*?</script>").expect("Invalid regex: RE_SCRIPT"));
static RE_STYLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?is)<style[^>]*>.*?</style>").expect("Invalid regex: RE_STYLE"));
static RE_NOSCRIPT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?is)<noscript[^>]*>.*?</noscript>").expect("Invalid regex: RE_NOSCRIPT")
});
static RE_TABLE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)<table[^>]*>").expect("Invalid regex: RE_TABLE"));
static RE_TR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)<tr[^>]*>").expect("Invalid regex: RE_TR"));
static RE_TD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)<(td|th)[^>]*>").expect("Invalid regex: RE_TD"));
static RE_TD_CLOSE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)</(td|th)>").expect("Invalid regex: RE_TD_CLOSE"));
static RE_TBODY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)</?(thead|tbody|tfoot)[^>]*>").expect("Invalid regex: RE_TBODY"));
static RE_NEWLINES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\n{2,}").expect("Invalid regex: RE_NEWLINES"));
static RE_TRAILING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[ \t]+\n").expect("Invalid regex: RE_TRAILING"));
static RE_LEADING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\n[ \t]+").expect("Invalid regex: RE_LEADING"));
static RE_SPACES: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[ \t]{2,}").expect("Invalid regex: RE_SPACES"));

/// Convert HTML to clean markdown
pub(super) fn convert_to_markdown(raw_html: &str) -> String {
    // Pre-process HTML to match legacy behavior (remove scripts, flatten tables)

    // 1. Remove script, style, noscript
    let s = RE_SCRIPT.replace_all(raw_html, "");
    let s = RE_STYLE.replace_all(&s, "");
    let s = RE_NOSCRIPT.replace_all(&s, "");

    // 2. Flatten tables by transmuting them to divs/spans
    // <table...> -> <div>
    let s = RE_TABLE.replace_all(&s, "<div>");
    let s = s.replace("</table>", "</div>");

    // <tr...> -> <div>
    let s = RE_TR.replace_all(&s, "<div>");
    let s = s.replace("</tr>", "</div>");

    // <td...> -> <span>
    let s = RE_TD.replace_all(&s, "<span> ");
    let s = RE_TD_CLOSE.replace_all(&s, "</span>");

    // Remove other table structural tags
    let s = RE_TBODY.replace_all(&s, "");

    // 3. Convert to Markdown
    let markdown = html2md::parse_html(&s);

    // Apply legacy cleaning rules
    // 1. Replace 2+ newlines with 1 (collapses paragraphs)
    let s = RE_NEWLINES.replace_all(&markdown, "\n");

    // 2. Remove trailing spaces before newline
    let s = RE_TRAILING.replace_all(&s, "\n");

    // 3. Remove leading spaces after newline
    let s = RE_LEADING.replace_all(&s, "\n");

    // 4. Replace multiple spaces/tabs with single space
    let s = RE_SPACES.replace_all(&s, " ");

    s.trim().to_string()
}

pub(super) fn extract_html_title(html: &str) -> Option<String> {
    static RE_TITLE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("title regex"));
    RE_TITLE
        .captures(html)
        .and_then(|caps| caps.get(1))
        .map(|m| {
            let cleaned = RE_SPACES.replace_all(m.as_str().trim(), " ");
            html_escape_minimal_decode(&cleaned)
        })
}

fn html_escape_minimal_decode(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}
