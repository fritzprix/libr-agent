#!/usr/bin/env python3
"""Render Markdown to a Word document with python-docx (MIT).

DOCX is a container of text, not a laid-out page, so this renderer cannot and
does not try to reproduce the PDF geometry. It maps the Markdown outline onto
Word constructs: real Heading styles (so the navigation pane and Word's own TOC
field keep working), a cover page, a static outline, a header, a footer with a
page field, and a shaded table header row.

Korean text needs the east-Asian font slot set explicitly. Setting only
`run.font.name` writes `w:ascii` and leaves `w:eastAsia` pointing at the theme
default, and Word then draws Hangul with a fallback face.
"""
from __future__ import annotations

import datetime
import pathlib

from docx import Document
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.shared import Cm, Pt, RGBColor

import md_blocks

# Templates mirror pdf_render.TEMPLATES so one --template value means the same
# document identity in every output format.
TEMPLATES = {
    "report": {
        "cover": True,
        "toc": True,
        "accent": "#1F4E79",
        "title": "Report",
    },
    "proposal": {
        "cover": True,
        "toc": True,
        "accent": "#2E6B4F",
        "title": "Proposal",
    },
    "minutes": {
        "cover": False,
        "toc": False,
        "accent": "#444444",
        "title": "Minutes",
    },
}

DEFAULT_TEMPLATE = "report"

# Malgun Gothic ships with Windows and covers Hangul plus the CJK punctuation the
# bundled Noto Sans KR covers in the PDF path.
BODY_FONT = "Malgun Gothic"
CODE_FONT = "Consolas"


def _set_east_asian_font(rpr, name: str) -> None:
    """Point every font slot at name, including the east-Asian one."""
    fonts = rpr.find(qn("w:rFonts"))
    if fonts is None:
        fonts = OxmlElement("w:rFonts")
        rpr.append(fonts)
    for slot in ("w:ascii", "w:hAnsi", "w:eastAsia", "w:cs"):
        fonts.set(qn(slot), name)


def _shade_cell(cell, fill: str) -> None:
    """Fill a table cell with a solid colour."""
    properties = cell._tc.get_or_add_tcPr()
    shaded = OxmlElement("w:shd")
    shaded.set(qn("w:val"), "clear")
    shaded.set(qn("w:color"), "auto")
    shaded.set(qn("w:fill"), fill.lstrip("#"))
    properties.append(shaded)


def _bottom_border(paragraph, color: str, size: int = 8) -> None:
    """Draw a horizontal rule under a paragraph."""
    properties = paragraph._p.get_or_add_pPr()
    borders = OxmlElement("w:pBdr")
    bottom = OxmlElement("w:bottom")
    bottom.set(qn("w:val"), "single")
    bottom.set(qn("w:sz"), str(size))
    bottom.set(qn("w:space"), "1")
    bottom.set(qn("w:color"), color.lstrip("#"))
    borders.append(bottom)
    properties.append(borders)


def _page_field(paragraph) -> None:
    """Append a Word PAGE field, so the number is filled in when opened."""
    run = paragraph.add_run()
    begin = OxmlElement("w:fldChar")
    begin.set(qn("w:fldCharType"), "begin")
    instruction = OxmlElement("w:instrText")
    instruction.set(qn("xml:space"), "preserve")
    instruction.text = "PAGE"
    end = OxmlElement("w:fldChar")
    end.set(qn("w:fldCharType"), "end")
    run._r.append(begin)
    run._r.append(instruction)
    run._r.append(end)


# Ordered lists can nest. The decimal abstract definitions that ship with
# python-docx declare only level 0, so a nested item would have no number
# format at all. This builder writes its own definition with every level we
# allow, and each list then gets its own w:num pointing at it.
ORDERED_LIST_LEVELS = 3


