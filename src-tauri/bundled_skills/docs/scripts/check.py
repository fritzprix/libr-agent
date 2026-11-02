#!/usr/bin/env python3
"""Verify a rendered document for the docs skill.

Every format is checked for the defects a bad render produces, using only the
library that already backs it:

* PDF (pdfplumber, MIT): empty file, table rows that overlap, cell text that
  spills outside its cell, text drawn with a substitution font, and characters
  the document font has no glyph for.
* DOCX (python-docx, MIT): empty file, no paragraphs and no tables, a Hangul run
  with no w:eastAsia font (Word then draws boxes), and a table whose rows
  disagree on column count.
* PPTX (python-pptx, MIT): empty file, no slides, a slide with no shapes, body
  text or a table that runs past the slide bottom, a shape past the slide right
  edge, and a table whose rows disagree on column count.

A repeated paragraph is not a defect: two identical list items are legitimate
Markdown, so DOCX has no duplicate-text rule.

Prints `OK <abs path> <pages or slides>` only when every check passes.
"""
from __future__ import annotations

import argparse
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import layout_metrics

BASE14_FONTS = (
    "helvetica",
    "courier",
    "times",
    "symbol",
    "zapfdingbats",
)

# Geometry tolerance in points. Cell padding and hairline rules move edges by a
# fraction of a point, so exact comparison would report false failures.
OVERLAP_TOL = 0.5
OVERFLOW_TOL = 1.0

# ReportLab routes a character the selected font cannot encode into a
# substitution font. For Latin plus Hangul documents these two fonts must never
# appear, so any run drawn with them means a missing glyph.
SUBSTITUTION_FONTS = ("zapfdingbats", "symbol")

BROKEN_GLYPHS = {"\ufffd", "\u0000"}

# The renderer is expected to draw every character with the bundled font, so a
# character outside that font's cmap is a .notdef box on the page. pdfplumber
# still extracts the original character, so the gap needs the cmap to detect.
SKILL_DIR = pathlib.Path(__file__).resolve().parent.parent
FONT_PATH = SKILL_DIR / "fonts" / "NotoSansKR-Regular.ttf"
FONT_NAME = "NotoSansKR"

_char_to_glyph: dict[int, int] | None = None


def font_cmap() -> dict[int, int] | None:
    """Return the bundled font cmap, or None when ReportLab is unavailable."""
    global _char_to_glyph
    if _char_to_glyph is not None:
        return _char_to_glyph
    try:
        from reportlab.pdfbase import pdfmetrics
        from reportlab.pdfbase.ttfonts import TTFont
    except ImportError:
        return None
    if not FONT_PATH.exists():
        return None
    if FONT_NAME not in pdfmetrics.getRegisteredFontNames():
        pdfmetrics.registerFont(TTFont(FONT_NAME, str(FONT_PATH)))
    _char_to_glyph = pdfmetrics.getFont(FONT_NAME).face.charToGlyph
    return _char_to_glyph


def uses_document_font(fontname: str | None) -> bool:
    """Match the bundled font, ignoring the subset prefix a PDF writer adds."""
    # ReportLab writes a six-letter subset tag before the name, e.g.
    # 'AAAAAA+NotoSansKR-Thin', so an exact or prefix match on the bare name fails.
    lowered = (fontname or "").lower()
    if "+" in lowered:
        lowered = lowered.split("+", 1)[1]
    return lowered.startswith(FONT_NAME.lower())


def check_undrawable_chars(page, cmap: dict[int, int] | None) -> list[str]:
    """Find characters the document font cannot draw at all."""
    if cmap is None:
        return []
    for char in page.chars:
        text = char.get("text") or ""
        if not text or not uses_document_font(char.get("fontname")):
            continue
        if any(ord(item) not in cmap for item in text):
            return [
                f"page {page.page_number}: '{text}' has no glyph in {FONT_NAME} "
                f"and would print as a blank box"
            ]
    return []


