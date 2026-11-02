#!/usr/bin/env python3
"""Render Markdown to a PowerPoint deck with python-pptx (MIT).

A deck is a sequence of slides, not a flowing page, so the Markdown outline is
cut into slides at heading boundaries:

* a level-1 heading starts a new section,
* a level-2 heading inside that section starts a new slide,
* content before the first level-2 heading shares the section-opening slide.

A table always gets a slide of its own, because a table needs real cells. When
one slide still holds more bullets than fit, the remainder continues onto an
extra slide rather than shrinking the text until it cannot be read.

Templates mirror pdf_render.TEMPLATES, so one --template value means the same
document identity in every output format.
"""
from __future__ import annotations

import pathlib

from pptx import Presentation
from pptx.dml.color import RGBColor
from pptx.enum.text import PP_ALIGN
from pptx.util import Emu, Inches, Pt

import md_blocks

TEMPLATES = {
    "report": {
        "accent": "#1F4E79",
        "title": "Report",
        "dark": False,
    },
    "proposal": {
        "accent": "#2E6B4F",
        "title": "Proposal",
        "dark": False,
    },
    "minutes": {
        "accent": "#444444",
        "title": "Minutes",
        "dark": True,
    },
}

DEFAULT_TEMPLATE = "report"

SLIDE_WIDTH = Inches(13.333)  # 16:9
SLIDE_HEIGHT = Inches(7.5)

# Slide bodies shrink the font as the bullet count grows, then split.
MAX_BULLETS = 8
BULLET_SIZES = ((4, 20), (6, 18), (MAX_BULLETS, 16))
MIN_BULLET_SIZE = 15

# Malgun Gothic covers Hangul on Windows and PowerPoint substitutes a
# Hangul-capable face elsewhere.
FONT = "Malgun Gothic"


