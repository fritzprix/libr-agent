#!/usr/bin/env python3
"""Flatten Markdown into the small block list the DOCX and PPTX writers need.

The PDF path walks markdown-it tokens straight into ReportLab flowables because
ReportLab stacks flowables itself. Word and PowerPoint do not: each writer has to
decide where a page or slide breaks, so it needs the document outline as data
before anything is drawn. This module produces that outline once.

Emoji are kept as-is. DOCX and PPTX store Unicode text and the reader's font draws
it, so unlike the PDF path there is no font to work around here.
"""
from __future__ import annotations

from dataclasses import dataclass, field

from markdown_it import MarkdownIt


@dataclass
class Block:
    """One block-level element of a Markdown document."""

    kind: str  # heading | paragraph | bullet | ordered | table | code | rule
    text: str = ""
    level: int = 0
    depth: int = 0
    marker: str = ""
    rows: list[list[str]] = field(default_factory=list)
    # Ordered lists need two extra facts. `list_id` distinguishes separate
    # lists in the same document so a renderer can restart numbering instead
    # of running one counter for the whole file. `start` keeps the number the
    # author wrote (5. six. seven.) for formats that number natively.
    list_id: int = 0
    start: int = 1


def inline_text(token) -> str:
    """Render an inline token to plain text, keeping the characters themselves."""
    if token is None:
        return ""
    parts: list[str] = []
    for child in token.children or []:
        if child.type in ("text", "code_inline"):
            parts.append(child.content)
        elif child.type in ("softbreak", "hardbreak"):
            parts.append(" ")
        elif child.type == "image":
            # Keep the alt text so the reader still sees what the image was.
            parts.append(child.content or child.attrGet("alt") or "")
    return "".join(parts).strip()


def _collect_table(tokens: list, start: int) -> tuple[list[list[str]], int]:
    """Collect table rows from the table_open token at start."""
    rows: list[list[str]] = []
    current: list[str] = []
    index = start + 1
    total = len(tokens)
    while index < total and tokens[index].type != "table_close":
        token = tokens[index]
        if token.type == "tr_open":
            current = []
        elif token.type == "tr_close":
            if current:
                rows.append(current)
        elif token.type == "inline":
            current.append(inline_text(token))
        index += 1
    return rows, index + 1


def parse_blocks(markdown: str) -> list[Block]:
    """Return the block outline of markdown, in document order."""
    parser = MarkdownIt("commonmark").enable("table")
    tokens = parser.parse(markdown)
    blocks: list[Block] = []

    # Frame per open list: [is_ordered, counter, pending_marker, list_id, start].
    # list_id is assigned when the list opens, so two lists in one document
    # never share a counter.
    list_stack: list[list] = []
    list_seq = 0

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
            blocks.append(Block("heading", inline_text(next_inline(index)), level=level))
            index += 3
            continue

        if kind == "paragraph_open":
            text = inline_text(next_inline(index)).strip()
            if text:
                if list_stack:
                    frame = list_stack[-1]
                    marker = frame[2]
                    frame[2] = ""
                    blocks.append(
                        Block(
                            "ordered" if frame[0] else "bullet",
                            text,
                            depth=max(1, len(list_stack)),
                            marker=marker,
                            list_id=frame[3] if frame[0] else 0,
                            start=frame[4] if frame[0] else 1,
                        )
                    )
                else:
                    blocks.append(Block("paragraph", text))
            index += 3
            continue

        if kind in ("fence", "code_block"):
            code = token.content.rstrip("\n")
            if code:
                blocks.append(Block("code", code))
            index += 1
            continue

        if kind == "table_open":
            rows, index = _collect_table(tokens, index)
            if rows:
                blocks.append(Block("table", rows=rows))
            continue

        if kind in ("bullet_list_open", "ordered_list_open"):
            ordered = kind == "ordered_list_open"
            start = 1
            if ordered:
                raw = token.attrGet("start")
                if raw:
                    start = int(raw)
            list_seq += 1
            list_stack.append([ordered, start - 1, "", list_seq, start])
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
            blocks.append(Block("rule"))

        index += 1

    return blocks


def document_title(blocks: list[Block]) -> str:
    """Return the first level-1 heading, or an empty string."""
    for block in blocks:
        if block.kind == "heading" and block.level == 1:
            return block.text
    return ""
