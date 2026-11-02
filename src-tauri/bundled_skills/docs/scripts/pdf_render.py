"""Markdown -> PDF rendering for the docs skill.

Pure Python: ReportLab (BSD) for layout, markdown-it-py (MIT) for parsing.
No native system dependencies, no Chromium, no GTK.

The renderer owns all layout. A caller supplies Markdown plus a template name and
gets a finished PDF, so no model-authored rendering code is ever needed.
"""
from __future__ import annotations

import pathlib
import re
import sys

from markdown_it import MarkdownIt
from reportlab.lib import colors
from reportlab.lib.enums import TA_CENTER, TA_JUSTIFY
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import ParagraphStyle
from reportlab.lib.units import cm
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (
    BaseDocTemplate,
    Frame,
    HRFlowable,
    PageBreak,
    PageTemplate,
    Paragraph,
    Preformatted,
    Spacer,
    Table,
    TableStyle,
)
from reportlab.platypus.tableofcontents import TableOfContents

SKILL_DIR = pathlib.Path(__file__).resolve().parent.parent
FONT_PATH = SKILL_DIR / "fonts" / "NotoSansKR-Regular.ttf"
FONT_NAME = "NotoSansKR"

# Templates: layout switches plus an accent colour used by headings and rules.
TEMPLATES = {
    "report": {
        "cover": True,
        "toc": True,
        "accent": "#1F4E79",
        "margin": 2.2 * cm,
        "title": "Report",
    },
    "proposal": {
        "cover": True,
        "toc": True,
        "accent": "#2E6B4F",
        "margin": 2.0 * cm,
        "title": "Proposal",
    },
    "minutes": {
        "cover": False,
        "toc": False,
        "accent": "#444444",
        "margin": 2.0 * cm,
        "title": "Minutes",
    },
}

DEFAULT_TEMPLATE = "report"

# Every character is decided by the bundled font's cmap, never by Unicode range:
# Noto Sans KR does cover many symbols that look like emoji (U+2605 ★, U+2665 ♥,
# U+2713 ✓, U+26A0 ⚠) and does not cover others (U+2705 ✅, U+2B50 ⭐, U+2B55 ⭕).
# A range rule deletes the first group and lets the second become a .notdef box.
#
# Fallbacks apply only to characters the font cannot draw. Any other undrawable
# character becomes MISSING_MARKER so the page never shows a blank box.
EMOJI_FALLBACKS = {
    "\u2705": "[OK]",      # white heavy check mark
    "\u2714": "[v]",       # heavy check mark
    "\u2717": "[x]",       # ballot x
    "\u274c": "[X]",       # cross mark
    "\u2716": "[X]",       # heavy multiplication x
    "\u2757": "[!]",       # heavy exclamation
    "\u2139": "[i]",       # information
    "\u2b50": "[*]",       # star
    "\u2b55": "[o]",       # heavy circle
    "\u274e": "[X]",
}

MISSING_MARKER = "[?]"

# Joiners and variation selectors carry no glyph of their own. Dropping them keeps
# sequences such as U+26A0 U+FE0F (⚠️) drawable with the base character.
_INVISIBLE = frozenset("\ufe0f\ufe0e\u200d\u200b\u20e3\ufe0e")

_warned_missing: set[str] = set()
_char_to_glyph: dict[int, int] | None = None


def _font_glyph_map() -> dict[int, int]:
    """Return the cmap of the bundled font, registering it on first use."""
    global _char_to_glyph
    if _char_to_glyph is None:
        load_font()
        _char_to_glyph = pdfmetrics.getFont(FONT_NAME).face.charToGlyph
    return _char_to_glyph


def load_font() -> str:
    """Register the bundled Noto Sans KR font and return its ReportLab name."""
    if FONT_NAME in pdfmetrics.getRegisteredFontNames():
        return FONT_NAME
    if not FONT_PATH.exists():
        raise FileNotFoundError(f"Bundled font is missing: {FONT_PATH}")
    pdfmetrics.registerFont(TTFont(FONT_NAME, str(FONT_PATH)))
    # Only one weight ships, so map bold/italic onto it. Registering the same file
    # for every variant keeps ReportLab from falling back to Helvetica, which has
    # no Hangul glyphs.
    pdfmetrics.registerFontFamily(
        FONT_NAME, normal=FONT_NAME, bold=FONT_NAME, italic=FONT_NAME, boldItalic=FONT_NAME
    )
    return FONT_NAME