class PptxRenderer:
    """Render Markdown into a .pptx using one of TEMPLATES."""

    def __init__(self, template: str = DEFAULT_TEMPLATE) -> None:
        if template not in TEMPLATES:
            raise ValueError(
                f"Unknown template '{template}'. "
                f"Available: {', '.join(sorted(TEMPLATES))}"
            )
        self.template = template
        self.config = TEMPLATES[template]
        self.accent = RGBColor.from_string(self.config["accent"].lstrip("#"))
        self.background = (
            RGBColor(0x1E, 0x1E, 0x1E)
            if self.config["dark"]
            else RGBColor(0xFF, 0xFF, 0xFF)
        )
        self.body_color = (
            RGBColor(0xF2, 0xF2, 0xF2)
            if self.config["dark"]
            else RGBColor(0x22, 0x22, 0x22)
        )
        self.muted_color = (
            RGBColor(0xAA, 0xAA, 0xAA)
            if self.config["dark"]
            else RGBColor(0x77, 0x77, 0x77)
        )

    def _blank(self, presentation):
        """Add a slide with no placeholders, so every shape is explicit."""
        slide = presentation.slides.add_slide(presentation.slide_layouts[6])
        fill = slide.background.fill
        fill.solid()
        fill.fore_color.rgb = self.background
        return slide

    def _textbox(self, slide, left, top, width, height):
        box = slide.shapes.add_textbox(left, top, width, height)
        frame = box.text_frame
        frame.word_wrap = True
        return box, frame

    def _accent_rule(self, slide, left, top, width) -> None:
        """Thin accent bar under a title."""
        shape = slide.shapes.add_shape(1, left, top, width, Emu(28575))
        shape.fill.solid()
        shape.fill.fore_color.rgb = self.accent
        shape.line.fill.background()
        shape.shadow.inherit = False

    def _set_run(self, paragraph, text, size, bold=False, color=None) -> None:
        run = paragraph.add_run()
        run.text = text
        run.font.size = Pt(size)
        run.font.bold = bold
        run.font.name = FONT
        run.font.color.rgb = color or self.body_color

    def _title_slide(self, presentation, title: str) -> None:
        slide = self._blank(presentation)
        _, frame = self._textbox(
            slide, Inches(0.9), Inches(2.6), Inches(11.5), Inches(1.6)
        )
        self._set_run(frame.paragraphs[0], title, 34, bold=True, color=self.accent)
        self._accent_rule(slide, Inches(0.9), Inches(4.15), Inches(2.4))
        _, footer = self._textbox(
            slide, Inches(0.9), Inches(4.45), Inches(11.5), Inches(0.6)
        )
        self._set_run(
            footer.paragraphs[0], self.config["title"], 14, color=self.muted_color
        )

    def _heading_slide(self, presentation, title: str) -> None:
        """Start a slide and draw its title, returning the slide."""
        slide = self._blank(presentation)
        _, frame = self._textbox(
            slide, Inches(0.8), Inches(0.55), Inches(11.7), Inches(1.15)
        )
        self._set_run(frame.paragraphs[0], title, 27, bold=True, color=self.accent)
        self._accent_rule(slide, Inches(0.8), Inches(1.72), Inches(1.9))
        return slide

    def _text_slide(self, presentation, title: str, blocks: list) -> None:
        """Render text blocks, continuing onto extra slides when they overflow."""
        chunks = [
            blocks[index : index + MAX_BULLETS]
            for index in range(0, len(blocks), MAX_BULLETS)
        ] or [[]]

        for chunk_index, chunk in enumerate(chunks):
            heading = title if chunk_index == 0 else f"{title} (continued)"
            slide = self._heading_slide(presentation, heading)
            if not chunk:
                continue

            size = MIN_BULLET_SIZE
            for limit, candidate in BULLET_SIZES:
                if len(chunk) <= limit:
                    size = candidate
                    break

            _, body = self._textbox(
                slide, Inches(0.9), Inches(2.0), Inches(11.5), Inches(4.9)
            )
            first = True
            for block in chunk:
                paragraph = body.paragraphs[0] if first else body.add_paragraph()
                first = False
                paragraph.space_after = Pt(9)

                if block.kind == "bullet":
                    prefix = "    " * (block.depth - 1)
                    marker = "•" if block.depth == 1 else "–"
                    self._set_run(paragraph, f"{prefix}{marker} {block.text}", size)
                elif block.kind == "ordered":
                    prefix = "    " * (block.depth - 1)
                    self._set_run(
                        paragraph, f"{prefix}{block.marker or '1.'} {block.text}", size
                    )
                elif block.kind == "code":
                    for line_index, line in enumerate(block.text.splitlines()):
                        target = paragraph if line_index == 0 else body.add_paragraph()
                        target.space_after = Pt(0)
                        self._set_run(
                            target,
                            line,
                            max(MIN_BULLET_SIZE - 3, 11),
                            color=self.muted_color,
                        )
                elif block.kind == "heading":
                    self._set_run(paragraph, block.text, size + 2, bold=True,
                                  color=self.accent)
                elif block.kind == "rule":
                    self._set_run(paragraph, "", size)
                else:
                    self._set_run(paragraph, block.text, size)

    def _table_slide(self, presentation, title: str, rows: list) -> None:
        """Render a Markdown table as a real PowerPoint table."""
        slide = self._heading_slide(presentation, title)

        columns = max(len(row) for row in rows)
        shape = slide.shapes.add_table(
            len(rows), columns, Inches(0.6), Inches(1.95), Inches(12.1), Inches(4.5)
        )
        table = shape.table

        for row_index, row in enumerate(rows):
            for column_index in range(columns):
                text = row[column_index] if column_index < len(row) else ""
                cell = table.cell(row_index, column_index)
                cell.text = ""
                paragraph = cell.text_frame.paragraphs[0]
                self._set_run(
                    paragraph,
                    text,
                    12 if row_index else 13,
                    bold=row_index == 0,
                    color=(
                        RGBColor(0xFF, 0xFF, 0xFF)
                        if row_index == 0
                        else self.body_color
                    ),
                )
                if row_index == 0:
                    fill = cell.fill
                    fill.solid()
                    fill.fore_color.rgb = self.accent

    def _slide_number(self, slide, number: int) -> None:
        _, frame = self._textbox(
            slide, Inches(12.0), Inches(6.95), Inches(0.9), Inches(0.35)
        )
        paragraph = frame.paragraphs[0]
        paragraph.alignment = PP_ALIGN.RIGHT
        self._set_run(paragraph, str(number), 11, color=self.muted_color)

    @staticmethod
    def _sections(blocks: list) -> list[tuple[str, list]]:
        """Cut the outline into (title, blocks) slide groups, in document order.

        A heading that nothing follows contributes no group: a level-1 heading
        used only to group level-2 sections would otherwise become an empty
        slide between the cover and the real content.
        """
        groups: list[tuple[str, list]] = []
        # The pending title is the heading the next block belongs under.
        pending: str | None = None
        buffer: list = []

        for block in blocks:
            if block.kind == "heading" and block.level <= 2:
                # Skip a heading that nothing followed; see the docstring.
                if pending is not None and buffer:
                    groups.append((pending, buffer))
                pending = block.text
                buffer = []
                continue
            if pending is None:
                # Content before any heading still needs a slide title.
                pending = ""
            buffer.append(block)

        if pending is not None and buffer:
            groups.append((pending, buffer))
        if not groups:
            groups.append(("", blocks))
        return groups

    def render(self, markdown: str, output_path: pathlib.Path) -> int:
        """Write the PPTX and return the slide count."""
        blocks = md_blocks.parse_blocks(markdown)
        if not blocks:
            raise ValueError("Markdown produced no renderable content")

        title = md_blocks.document_title(blocks)
        deck_title = title or self.config["title"]

        presentation = Presentation()
        presentation.slide_width = SLIDE_WIDTH
        presentation.slide_height = SLIDE_HEIGHT

        self._title_slide(presentation, deck_title)

        for group_title, group_blocks in self._sections(blocks):
            heading = group_title or deck_title
            # A table cannot share a slide with flowing text.
            text_blocks = [b for b in group_blocks if b.kind != "table"]
            table_blocks = [b for b in group_blocks if b.kind == "table"]

            if text_blocks:
                self._text_slide(presentation, heading, text_blocks)
            for table_block in table_blocks:
                table_title = heading if not text_blocks else f"{heading} (table)"
                self._table_slide(presentation, table_title, table_block.rows)
            if not text_blocks and not table_blocks:
                self._text_slide(presentation, heading, [])

        for index, slide in enumerate(presentation.slides, start=1):
            if index > 1:
                self._slide_number(slide, index)

        output_path.parent.mkdir(parents=True, exist_ok=True)
        presentation.save(str(output_path))
        return len(presentation.slides)


def render_markdown_to_pptx(
    markdown: str, output_path: pathlib.Path, template: str
) -> int:
    """Convenience wrapper returning the rendered slide count."""
    return PptxRenderer(template).render(markdown, output_path)