def fail(msg: str, next_hint: str, code: int = 1) -> int:
    """Print uniform failure output conforming to the docs skill contract."""
    print(f"Error: {msg}", file=sys.stderr)
    print(f"ExitCode: {code}")
    print(f"NEXT: {next_hint}", file=sys.stderr)
    return code


def is_base14(fontname: str) -> bool:
    """Return True when pdfplumber reports one of ReportLab's built-in fonts."""
    lowered = (fontname or "").lower()
    return any(lowered.startswith(base) for base in BASE14_FONTS)


def check_broken_glyphs(page) -> list[str]:
    """Find characters that cannot be drawn with the font used for the run."""
    problems: list[str] = []
    for char in page.chars:
        text = char.get("text") or ""
        if not text:
            continue
        fontname = (char.get("fontname") or "").lower()
        if text in BROKEN_GLYPHS:
            problems.append(f"page {page.page_number}: replacement glyph")
            break
        if fontname.startswith(SUBSTITUTION_FONTS):
            problems.append(
                f"page {page.page_number}: font {char.get('fontname')} means a "
                f"glyph is missing from the document font"
            )
            break
        if not text.isascii() and is_base14(char.get("fontname", "")):
            problems.append(
                f"page {page.page_number}: '{text}' drawn with base-14 font "
                f"{char.get('fontname')} (no Korean glyph)"
            )
            break
    return problems


def check_table_rows(page) -> list[str]:
    """Detect overlapping rows and text that overflows its table cell."""
    problems: list[str] = []
    for table in page.find_tables():
        # pdfplumber derives row boxes from the drawn grid, so the boxes never
        # overlap each other. A real overlap shows up as a glyph that straddles a
        # horizontal rule: it starts above the line and finishes below it, so the
        # two rows print on top of each other.
        rows = [row.bbox for row in table.rows]
        for previous, current in zip(rows, rows[1:]):
            boundary = (previous[3] + current[1]) / 2
            for char in page.chars:
                straddles = (
                    char["top"] < boundary - OVERLAP_TOL
                    and char["bottom"] > boundary + OVERLAP_TOL
                )
                if not straddles:
                    continue
                if not (table.bbox[0] <= char["x0"] <= table.bbox[2]):
                    continue
                text = (char.get("text") or "").strip() or "glyph"
                problems.append(
                    f"page {page.page_number}: table rows overlap "
                    f"('{text}' spans {char['top']:.1f}-{char['bottom']:.1f} "
                    f"across row boundary {boundary:.1f})"
                )
                break
            if problems:
                break

        for row in table.rows:
            cells = [cell for cell in row.cells if cell]
            for char in page.chars:
                x0, top, x1, bottom = char["x0"], char["top"], char["x1"], char["bottom"]
                inside_row = (
                    top >= row.bbox[1] - OVERFLOW_TOL
                    and bottom <= row.bbox[3] + OVERFLOW_TOL
                )
                if not inside_row:
                    continue
                # Match the cell whose horizontal span contains where the glyph
                # starts, then measure how far it reaches past that cell's edge.
                for cell in cells:
                    if not (cell[0] - OVERFLOW_TOL <= x0 <= cell[2] + OVERFLOW_TOL):
                        continue
                    if x1 > cell[2] + OVERFLOW_TOL or x0 < cell[0] - OVERFLOW_TOL:
                        text = (char.get("text") or "").strip() or "glyph"
                        problems.append(
                            f"page {page.page_number}: cell text overflows horizontally "
                            f"('{text}' x0 {x0:.1f}-x1 {x1:.1f} vs cell "
                            f"{cell[0]:.1f}-{cell[2]:.1f})"
                        )
                    if bottom > cell[3] + OVERFLOW_TOL:
                        text = (char.get("text") or "").strip() or "glyph"
                        problems.append(
                            f"page {page.page_number}: cell text overflows vertically "
                            f"('{text}' bottom {bottom:.1f} vs cell bottom {cell[3]:.1f})"
                        )
                    break
                if problems:
                    break
            if problems:
                break
    return problems


