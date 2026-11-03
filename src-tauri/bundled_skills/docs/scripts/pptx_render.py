#!/usr/bin/env python3
"""Render Markdown to a PowerPoint deck with python-pptx (MIT).

A deck is a sequence of slides, not a flowing page, so the Markdown outline is
cut into slides at heading boundaries:

* a level-1 heading starts a new section,
* a level-2 heading inside that section starts a new slide,
* content before the first level-2 heading shares the section-opening slide.

A table always gets a slide of its own, because a table needs real cells. A table
taller than one slide continues onto another with its header row repeated.

A text slide closes on whichever limit it reaches first: `MAX_BLOCKS` separate
blocks, or the vertical room its text actually needs (see layout_metrics). A
forty-line code block is one block but far more than one slide of text, so the
block count alone cannot decide. The remainder continues onto an extra slide
titled "(continued)" rather than shrinking the text until it cannot be read.

Templates mirror pdf_render.TEMPLATES, so one --template value means the same
document identity in every output format.
"""
from __future__ import annotations

from dataclasses import dataclass
import pathlib

from pptx import Presentation
from pptx.dml.color import RGBColor
from pptx.enum.text import PP_ALIGN
from pptx.util import Emu, Inches, Pt

import layout_metrics
import md_blocks

TEMPLATES = {
    "report": {
        "accent": "#1F4E79",
        "title": "Report",
        "dark": False,
        # A report and a proposal are read as arguments, so they open with an
        # agenda. Minutes are read after the fact and do not.
        "toc": True,
    },
    "proposal": {
        "accent": "#2E6B4F",
        "title": "Proposal",
        "dark": False,
        "toc": True,
    },
    "minutes": {
        "accent": "#444444",
        "title": "Minutes",
        "dark": True,
        "toc": False,
    },
}

DEFAULT_TEMPLATE = "report"

SLIDE_WIDTH = Inches(13.333)  # 16:9
SLIDE_HEIGHT = Inches(7.5)

# Body geometry. Text stays above BODY_BOTTOM so the footer drawn below it never
# collides with the last line.
BODY_LEFT = Inches(0.9)
BODY_TOP = Inches(2.0)
BODY_WIDTH = Inches(11.5)
BODY_BOTTOM = Inches(6.85)

FOOTER_TOP = Inches(6.95)
FOOTER_HEIGHT = Inches(0.4)

TABLE_LEFT = Inches(0.6)
TABLE_TOP = Inches(1.95)
TABLE_WIDTH = Inches(12.1)
TABLE_BOTTOM = Inches(6.75)

# A slide closes on this many separate blocks, or when their text runs out of
# vertical room, whichever comes first.
MAX_BLOCKS = 8

BODY_SIZE = 18
SUB_HEADING_SIZE = 20
CODE_SIZE = 12
TABLE_SIZE = 12
TABLE_HEADER_SIZE = 13
BULLET_SPACE_AFTER = 9
AGENDA_SIZE = 18
AGENDA_MIN_SIZE = 14
FOOTER_SIZE = 10

BODY_BUDGET_PT = (
    layout_metrics.emu_to_points(BODY_BOTTOM - BODY_TOP)
    - 2 * layout_metrics.INSET_Y_PT
)
TABLE_BUDGET_PT = layout_metrics.emu_to_points(TABLE_BOTTOM - TABLE_TOP)
AGENDA_BUDGET_PT = BODY_BUDGET_PT
BODY_WIDTH_PT = layout_metrics.emu_to_points(BODY_WIDTH)

# Malgun Gothic covers Hangul on Windows and PowerPoint substitutes a
# Hangul-capable face elsewhere.
FONT = "Malgun Gothic"