def _ordered_abstract_id(document) -> str:
    """Register a multi-level decimal definition once and return its id."""
    numbering = document.part.numbering_part.element
    existing = [
        int(abstract.get(qn("w:abstractNumId")))
        for abstract in numbering.findall(qn("w:abstractNum"))
    ]
    new_id = str(max(existing + [-1]) + 1)

    abstract = OxmlElement("w:abstractNum")
    abstract.set(qn("w:abstractNumId"), new_id)
    multi = OxmlElement("w:multiLevelType")
    multi.set(qn("w:val"), "hybridMultilevel")
    abstract.append(multi)

    for level in range(ORDERED_LIST_LEVELS):
        lvl = OxmlElement("w:lvl")
        lvl.set(qn("w:ilvl"), str(level))

        start = OxmlElement("w:start")
        start.set(qn("w:val"), "1")
        lvl.append(start)

        fmt = OxmlElement("w:numFmt")
        fmt.set(qn("w:val"), "decimal")
        lvl.append(fmt)

        text = OxmlElement("w:lvlText")
        text.set(qn("w:val"), f"%{level + 1}.")
        lvl.append(text)

        justification = OxmlElement("w:lvlJc")
        justification.set(qn("w:val"), "left")
        lvl.append(justification)

        properties = OxmlElement("w:pPr")
        indent = OxmlElement("w:ind")
        indent.set(qn("w:left"), str(360 + level * 360))
        indent.set(qn("w:hanging"), "360")
        properties.append(indent)
        lvl.append(properties)

        abstract.append(lvl)

    # All abstractNum elements must precede every w:num element.
    last_abstract = None
    for child in numbering:
        if child.tag == qn("w:abstractNum"):
            last_abstract = child
    if last_abstract is None:
        numbering.insert(0, abstract)
    else:
        last_abstract.addnext(abstract)
    return new_id


def _new_list_num_id(document, abstract_id: str, start: int) -> str:
    """Register a fresh w:num for one Markdown list and return its numId.

    A new w:num pointing at the shared abstract definition gives the list its
    own counter, and w:startOverride carries the number the author wrote
    (``5.``), which Word honours for the first item.
    """
    numbering = document.part.numbering_part.element
    ids = [int(num.get(qn("w:numId"))) for num in numbering.findall(qn("w:num"))]
    new_id = str(max(ids + [0]) + 1)

    num = OxmlElement("w:num")
    num.set(qn("w:numId"), new_id)
    reference = OxmlElement("w:abstractNumId")
    reference.set(qn("w:val"), abstract_id)
    num.append(reference)

    override = OxmlElement("w:lvlOverride")
    override.set(qn("w:ilvl"), "0")
    start_override = OxmlElement("w:startOverride")
    start_override.set(qn("w:val"), str(start))
    override.append(start_override)
    num.append(override)

    numbering.append(num)
    return new_id


def _attach_numbering(paragraph, num_id: str, level: int) -> None:
    """Point a paragraph at num_id instead of inheriting the style's numId."""
    num_pr = paragraph._p.get_or_add_pPr().get_or_add_numPr()
    num_pr.get_or_add_ilvl().val = level
    num_pr.get_or_add_numId().val = int(num_id)