def sanitize_text(text: str) -> str:
    """Make text drawable with the bundled font.

    Characters the font covers are kept as they are. Characters it cannot draw
    are replaced by an ASCII fallback when one exists, and by MISSING_MARKER
    otherwise, so a gap is visible on the page instead of a blank box.
    """
    if not text:
        return text
    cmap = _font_glyph_map()
    out: list[str] = []
    for char in text:
        if char in _INVISIBLE:
            continue
        if ord(char) in cmap:
            out.append(char)
            continue
        fallback = EMOJI_FALLBACKS.get(char)
        if fallback is not None:
            out.append(fallback)
            continue
        if char not in _warned_missing:
            _warned_missing.add(char)
            print(
                f"Warning: no glyph for U+{ord(char):04X} '{char}' in {FONT_NAME}; "
                f"rendering {MISSING_MARKER}.",
                file=sys.stderr,
            )
        out.append(MISSING_MARKER)
    return "".join(out)


def wrap_code(code: str, max_width: float, font_size: float) -> str:
    """Wrap code lines to max_width points.

    Preformatted never wraps, so a long command line would be clipped at the
    frame edge. Measuring each line with the code font keeps the block inside
    the margins and preserves indentation on continuation lines.
    """
    if max_width <= 0:
        return code
    wrapped: list[str] = []
    for line in code.split("\n"):
        expanded = line.expandtabs(4)
        indent = expanded[: len(expanded) - len(expanded.lstrip(" "))]
        rest = expanded[len(indent):]
        if pdfmetrics.stringWidth(expanded, FONT_NAME, font_size) <= max_width:
            wrapped.append(expanded)
            continue
        while rest:
            # Longest prefix that fits, then back off to the last space.
            cut = len(rest)
            while cut > 1 and pdfmetrics.stringWidth(
                indent + rest[:cut], FONT_NAME, font_size
            ) > max_width:
                cut -= 1
            head, rest = rest[:cut], rest[cut:]
            if rest and " " in head:
                split_at = head.rindex(" ")
                if split_at > 0:
                    rest = head[split_at + 1:] + rest
                    head = head[:split_at]
            wrapped.append(indent + head.rstrip())
    return "\n".join(wrapped)


def escape_markup(text: str) -> str:
    """Escape XML metacharacters for ReportLab's paragraph mini-markup."""
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def inline_text(token) -> str:
    """Convert an inline token to escaped ReportLab markup, keeping emphasis."""
    if token is None:
        return ""
    if token.children is None:
        return escape_markup(sanitize_text(token.content))

    out: list[str] = []
    for child in token.children:
        if child.type in ("text", "html_inline", "code_inline"):
            out.append(escape_markup(sanitize_text(child.content)))
        elif child.type == "strong_open":
            out.append("<b>")
        elif child.type == "strong_close":
            out.append("</b>")
        elif child.type == "em_open":
            out.append("<i>")
        elif child.type == "em_close":
            out.append("</i>")
        elif child.type == "softbreak":
            out.append(" ")
        elif child.type == "hardbreak":
            out.append("<br/>")
        else:
            out.append(escape_markup(sanitize_text(child.content)))
    return "".join(out)


_TAG_RE = re.compile(r"</?(?:b|i|br\s*/?|u|font|super|sub)[^>]*>")


def plain_text(markup: str) -> str:
    """Strip ReportLab mini-markup and unescape entities back to plain text."""
    text = _TAG_RE.sub("", markup)
    return (
        text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .strip()
    )