HANGUL_RANGES = (
    (0xAC00, 0xD7A3),  # Hangul syllables
    (0x1100, 0x11FF),  # Hangul Jamo
    (0x3130, 0x318F),  # Hangul compatibility Jamo
    (0xA960, 0xA97F),  # Hangul Jamo Extended-A
    (0xD7B0, 0xD7FF),  # Hangul Jamo Extended-B
)


def _has_hangul(text: str) -> bool:
    """Return True when text contains a Hangul character."""
    for character in text:
        point = ord(character)
        for low, high in HANGUL_RANGES:
            if low <= point <= high:
                return True
    return False


def _run_east_asian_font(run) -> str | None:
    """Return the run's own w:eastAsia font, or None when the run sets none."""
    from docx.oxml.ns import qn

    rpr = run._element.rPr
    if rpr is None:
        return None
    fonts = rpr.find(qn("w:rFonts"))
    if fonts is None:
        return None
    return fonts.get(qn("w:eastAsia"))


def check_docx(path: pathlib.Path) -> tuple[list[str], int]:
    """Return (problems, section count) for a DOCX file.

    Word decides pagination when the file is opened, so there is no page count to
    report. The section count is used instead, which keeps this line and the
    `Sections:` line convert.py prints in agreement.
    """
    try:
        import docx
    except ImportError:
        return (["python-docx is not installed in this interpreter."], 0)

    try:
        document = docx.Document(str(path))
    except Exception as exc:
        return ([f"python-docx could not read the file: {exc}"], 0)

    problems: list[str] = []
    paragraphs = list(document.paragraphs)

    # Word picks a Hangul-capable font for a run only when the run names one in
    # w:eastAsia. Without it Word asks the Latin font for Hangul glyphs, and a
    # Latin face has none, so the text renders as boxes for a reader on any
    # platform. A style-level font does not count: Word resolves a run's own
    # rPr first and only falls back to the style when the run says nothing, and
    # the style fallback is exactly what renders badly here.
    #
    # Table cells hold their own paragraphs, which document.paragraphs does not
    # reach, and the writer sets the font on those runs too. Checking only the
    # body would pass a document whose table renders as boxes.
    cell_paragraphs = [
        paragraph
        for table in document.tables
        for row in table.rows
        for cell in row.cells
        for paragraph in cell.paragraphs
    ]
    for paragraph in paragraphs + cell_paragraphs:
        for run in paragraph.runs:
            text = run.text
            if not text or not _has_hangul(text):
                continue
            if _run_east_asian_font(run):
                continue
            problems.append(
                _truncate("Hangul run has no w:eastAsia font", text)
            )
            break
        else:
            continue
        break

    for table_index, table in enumerate(document.tables, start=1):
        counts = {len(row.cells) for row in table.rows}
        if len(counts) > 1:
            problems.append(
                f"table {table_index}: rows disagree on column count {sorted(counts)}"
            )

    if not paragraphs and not document.tables:
        problems.append("document has no paragraphs and no tables")

    return (problems, len(document.sections))


def _truncate(label: str, text: str, width: int = 60) -> str:
    """Return a short single-line excerpt for an error message."""
    flat = " ".join(text.split())
    if len(flat) > width:
        flat = flat[:width] + "..."
    return f"{label}: '{flat}'"