class DocxRenderer:
    """Render Markdown into a .docx using one of TEMPLATES."""

    def __init__(self, template: str = DEFAULT_TEMPLATE) -> None:
        if template not in TEMPLATES:
            raise ValueError(
                f"Unknown template '{template}'. "
                f"Available: {', '.join(sorted(TEMPLATES))}"
            )
        self.template = template
        self.config = TEMPLATES[template]
        self.accent = RGBColor.from_string(self.config["accent"].lstrip("#"))

    def _configure_styles(self, document) -> None:
        """Apply the body font and accent colour to the built-in styles."""
        normal = document.styles["Normal"]
        _set_east_asian_font(normal.element.get_or_add_rPr(), BODY_FONT)
        normal.font.size = Pt(10.5)

        for level in range(1, 5):
            style = document.styles[f"Heading {level}"]
            _set_east_asian_font(style.element.get_or_add_rPr(), BODY_FONT)
            style.font.color.rgb = self.accent
            style.font.name = BODY_FONT

    def _cover(self, document, title: str) -> None:
        """Cover page: title, accent rule, template label."""
        for _ in range(6):
            document.add_paragraph()
        paragraph = document.add_paragraph()
        paragraph.alignment = WD_ALIGN_PARAGRAPH.CENTER
        run = paragraph.add_run(title)
        run.font.size = Pt(28)
        run.font.bold = True
        run.font.color.rgb = self.accent
        _set_east_asian_font(run._element.get_or_add_rPr(), BODY_FONT)

        rule = document.add_paragraph()
        rule.alignment = WD_ALIGN_PARAGRAPH.CENTER
        _bottom_border(rule, self.config["accent"], size=12)

        label = document.add_paragraph()
        label.alignment = WD_ALIGN_PARAGRAPH.CENTER
        label_run = label.add_run(self.config["title"])
        label_run.font.size = Pt(12)
        label_run.font.color.rgb = RGBColor(0x66, 0x66, 0x66)
        _set_east_asian_font(label_run._element.get_or_add_rPr(), BODY_FONT)

        document.add_page_break()

    def _toc(self, document, blocks: list[md_blocks.Block]) -> None:
        """Static outline of the headings already in the document."""
        headings = [b for b in blocks if b.kind == "heading"]
        if not headings:
            return
        title = document.add_paragraph()
        run = title.add_run("Contents")
        run.font.size = Pt(16)
        run.font.bold = True
        run.font.color.rgb = self.accent
        _set_east_asian_font(run._element.get_or_add_rPr(), BODY_FONT)
        _bottom_border(title, self.config["accent"], size=6)

        for block in headings:
            entry = document.add_paragraph()
            # Word supplies page numbers only inside its own TOC field, which is
            # recalculated on open. A static outline stays correct in any reader.
            entry.paragraph_format.left_indent = Cm(0.6 * max(0, block.level - 1))
            entry.paragraph_format.space_after = Pt(2)
            text_run = entry.add_run(block.text)
            text_run.font.size = Pt(10.5 if block.level > 1 else 11.5)
            text_run.font.bold = block.level == 1
            _set_east_asian_font(text_run._element.get_or_add_rPr(), BODY_FONT)

        document.add_page_break()

    def _header_footer(self, document, title: str) -> None:
        """Title in the header, title plus page number in the footer."""
        section = document.sections[0]
        section.header.is_linked_to_previous = False
        section.footer.is_linked_to_previous = False

        header = section.header.paragraphs[0]
        header.alignment = WD_ALIGN_PARAGRAPH.RIGHT
        header_run = header.add_run(title)
        header_run.font.size = Pt(8.5)
        header_run.font.color.rgb = RGBColor(0x80, 0x80, 0x80)
        _set_east_asian_font(header_run._element.get_or_add_rPr(), BODY_FONT)
        _bottom_border(header, self.config["accent"], size=4)

        footer = section.footer.paragraphs[0]
        footer.alignment = WD_ALIGN_PARAGRAPH.CENTER
        footer_run = footer.add_run(f"{title}   ")
        footer_run.font.size = Pt(8.5)
        footer_run.font.color.rgb = RGBColor(0x80, 0x80, 0x80)
        _set_east_asian_font(footer_run._element.get_or_add_rPr(), BODY_FONT)
        _page_field(footer)

    def _table(self, document, rows: list[list[str]]) -> None:
        """Table with a shaded accent header row."""
        columns = max(len(row) for row in rows)
        table = document.add_table(rows=len(rows), cols=columns)
        table.style = "Table Grid"
        table.alignment = WD_TABLE_ALIGNMENT.CENTER
        table.autofit = True

        for row_index, row in enumerate(rows):
            for column_index in range(columns):
                text = row[column_index] if column_index < len(row) else ""
                cell = table.cell(row_index, column_index)
                cell.text = ""
                paragraph = cell.paragraphs[0]
                run = paragraph.add_run(text)
                run.font.size = Pt(9)
                _set_east_asian_font(run._element.get_or_add_rPr(), BODY_FONT)
                if row_index == 0:
                    run.font.bold = True
                    run.font.color.rgb = RGBColor(0xFF, 0xFF, 0xFF)
                    _shade_cell(cell, self.config["accent"])

        document.add_paragraph()

    def _code(self, document, code: str) -> None:
        """Code block in a monospaced face with a light shaded background."""
        paragraph = document.add_paragraph()
        paragraph.paragraph_format.space_after = Pt(6)
        run = paragraph.add_run(code)
        run.font.size = Pt(9)
        run.font.name = CODE_FONT
        _set_east_asian_font(run._element.get_or_add_rPr(), CODE_FONT)
        properties = paragraph._p.get_or_add_pPr()
        shaded = OxmlElement("w:shd")
        shaded.set(qn("w:val"), "clear")
        shaded.set(qn("w:color"), "auto")
        shaded.set(qn("w:fill"), "F2F2F2")
        properties.append(shaded)

    def render(self, markdown: str, output_path: pathlib.Path) -> int:
        """Write the DOCX and return the section count."""
        blocks = md_blocks.parse_blocks(markdown)
        if not blocks:
            raise ValueError("Markdown produced no renderable content")

        title = md_blocks.document_title(blocks) or self.config["title"]

        document = Document()
        self._configure_styles(document)
        section = document.sections[0]
        section.top_margin = Cm(2.0)
        section.bottom_margin = Cm(2.0)
        section.left_margin = Cm(2.0)
        section.right_margin = Cm(2.0)

        self._header_footer(document, title)
        if self.config["cover"]:
            self._cover(document, title)
        if self.config["toc"]:
            self._toc(document, blocks)

        # One w:num per Markdown ordered list keeps the counters independent.
        ordered_num_ids: dict[int, str] = {}
        ordered_abstract_id = _ordered_abstract_id(document)

        for block in blocks:
            if block.kind == "heading":
                level = min(max(block.level, 1), 4)
                heading = document.add_heading(block.text, level=level)
                for run in heading.runs:
                    _set_east_asian_font(run._element.get_or_add_rPr(), BODY_FONT)
            elif block.kind == "paragraph":
                paragraph = document.add_paragraph()
                run = paragraph.add_run(block.text)
                _set_east_asian_font(run._element.get_or_add_rPr(), BODY_FONT)
            elif block.kind in ("bullet", "ordered"):
                style = "List Number" if block.kind == "ordered" else "List Bullet"
                paragraph = document.add_paragraph(style=style)
                if block.kind == "ordered":
                    num_id = ordered_num_ids.get(block.list_id)
                    if num_id is None:
                        num_id = _new_list_num_id(
                            document, ordered_abstract_id, block.start
                        )
                        ordered_num_ids[block.list_id] = num_id
                    _attach_numbering(
                        paragraph,
                        num_id,
                        min(block.depth - 1, ORDERED_LIST_LEVELS - 1),
                    )
                if block.depth > 1:
                    paragraph.paragraph_format.left_indent = Cm(0.63 * block.depth)
                run = paragraph.add_run(block.text)
                _set_east_asian_font(run._element.get_or_add_rPr(), BODY_FONT)
            elif block.kind == "table":
                self._table(document, block.rows)
            elif block.kind == "code":
                self._code(document, block.text)
            elif block.kind == "rule":
                rule = document.add_paragraph()
                _bottom_border(rule, self.config["accent"], size=6)

        output_path.parent.mkdir(parents=True, exist_ok=True)
        document.save(str(output_path))
        return len(document.sections)


def render_markdown_to_docx(
    markdown: str, output_path: pathlib.Path, template: str
) -> int:
    """Convenience wrapper returning the rendered section count."""
    return DocxRenderer(template).render(markdown, output_path)
