#!/usr/bin/env python3
"""Verify a rendered PDF for the docs skill.

Reads text positions with pdfplumber (MIT) and rejects the three defects that a
bad layout produces: an empty file, table rows that overlap each other, and cell
text that spills outside its cell. Also rejects text drawn with a base-14 font
when the character is non-ASCII, because that font has no Korean glyph and the
page would show a blank box or a notdef.

Prints `OK <abs path> <pages>` only when every check passes.
"""
from __future__ import annotations

import argparse
import pathlib
import sys

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
        description="Verify a docs-skill PDF output", add_help=False
    )
    parser.add_argument("--pdf", dest="pdf_file", type=str, help="PDF path to verify")
    parser.add_argument("-h", "--help", action="store_true", help="Show help")
    args = parser.parse_args()

    if args.help:
        parser.print_help()
        print("ExitCode: 0")
        return 0

    if not args.pdf_file:
        return fail(
            "Missing required argument '--pdf'.",
            "Specify the rendered PDF via --pdf '<abs-path>'.",
            code=2,
        )

    return run_check(pathlib.Path(args.pdf_file).resolve())


if __name__ == "__main__":
    sys.exit(main())
