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

import math

EMU_PER_POINT = 12700
POINTS_PER_INCH = 72.0

# PowerPoint's single line spacing is about 1.2 times the font size.
LINE_SPACING = 1.2

# python-pptx defaults: 0.1in of inset on each side of a text frame or cell.
INSET_X_PT = 0.1 * POINTS_PER_INCH
INSET_Y_PT = 0.1 * POINTS_PER_INCH

# Mean glyph advance as a fraction of the font size. Hangul advances about one
# em per character and Latin much less; these decks mix both, so this figure
# leans wide.
GLYPH_WIDTH = 0.55


def emu_to_points(value) -> float:
    """Convert an EMU length to points."""
    return float(value) / EMU_PER_POINT


def points_to_emu(value: float) -> int:
    """Convert points to EMU, rounded to the nearest whole EMU."""
    return int(round(value * EMU_PER_POINT))


def wrapped_lines(text: str, size_pt: float, width_pt: float) -> int:
    """Return how many lines text occupies in a box width_pt wide."""
    usable = max(1.0, width_pt - 2 * INSET_X_PT)
    per_line = max(1, int(usable / max(size_pt * GLYPH_WIDTH, 1.0)))
    return max(1, math.ceil(len(text) / per_line))


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
    """Break text at spaces into lines that fit width_pt."""
    usable = max(1.0, width_pt - 2 * INSET_X_PT)
    per_line = max(1, int(usable / max(size_pt * GLYPH_WIDTH, 1.0)))
    lines = []
    current = ""
    for word in str(text).split(" "):
        candidate = f"{current} {word}".strip() if current else word
        while len(candidate) > per_line:
            head = candidate[:per_line]
            lines.append(head)
            candidate = candidate[per_line:]
        current = candidate
    if current:
        lines.append(current)
    return lines or [""]


def lines_that_fit(size_pt: float, budget_pt: float) -> int:
    """Return how many lines of size_pt fit inside budget_pt."""
    return max(1, int(budget_pt / max(size_pt * LINE_SPACING, 1.0)))