def check_pptx(path: pathlib.Path) -> tuple[list[str], int]:
    """Return (problems, slide count) for a PPTX file."""
    try:
        from pptx import Presentation
        from pptx.util import Emu
    except ImportError:
        return (["python-pptx is not installed in this interpreter."], 0)

    try:
        presentation = Presentation(str(path))
    except Exception as exc:
        return ([f"python-pptx could not read the file: {exc}"], 0)

    problems: list[str] = []
    slides = list(presentation.slides)
    if not slides:
        return (["presentation has no slides"], 0)

    slide_width = presentation.slide_width
    slide_height_pt = layout_metrics.emu_to_points(presentation.slide_height)

    for number, slide in enumerate(slides, start=1):
        shapes = list(slide.shapes)
        if not shapes:
            problems.append(f"slide {number}: has no shapes")
            continue

        for shape in shapes:
            # A shape placed past the slide edge is clipped when presented.
            if shape.left is None or shape.width is None:
                continue
            right = shape.left + shape.width
            if right > slide_width + Emu(9525):
                problems.append(
                    f"slide {number}: a shape runs past the slide edge "
                    f"(right {Emu(right).inches:.2f}in > "
                    f"{Emu(slide_width).inches:.2f}in)"
                )
                break

            if getattr(shape, "has_table", False):
                table = shape.table
                counts = {len(row.cells) for row in table.rows}
                if len(counts) > 1:
                    problems.append(
                        f"slide {number}: table rows disagree on column count "
                        f"{sorted(counts)}"
                    )
                needed = _table_needed_pt(table)
                label = "table"
            elif getattr(shape, "has_text_frame", False) and shape.text_frame.text.strip():
                widths_pt = layout_metrics.emu_to_points(shape.width)
                needed = _frame_needed_pt(shape.text_frame, widths_pt)
                label = "text box"
            else:
                continue

            # PowerPoint grows a shape whose content needs more room than the
            # shape declares, so the declared box can fit the slide while the
            # drawn text still runs off the bottom edge.
            bottom = layout_metrics.emu_to_points(shape.top) + needed
            if bottom > slide_height_pt + OVERFLOW_TOL:
                problems.append(
                    f"slide {number}: {label} runs past the slide bottom "
                    f"({bottom:.0f}pt > {slide_height_pt:.0f}pt)"
                )
                break

    return (problems, len(slides))


def _frame_needed_pt(frame, width_pt: float) -> float:
    """Return the height a text frame needs for its paragraphs."""
    paragraphs = []
    for paragraph in frame.paragraphs:
        text = paragraph.text
        if not text.strip():
            continue
        size = _paragraph_size(paragraph)
        space_after = (
            layout_metrics.emu_to_points(paragraph.space_after)
            if paragraph.space_after is not None
            else 0.0
        )
        paragraphs.append((text, size, space_after))
    if not paragraphs:
        return 0.0
    body = sum(
        layout_metrics.paragraph_height_pt(text, size, width_pt, after)
        for text, size, after in paragraphs
    )
    return body + 2 * layout_metrics.INSET_Y_PT


def _paragraph_size(paragraph) -> float:
    """Return the largest font size any run in the paragraph asks for.

    Runs are measured together because a paragraph whose runs disagree is as
    tall as its biggest one, and the estimate must not come in under.
    """
    sizes = [
        layout_metrics.emu_to_points(run.font.size)
        for run in paragraph.runs
        if run.font.size is not None
    ]
    return max(sizes) if sizes else 12.0


def _table_needed_pt(table) -> float:
    """Return the height a table needs, read from its own column widths."""
    widths_pt = [layout_metrics.emu_to_points(column.width) for column in table.columns]
    total = 0.0
    for row in table.rows:
        tallest = 0.0
        for index, cell in enumerate(row.cells):
            width = widths_pt[min(index, len(widths_pt) - 1)] if widths_pt else 0.0
            tallest = max(tallest, _cell_needed_pt(cell.text_frame, width))
        total += tallest
    return total


def _cell_needed_pt(frame, width_pt: float) -> float:
    """Return the height a table cell needs for its paragraphs."""
    body = 0.0
    for paragraph in frame.paragraphs:
        if not paragraph.text.strip():
            continue
        body += layout_metrics.paragraph_height_pt(
            paragraph.text, _paragraph_size(paragraph), width_pt
        )
    if body == 0.0:
        body = layout_metrics.paragraph_height_pt("", 12.0, width_pt)
    return body + 2 * layout_metrics.INSET_Y_PT