@dataclass
class Paragraph:
    """One paragraph the deck will draw, with the size it needs."""

    text: str
    size: float
    bold: bool = False
    color: RGBColor | None = None
    space_after: float = 0.0


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
        self.deck_title = self.config["title"]
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

    def _draw_paragraphs(self, slide, specs):
        """Draw paragraph specs into one body text box on slide."""
        _, body = self._textbox(
            slide, BODY_LEFT, BODY_TOP, BODY_WIDTH, BODY_BOTTOM - BODY_TOP
        )
        for index, spec in enumerate(specs):
            paragraph = body.paragraphs[0] if index == 0 else body.add_paragraph()
            paragraph.space_after = Pt(spec.space_after)
            self._set_run(
                paragraph, spec.text, spec.size, bold=spec.bold, color=spec.color
            )

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

    def _heading_slide(self, presentation, title: str):
        """Start a slide and draw its title, returning the slide."""
        slide = self._blank(presentation)
        _, frame = self._textbox(
            slide, Inches(0.8), Inches(0.55), Inches(11.7), Inches(1.15)
        )
        self._set_run(frame.paragraphs[0], title, 27, bold=True, color=self.accent)
        self._accent_rule(slide, Inches(0.8), Inches(1.72), Inches(1.9))
        return slide

    def _block_paragraphs(self, block):
        """Flatten one outline block into the paragraphs that draw it.

        A code block becomes one paragraph per line so the slide budget can
        break it anywhere; a forty-line code block is one Markdown block but far
        more than one slide of text.
        """
        indent = "    " * (block.depth - 1)
        if block.kind == "bullet":
            marker = "\u2022" if block.depth == 1 else "\u2013"
            return [
                Paragraph(
                    f"{indent}{marker} {block.text}",
                    BODY_SIZE,
                    color=self.body_color,
                    space_after=BULLET_SPACE_AFTER,
                )
            ]
        if block.kind == "ordered":
            # A loose list continues an item in a paragraph of its own, and the
            # outline carries no number for it. Keep the number's indent and
            # draw no marker rather than repeat a wrong "1.".
            lead = f"{indent}{block.marker} " if block.marker else indent
            return [
                Paragraph(
                    f"{lead}{block.text}",
                    BODY_SIZE,
                    color=self.body_color,
                    space_after=BULLET_SPACE_AFTER,
                )
            ]
        if block.kind == "code":
            label = (
                [Paragraph(block.label, CODE_SIZE, bold=True, color=self.accent)]
                if block.label
                else []
            )
            return label + [
                Paragraph(line, CODE_SIZE, color=self.muted_color)
                for line in block.text.splitlines()
            ]
        if block.kind == "heading":
            return [
                Paragraph(
                    block.text,
                    SUB_HEADING_SIZE,
                    bold=True,
                    color=self.accent,
                    space_after=BULLET_SPACE_AFTER,
                )
            ]
        if block.kind == "rule":
            return [
                Paragraph(
                    "",
                    BODY_SIZE,
                    color=self.body_color,
                    space_after=BULLET_SPACE_AFTER,
                )
            ]
        return [
            Paragraph(
                block.text,
                BODY_SIZE,
                color=self.body_color,
                space_after=BULLET_SPACE_AFTER,
            )
        ]

    def _pack(self, specs, budget_pt: float):
        """Cut paragraph specs into groups that fit one slide's budget."""
        expanded = []
        for spec in specs:
            expanded.extend(self._fit(spec, budget_pt))

        chunks = []
        current = []
        used = 0.0
        for spec in expanded:
            height = layout_metrics.paragraph_height_pt(
                spec.text, spec.size, BODY_WIDTH_PT, spec.space_after
            )
            if current and used + height > budget_pt:
                chunks.append(current)
                current, used = [], 0.0
            current.append(spec)
            used += height
        if current:
            chunks.append(current)
        return chunks or [[]]

    def _fit(self, spec, budget_pt: float):
        """Break a paragraph that cannot fit one whole slide.

        A single paragraph longer than a slide would overflow wherever the slide
        broke, so it is wrapped here instead and each wrapped line becomes its
        own paragraph. Continuation lines keep the marker's indent, and only the
        last line carries the paragraph's trailing space.
        """
        lines = layout_metrics.wrap_words(spec.text, spec.size, BODY_WIDTH_PT)
        fits = layout_metrics.lines_that_fit(spec.size, budget_pt)
        if len(lines) <= 1 or len(lines) <= fits:
            return [spec]

        marker = ""
        for candidate in ("\u2022 ", "\u2013 "):
            position = spec.text.find(candidate)
            if 0 <= position <= 8:
                marker = spec.text[: position + len(candidate)]
                break

        pieces = []
        for index, line in enumerate(lines):
            text = line if index == 0 else " " * len(marker) + line
            last = index == len(lines) - 1
            pieces.append(
                Paragraph(
                    text,
                    spec.size,
                    bold=spec.bold,
                    color=spec.color,
                    space_after=spec.space_after if last else 0.0,
                )
            )
        return pieces

    def _chunk_blocks(self, blocks):
        """Group outline blocks for one slide title, honouring MAX_BLOCKS.

        A code block counts once, however many lines it holds, which is why the
        vertical budget in _pack has the final say.
        """
        chunks = []
        current = []
        for block in blocks:
            if current and len(current) >= MAX_BLOCKS:
                chunks.append(current)
                current = []
            current.append(block)
        if current:
            chunks.append(current)
        return chunks or [[]]

    def _text_slide(self, presentation, title: str, blocks) -> None:
        """Render text blocks, continuing onto extra slides when they overflow."""
        opened = 0
        for chunk in self._chunk_blocks(blocks):
            specs = []
            for block in chunk:
                specs.extend(self._block_paragraphs(block))

            if not specs:
                heading = title if opened == 0 else f"{title} (continued)"
                self._heading_slide(presentation, heading)
                opened += 1
                continue

            for part in self._pack(specs, BODY_BUDGET_PT):
                heading = title if opened == 0 else f"{title} (continued)"
                slide = self._heading_slide(presentation, heading)
                self._draw_paragraphs(slide, part)
                opened += 1

    def _table_slide(self, presentation, title: str, rows) -> None:
        """Render a Markdown table, continuing onto extra slides when tall.

        A table taller than the slide is cut into groups and the header row is
        repeated on each one; PowerPoint would otherwise draw rows past the
        bottom edge with no column labels on the visible part.
        """
        columns = max(len(row) for row in rows)
        column_pt = layout_metrics.emu_to_points(TABLE_WIDTH) / columns
        widths_pt = [column_pt] * columns

        groups = layout_metrics.split_rows(
            rows, widths_pt, TABLE_SIZE, TABLE_BUDGET_PT, TABLE_HEADER_SIZE
        )

        for index, group in enumerate(groups):
            heading = title if index == 0 else f"{title} (continued)"
            slide = self._heading_slide(presentation, heading)

            heights = [
                layout_metrics.row_height_pt(
                    row,
                    widths_pt,
                    TABLE_HEADER_SIZE if row_index == 0 else TABLE_SIZE,
                )
                for row_index, row in enumerate(group)
            ]

            shape = slide.shapes.add_table(
                len(group),
                columns,
                TABLE_LEFT,
                TABLE_TOP,
                TABLE_WIDTH,
                layout_metrics.points_to_emu(sum(heights)),
            )
            table = shape.table

            for column_index in range(columns):
                table.columns[column_index].width = layout_metrics.points_to_emu(
                    column_pt
                )
            for row_index, height in enumerate(heights):
                table.rows[row_index].height = layout_metrics.points_to_emu(height)

            for row_index, row in enumerate(group):
                header = row_index == 0
                size = TABLE_HEADER_SIZE if header else TABLE_SIZE
                for column_index in range(columns):
                    text = row[column_index] if column_index < len(row) else ""
                    cell = table.cell(row_index, column_index)
                    cell.text = ""
                    self._set_run(
                        cell.text_frame.paragraphs[0],
                        text,
                        size,
                        bold=header,
                        color=(
                            RGBColor(0xFF, 0xFF, 0xFF)
                            if header
                            else self.body_color
                        ),
                    )
                    if header:
                        fill = cell.fill
                        fill.solid()
                        fill.fore_color.rgb = self.accent

    def _image_slide(self, presentation, title: str, block) -> None:
        """Draw one picture fitted inside the body area, centred horizontally."""
        slide = self._heading_slide(presentation, title)
        width, height = md_blocks.fit_size(
            block.image_size, BODY_WIDTH, BODY_BOTTOM - BODY_TOP
        )
        left = BODY_LEFT + int((BODY_WIDTH - width) / 2)
        slide.shapes.add_picture(
            block.image_path, left, BODY_TOP, Emu(int(width)), Emu(int(height))
        )

    def _agenda_slide(self, presentation, titles) -> None:
        """List the section titles so the reader knows the deck's shape."""
        size = AGENDA_SIZE
        while size > AGENDA_MIN_SIZE:
            needed = sum(
                layout_metrics.paragraph_height_pt(
                    title, size, BODY_WIDTH_PT, BULLET_SPACE_AFTER
                )
                for title in titles
            )
            if needed <= AGENDA_BUDGET_PT:
                break
            size -= 2

        specs = [
            Paragraph(
                f"{index + 1}. {title}",
                size,
                color=self.body_color,
                space_after=BULLET_SPACE_AFTER,
            )
            for index, title in enumerate(titles)
        ]

        for index, chunk in enumerate(self._pack(specs, AGENDA_BUDGET_PT)):
            heading = "Agenda" if index == 0 else "Agenda (continued)"
            slide = self._heading_slide(presentation, heading)
            self._draw_paragraphs(slide, chunk)

    def _footer(self, slide, number: int) -> None:
        """Draw the deck title beside the slide number."""
        _, frame = self._textbox(
            slide, BODY_LEFT, FOOTER_TOP, Inches(10.0), FOOTER_HEIGHT
        )
        paragraph = frame.paragraphs[0]
        paragraph.alignment = PP_ALIGN.LEFT
        self._set_run(
            paragraph, self.deck_title, FOOTER_SIZE, color=self.muted_color
        )

        _, counter = self._textbox(
            slide, Inches(12.0), FOOTER_TOP, Inches(0.7), FOOTER_HEIGHT
        )
        counter.paragraphs[0].alignment = PP_ALIGN.RIGHT
        self._set_run(
            counter.paragraphs[0],
            str(number),
            FOOTER_SIZE,
            color=self.muted_color,
        )

    @staticmethod
    def _sections(blocks):
        """Cut the outline into (title, blocks) slide groups, in document order.

        A heading that nothing follows contributes no group: a level-1 heading
        used only to group level-2 sections would otherwise become an empty
        slide between the cover and the real content.
        """
        groups = []
        # The pending title is the heading the next block belongs under.
        pending = None
        buffer = []

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

    def render(
        self,
        markdown: str,
        output_path: pathlib.Path,
        base_dir: pathlib.Path | None = None,
    ) -> int:
        """Write the PPTX and return the slide count."""
        blocks = md_blocks.parse_blocks(markdown, base_dir)
        if not blocks:
            raise ValueError("Markdown produced no renderable content")

        title = md_blocks.document_title(blocks)
        self.deck_title = title or self.config["title"]

        presentation = Presentation()
        presentation.slide_width = SLIDE_WIDTH
        presentation.slide_height = SLIDE_HEIGHT

        self._title_slide(presentation, self.deck_title)

        sections = self._sections(blocks)
        # An agenda needs something to list; a one-section deck has none.
        agenda_titles = [name for name, _ in sections if name]
        if self.config["toc"] and len(agenda_titles) >= 2:
            self._agenda_slide(presentation, agenda_titles)

        for group_title, group_blocks in sections:
            heading = group_title or self.deck_title
            # A table or a picture cannot share a slide with flowing text.
            text_blocks = [b for b in group_blocks if b.kind not in ("table", "image")]
            own_slide_blocks = [b for b in group_blocks if b.kind in ("table", "image")]

            if text_blocks:
                self._text_slide(presentation, heading, text_blocks)
            # One pass keeps tables and pictures in source order.
            for block in own_slide_blocks:
                if block.kind == "table":
                    table_title = heading if not text_blocks else f"{heading} (table)"
                    self._table_slide(presentation, table_title, block.rows)
                else:
                    self._image_slide(presentation, block.text or heading, block)
            if not text_blocks and not own_slide_blocks:
                self._text_slide(presentation, heading, [])

        # The cover already shows the deck title, so numbering starts at 2.
        for index, slide in enumerate(presentation.slides, start=1):
            if index > 1:
                self._footer(slide, index)

        output_path.parent.mkdir(parents=True, exist_ok=True)
        presentation.save(str(output_path))
        return len(presentation.slides)


def render_markdown_to_pptx(
    markdown: str,
    output_path: pathlib.Path,
    template: str,
    base_dir: pathlib.Path | None = None,
) -> int:
    """Convenience wrapper returning the rendered slide count."""
    return PptxRenderer(template).render(markdown, output_path, base_dir)
