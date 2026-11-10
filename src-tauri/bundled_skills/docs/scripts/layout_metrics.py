#!/usr/bin/env python3
"""Estimate how much vertical space rendered text needs, in points.

python-pptx and python-docx write files; neither lays text out. PowerPoint grows
a table row or a text box when its content needs more room than the declared
height, so a deck can push its last bullet past the bottom of a slide while every
declared shape still fits. pptx_render.py needs that budget while it writes the
deck, and check.py needs the same arithmetic to catch a deck that overflows, so
both import this module: one estimate means the writer and the checker cannot
disagree.

Every number here is an approximation and deliberately errs wide. Breaking a
slide one line early costs a little whitespace; guessing low lets PowerPoint
clip the last line, which is the defect this module exists to prevent.
"""
from __future__ import annotations

import unicodedata

EMU_PER_POINT = 12700
POINTS_PER_INCH = 72.0

# PowerPoint's single line spacing is about 1.2 times the font size.
LINE_SPACING = 1.2

# python-pptx defaults: 0.1in of inset on each side of a text frame or cell.
INSET_X_PT = 0.1 * POINTS_PER_INCH
INSET_Y_PT = 0.1 * POINTS_PER_INCH

# Glyph advance as a fraction of the font size. A Hangul syllable, like any other
# East Asian Wide or Fullwidth character, advances about one em; a Latin letter
# averages much less. One shared figure cannot describe both: applying a Latin
# average to Korean text predicts about half the real line count, so a Korean
# deck overflows while the estimate says it fits. Each character is therefore
# measured by its own width.
LATIN_GLYPH_WIDTH = 0.55
WIDE_GLYPH_WIDTH = 1.0

# Characters of these East Asian width classes advance about one em.
WIDE_EAST_ASIAN_WIDTHS = ("W", "F")


def emu_to_points(value) -> float:
    """Convert an EMU length to points."""
    return float(value) / EMU_PER_POINT


def points_to_emu(value: float) -> int:
    """Convert points to EMU, rounded to the nearest whole EMU."""
    return int(round(value * EMU_PER_POINT))


def char_width_em(character: str) -> float:
    """Return the advance of one character, in em."""
    if unicodedata.east_asian_width(character) in WIDE_EAST_ASIAN_WIDTHS:
        return WIDE_GLYPH_WIDTH
    return LATIN_GLYPH_WIDTH


def text_width_pt(text: str, size_pt: float) -> float:
    """Return the advance of a whole string, in points."""
    return sum(char_width_em(character) for character in str(text)) * max(size_pt, 1.0)


def _take_prefix(text: str, size_pt: float, usable_pt: float) -> tuple[str, str]:
    """Split text into the longest head that fits usable_pt and the rest.

    The head always holds at least one character, so a single glyph wider than
    the line still advances the split instead of looping forever.
    """
    used = 0.0
    index = 0
    while index < len(text):
        advance = char_width_em(text[index]) * max(size_pt, 1.0)
        if index and used + advance > usable_pt:
            break
        used += advance
        index += 1
    return (text[:index], text[index:])


def wrapped_lines(text: str, size_pt: float, width_pt: float) -> int:
    """Return how many lines text occupies in a box width_pt wide."""
    usable = max(1.0, width_pt - 2 * INSET_X_PT)
    text = str(text)
    if not text:
        return 1
    used = 0.0
    lines = 1
    for character in text:
        advance = char_width_em(character) * max(size_pt, 1.0)
        if used > 0 and used + advance > usable:
            lines += 1
            used = 0.0
        used += advance
    return max(1, lines)


def paragraph_height_pt(
    text: str, size_pt: float, width_pt: float, space_after_pt: float = 0.0
) -> float:
    """Height of one paragraph, including its trailing space."""
    lines = wrapped_lines(text, size_pt, width_pt)
    return lines * size_pt * LINE_SPACING + space_after_pt


def cell_height_pt(text: str, size_pt: float, width_pt: float) -> float:
    """Height of one table cell; a newline starts a new paragraph."""
    parts = str(text).split("\n")
    lines = sum(wrapped_lines(part, size_pt, width_pt) for part in parts)
    return lines * size_pt * LINE_SPACING + 2 * INSET_Y_PT


def row_height_pt(cells, widths_pt, size_pt: float) -> float:
    """Height of one table row: its tallest cell decides."""
    if not widths_pt:
        return 0.0
    tallest = 0.0
    for index, cell in enumerate(cells):
        width = widths_pt[min(index, len(widths_pt) - 1)]
        tallest = max(tallest, cell_height_pt(cell, size_pt, width))
    return tallest


def table_height_pt(rows, widths_pt, size_pt: float) -> float:
    """Total height of a table: the sum of its row heights."""
    return sum(row_height_pt(row, widths_pt, size_pt) for row in rows)


def split_rows(rows, widths_pt, size_pt: float, budget_pt: float,
               header_size_pt: float | None = None):
    """Cut rows into page-sized groups, keeping a header on every group.

    Returns a list of row groups. The first row is treated as a header and is
    repeated at the top of every group after the first, because a table longer
    than one slide is unreadable without its column labels.
    """
    if not rows:
        return []
    header = rows[0]
    body = rows[1:]
    header_height = row_height_pt(
        header, widths_pt, header_size_pt if header_size_pt is not None else size_pt
    )
    groups = []
    current = [header]
    used = header_height
    for row in body:
        height = row_height_pt(row, widths_pt, size_pt)
        if current and used + height > budget_pt:
            groups.append(current)
            current = [header]
            used = header_height
        current.append(row)
        used += height
    groups.append(current)
    return groups


def wrap_words(text: str, size_pt: float, width_pt: float):
    """Break text at spaces into lines that fit width_pt.

    A word that only fits on a line of its own pushes the line already collected
    before it, which is what the docstring promises. Only a word too wide for an
    empty line is cut mid-word, and its continuation keeps the remaining text so
    no character is dropped or duplicated.
    """
    usable = max(1.0, width_pt - 2 * INSET_X_PT)
    lines: list[str] = []
    current = ""
    for word in str(text).split(" "):
        candidate = f"{current} {word}" if current else word
        if text_width_pt(candidate, size_pt) <= usable:
            current = candidate
            continue
        if current:
            lines.append(current)
            current = ""
        # The word still overflows on a line of its own, so cut it and keep the
        # remainder for the next line.
        while text_width_pt(word, size_pt) > usable:
            head, word = _take_prefix(word, size_pt, usable)
            if not head:
                head, word = word[:1], word[1:]
            lines.append(head)
        current = word
    if current:
        lines.append(current)
    return lines or [""]


def lines_that_fit(size_pt: float, budget_pt: float) -> int:
    """Return how many lines of size_pt fit inside budget_pt."""
    return max(1, int(budget_pt / max(size_pt * LINE_SPACING, 1.0)))