def build_styles(accent: str) -> dict[str, ParagraphStyle]:
    """Heading and body styles shared by every template."""
    accent_color = colors.HexColor(accent)
    ink = colors.HexColor("#1A1A1A")
    base = dict(fontName=FONT_NAME, wordWrap="CJK")
    return {
        "title": ParagraphStyle(
            "DocsTitle", fontSize=26, leading=34, alignment=TA_CENTER,
            textColor=accent_color, spaceAfter=18, **base
        ),
        "h1": ParagraphStyle(
            "DocsH1", fontSize=17, leading=24, spaceBefore=16, spaceAfter=8,
            textColor=accent_color, **base
        ),
        "h2": ParagraphStyle(
            "DocsH2", fontSize=14, leading=20, spaceBefore=13, spaceAfter=6,
            textColor=accent_color, **base
        ),
        "h3": ParagraphStyle(
            "DocsH3", fontSize=12, leading=17, spaceBefore=10, spaceAfter=5,
            textColor=ink, **base
        ),
        "body": ParagraphStyle(
            "DocsBody", fontSize=10.5, leading=16.5, alignment=TA_JUSTIFY,
            spaceAfter=7, textColor=ink, **base
        ),
        "bullet": ParagraphStyle(
            "DocsBullet", fontSize=10.5, leading=16.5, leftIndent=16,
            bulletIndent=4, spaceAfter=3, textColor=ink,
            bulletFontName=FONT_NAME, bulletFontSize=10.5, **base
        ),
        # Preformatted does not wrap, so wrap_code() breaks long lines first.
        "code": ParagraphStyle(
            "DocsCode", fontName=FONT_NAME, fontSize=8.5, leading=11.5,
            leftIndent=10, spaceBefore=4, spaceAfter=8,
            textColor=colors.HexColor("#24292F"), backColor=colors.HexColor("#F4F5F7"),
            borderPadding=6,
        ),
        "cell": ParagraphStyle(
            "DocsCell", fontName=FONT_NAME, fontSize=8.5, leading=11.5,
            wordWrap="CJK", textColor=ink
        ),
        "cell_head": ParagraphStyle(
            "DocsCellHead", fontSize=8.5, leading=11.5, fontName=FONT_NAME,
            wordWrap="CJK", textColor=colors.white,
        ),
    }


class DocsDocTemplate(BaseDocTemplate):
    """DocTemplate that records headings so the table of contents can resolve."""

    def __init__(self, *args, **kwargs):
        BaseDocTemplate.__init__(self, *args, **kwargs)
        self.toc_entries: list[tuple[int, str, int]] = []

    def afterFlowable(self, flowable) -> None:
        entry = getattr(flowable, "_toc_entry", None)
        if entry is not None:
            level, text = entry
            self.notify("TOCEntry", (level, text, self.page))