def run_format_check(path: pathlib.Path, kind: str) -> int:
    """Validate a DOCX or PPTX file and print the success line or failures."""
    if not path.exists():
        return fail(
            f"Output file does not exist: {path}",
            "Re-run convert.py and confirm the --out path is writable.",
        )

    if path.stat().st_size == 0:
        return fail(
            f"Output file is 0 bytes: {path}",
            "Re-run convert.py; a 0-byte file means the writer saved nothing.",
        )

    if kind == "docx":
        problems, count = check_docx(path)
    else:
        problems, count = check_pptx(path)

    if problems:
        print(f"Error: {kind.upper()} verification failed for {path}", file=sys.stderr)
        for problem in problems[:10]:
            print(f"  - {problem}", file=sys.stderr)
        print("ExitCode: 1")
        print(
            f"NEXT: Fix the layout defect or pick another template, then re-run convert.py.",
            file=sys.stderr,
        )
        return 1

    print(f"OK {path} {count}")
    return 0


def run_check(pdf_path: pathlib.Path) -> int:
    """Validate pdf_path and print the success line or a failure report."""
    if not pdf_path.exists():
        return fail(
            f"Output file does not exist: {pdf_path}",
            "Re-run convert.py and confirm the --out path is writable.",
        )

    size = pdf_path.stat().st_size
    if size == 0:
        return fail(
            f"Output file is 0 bytes: {pdf_path}",
            "Re-run convert.py; a 0-byte PDF means the renderer wrote nothing.",
        )

    try:
        import pdfplumber
    except ImportError:
        return fail(
            "pdfplumber is not installed in this interpreter.",
            "Run the doctor command to install the docs skill dependencies.",
        )

    problems: list[str] = []
    try:
        with pdfplumber.open(pdf_path) as pdf:
            pages = len(pdf.pages)
            if pages == 0:
                return fail(
                    f"PDF has no pages: {pdf_path}",
                    "Re-run convert.py with non-empty Markdown input.",
                )
            cmap = font_cmap()
            for page in pdf.pages:
                problems.extend(check_broken_glyphs(page))
                problems.extend(check_undrawable_chars(page, cmap))
                problems.extend(check_table_rows(page))
    except Exception as exc:
        return fail(
            f"pdfplumber could not read {pdf_path}: {exc}",
            "Re-run convert.py; the file is not a valid PDF.",
        )

    if problems:
        print(f"Error: PDF verification failed for {pdf_path}", file=sys.stderr)
        for problem in problems[:10]:
            print(f"  - {problem}", file=sys.stderr)
        print("ExitCode: 1")
        print(
            "NEXT: Fix the layout defect or pick a narrower template, then re-run convert.py.",
            file=sys.stderr,
        )
        return 1

    print(f"OK {pdf_path} {pages}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Verify a docs-skill rendered document", add_help=False
    )
    parser.add_argument("--pdf", dest="pdf_file", type=str, help="PDF path to verify")
    parser.add_argument("--docx", dest="docx_file", type=str, help="DOCX path to verify")
    parser.add_argument("--pptx", dest="pptx_file", type=str, help="PPTX path to verify")
    parser.add_argument("-h", "--help", action="store_true", help="Show help")
    args = parser.parse_args()

    if args.help:
        parser.print_help()
        print("ExitCode: 0")
        return 0

    chosen = [
        ("pdf", args.pdf_file),
        ("docx", args.docx_file),
        ("pptx", args.pptx_file),
    ]
    given = [(kind, value) for kind, value in chosen if value]
    if not given:
        return fail(
            "Missing required argument: one of --pdf, --docx, --pptx.",
            "Specify the rendered file, e.g. --pdf '<abs-path>'.",
            code=2,
        )
    if len(given) > 1:
        return fail(
            "Only one of --pdf, --docx, --pptx may be given.",
            "Pass exactly one file to verify.",
            code=2,
        )

    kind, value = given[0]
    if kind == "pdf":
        return run_check(pathlib.Path(value).resolve())
    return run_format_check(pathlib.Path(value).resolve(), kind)


if __name__ == "__main__":
    sys.exit(main())