class MarkdownPdfRenderer:
    """Render Markdown into a laid-out PDF using one of TEMPLATES."""

    def __init__(self, template: str = DEFAULT_TEMPLATE) -> None:
        if template not in TEMPLATES:
            raise ValueError(
                f"Unknown template '{template}'. Available: {', '.join(sorted(TEMPLATES))}"
            )
        self.template_name = template
        self.config = TEMPLATES[template]
        self.styles = build_styles(self.config["accent"])
        self.parser = MarkdownIt("commonmark").enable("table")

    # -- flowable construction -------------------------------------------------

    def _heading(self, level: int, text: str, doc_title: str):
        style = self.styles["h1"] if level == 1 else (
            self.styles["h2"] if level == 2 else self.styles["h3"]
        )
        para = Paragraph(text, style)
        # Only top two levels belong in the table of contents.
        if level <= 2 and text.strip() and text.strip() != escape_markup(doc_title):
            para._toc_entry = (0 if level == 1 else 1, plain_text(text))
        return para

    def _table(self, rows: list[list[str]], available_width: float) -> Table:
        if not rows:
            return Spacer(1, 0)
        ncols = max(len(r) for r in rows)
        padded = [r + [""] * (ncols - len(r)) for r in rows]

        data = []
        for index, row in enumerate(padded):
            style = self.styles["cell_head"] if index == 0 else self.styles["cell"]
            data.append([Paragraph(cell, style) for cell in row])

        # Equal column widths keep the total at available_width, so no row can
        # grow past the frame. CJK wrapping breaks long cells inside the column.
        col_width = available_width / ncols
        table = Table(data, colWidths=[col_width] * ncols, repeatRows=1)
        accent = colors.HexColor(self.config["accent"])
        table.setStyle(
            TableStyle(
                [
                    ("BACKGROUND", (0, 0), (-1, 0), accent),
                    ("GRID", (0, 0), (-1, -1), 0.4, colors.HexColor("#B9BEC6")),
                    ("VALIGN", (0, 0), (-1, -1), "TOP"),
                    ("LEFTPADDING", (0, 0), (-1, -1), 5),
                    ("RIGHTPADDING", (0, 0), (-1, -1), 5),
                    ("TOPPADDING", (0, 0), (-1, -1), 4),
                    ("BOTTOMPADDING", (0, 0), (-1, -1), 4),
                    ("ROWBACKGROUNDS", (0, 1), (-1, -1), [colors.white, colors.HexColor("#F7F8FA")]),
                ]
            )
        )
        return table

    def build_story(self, markdown: str, doc_title: str, available_width: float) -> list:
        """Walk the token stream once and emit one flowable per block element."""
        tokens = self.parser.parse(markdown)
        story: list = []
        # Each frame is [is_ordered, counter, pending_marker]. The marker is
        # consumed by the first paragraph of the item, so a loose item with two
        # paragraphs is numbered once, not twice.
        list_stack: list[list] = []
        index = 0
        total = len(tokens)

        def next_inline(pos: int):
            """Return the inline token that belongs to the block opened at pos."""
            for probe in range(pos + 1, min(pos + 3, total)):
                if tokens[probe].type == "inline":
                    return tokens[probe]
            return None

        while index < total:
            token = tokens[index]
            kind = token.type

            if kind == "heading_open":
                level = int(token.tag[1:])
                inline = next_inline(index)
                story.append(self._heading(level, inline_text(inline), doc_title))
                # Skip inline + heading_close so the heading text is not re-emitted.
                index += 3
                continue

            if kind == "paragraph_open":
                inline = next_inline(index)
                markup = inline_text(inline).strip()
                if markup:
                    if list_stack:
                        marker = list_stack[-1][2]
                        list_stack[-1][2] = ""
                        depth = max(1, len(list_stack))
                        style = self.styles["bullet"]
                        if depth > 1:
                            # Nested items step right so the outline stays readable.
                            style = ParagraphStyle(
                                f"DocsBullet{depth}",
                                parent=self.styles["bullet"],
                                leftIndent=16 + (depth - 1) * 14,
                                bulletIndent=4 + (depth - 1) * 14,
                            )
                        if marker:
                            story.append(Paragraph(markup, style, bulletText=marker))
                        else:
                            # Continuation paragraph of the same item.
                            story.append(Paragraph(markup, style))
                    else:
                        story.append(Paragraph(markup, self.styles["body"]))
                index += 3
                continue

            if kind == "fence" or kind == "code_block":
                code = sanitize_text(token.content.rstrip("\n"))
                if code:
                    code_style = self.styles["code"]
                    # The block is inset by leftIndent plus borderPadding on each side.
                    code_width = available_width - code_style.leftIndent - 2 * code_style.borderPadding
                    story.append(
                        Preformatted(
                            wrap_code(code, code_width, code_style.fontSize), code_style
                        )
                    )
                index += 1
                continue

            if kind == "table_open":
                rows, index = self._collect_table(tokens, index)
                story.append(self._table(rows, available_width))
                story.append(Spacer(1, 8))
                continue

            if kind in ("bullet_list_open", "ordered_list_open"):
                ordered = kind == "ordered_list_open"
                # A list written as `3.` starts at three.
                start = 1
                if ordered:
                    raw = token.attrGet("start")
                    if raw:
                        start = int(raw)
                list_stack.append([ordered, start - 1, ""])
            elif kind == "list_item_open":
                if list_stack:
                    frame = list_stack[-1]
                    if frame[0]:
                        frame[1] += 1
                        frame[2] = f"{frame[1]}."
                    else:
                        frame[2] = "\u2022"
            elif kind in ("bullet_list_close", "ordered_list_close"):
                if list_stack:
                    list_stack.pop()
            elif kind == "hr":
                story.append(Spacer(1, 6))
                story.append(
                    HRFlowable(
                        width="100%",
                        thickness=0.8,
                        color=colors.HexColor(self.config["accent"]),
                    )
                )
                story.append(Spacer(1, 6))

            index += 1

        return story

    @staticmethod
    def _collect_table(tokens: list, start: int) -> tuple[list[list[str]], int]:
        """Collect table rows from the table_open token at start."""
        rows: list[list[str]] = []
        current: list[str] | None = None
        index = start + 1
        while index < len(tokens):
            kind = tokens[index].type
            if kind == "table_close":
                return rows, index + 1
            if kind == "tr_open":
                current = []
            elif kind == "tr_close":
                if current is not None:
                    rows.append(current)
                current = None
            elif kind in ("th_open", "td_open") and current is not None:
                for probe in range(index + 1, min(index + 4, len(tokens))):
                    if tokens[probe].type == "inline":
                        current.append(inline_text(tokens[probe]))
                        break
            index += 1
        return rows, index

    # -- document assembly ----------------------------------------------------

    def render(self, markdown: str, output_path: pathlib.Path) -> int:
        """Render markdown into output_path and return the page count."""
        load_font()
        output_path.parent.mkdir(parents=True, exist_ok=True)

        doc_title = self._first_heading(markdown) or self.config["title"]
        margin = self.config["margin"]
        accent = colors.HexColor(self.config["accent"])

        def decorate(canvas, doc) -> None:
            """Draw the header and footer, and leave the cover page clean."""
            canvas.saveState()
            canvas.setFont(FONT_NAME, 8)
            canvas.setFillColor(colors.HexColor("#5A6069"))
            canvas.setStrokeColor(accent)
            canvas.setLineWidth(0.6)

            # The cover already shows the title, so a header there would repeat it.
            is_cover = bool(self.config["cover"]) and canvas.getPageNumber() == 1
            if not is_cover:
                header_y = A4[1] - margin + 0.75 * cm
                canvas.drawString(margin, header_y, sanitize_text(doc_title))
                canvas.line(
                    margin, header_y - 0.3 * cm, A4[0] - margin, header_y - 0.3 * cm
                )

            canvas.drawString(margin, 1.25 * cm, sanitize_text(doc_title))
            canvas.drawRightString(A4[0] - margin, 1.25 * cm, f"{doc.page}")
            canvas.line(margin, 1.55 * cm, A4[0] - margin, 1.55 * cm)
            canvas.restoreState()

        template = DocsDocTemplate(
            str(output_path),
            pagesize=A4,
            leftMargin=margin,
            rightMargin=margin,
            topMargin=margin,
            bottomMargin=margin,
            title=doc_title,
        )
        frame = Frame(
            margin, margin, A4[0] - 2 * margin, A4[1] - 2 * margin, id="body",
            leftPadding=0, rightPadding=0, topPadding=0, bottomPadding=0,
        )
        template.addPageTemplates(
            [PageTemplate(id="main", frames=[frame], onPage=decorate)]
        )

        available_width = A4[0] - 2 * margin
        # Judge emptiness on the body alone: the cover and table of contents are
        # always emitted, so testing the combined story would never fail.
        body_story = self.build_story(markdown, doc_title, available_width)
        if not body_story:
            raise ValueError("Markdown produced no renderable content")

        story: list = []
        if self.config["cover"]:
            story.extend(
                [Spacer(1, 6.5 * cm), Paragraph(escape_markup(sanitize_text(doc_title)), self.styles["title"])]
            )
            story.append(
                HRFlowable(width="45%", thickness=1.6, color=accent, hAlign="CENTER")
            )
            story.append(Spacer(1, 0.8 * cm))
            subtitle = Paragraph(
                escape_markup(self.config["title"]), self.styles["h3"]
            )
            subtitle.alignment = TA_CENTER
            story.append(subtitle)
            story.append(PageBreak())

        if self.config["toc"]:
            story.append(Paragraph("Contents", self.styles["h1"]))
            story.append(Spacer(1, 0.3 * cm))
            toc = TableOfContents()
            toc.levelStyles = [
                ParagraphStyle(
                    "TOC0", fontName=FONT_NAME, fontSize=11, leading=18,
                    leftIndent=0, textColor=colors.HexColor("#1A1A1A"),
                ),
                ParagraphStyle(
                    "TOC1", fontName=FONT_NAME, fontSize=9.5, leading=15,
                    leftIndent=16, textColor=colors.HexColor("#4A4F57"),
                ),
            ]
            story.append(toc)
            story.append(PageBreak())

        story.extend(body_story)

        template.multiBuild(story)
        # BaseDocTemplate.page is the last page number it painted.
        return int(template.page)

    @staticmethod
    def _first_heading(markdown: str) -> str | None:
        for line in markdown.splitlines():
            stripped = line.strip()
            if stripped.startswith("# "):
                return stripped[2:].strip()
        return None


def render_markdown_to_pdf(markdown: str, output_path: pathlib.Path, template: str) -> int:
    """Convenience wrapper returning the rendered page count."""
    return MarkdownPdfRenderer(template).render(markdown, output_path)
