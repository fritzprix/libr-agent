#!/usr/bin/env python3
"""Flatten Markdown into the small block list the DOCX and PPTX writers need.

The PDF path walks markdown-it tokens straight into ReportLab flowables because
ReportLab stacks flowables itself. Word and PowerPoint do not: each writer has to
decide where a page or slide breaks, so it needs the document outline as data
before anything is drawn. This module produces that outline once.

Emoji are kept as-is. DOCX and PPTX store Unicode text and the reader's font draws
it, so unlike the PDF path there is no font to work around here.

The rules for content that no target can draw live here too, so all three formats
degrade it the same way: a paragraph holding only a local PNG or JPEG becomes an
image, a mermaid fence stays a labeled code block, and raw HTML is removed. Each
case prints one stderr warning per document.
"""
from __future__ import annotations

from dataclasses import dataclass, field
import pathlib
import re
import sys
import urllib.parse

from markdown_it import MarkdownIt

MERMAID_LABEL = "Diagram (Mermaid)"
IMAGE_FORMATS = ("PNG", "JPEG")
# "https:", "data:" and the like; a Windows drive letter ("C:\") is a local path.
_URL_SCHEME = re.compile(r"^[a-z][a-z0-9+.-]+:", re.IGNORECASE)


class ContentNotes:
    """Print each degraded-content warning once per document."""

    def __init__(self) -> None:
        self._seen: set[str] = set()

    def warn(self, key: str, message: str) -> None:
        if key not in self._seen:
            self._seen.add(key)
            print(f"Warning: {message}", file=sys.stderr)


def is_mermaid(token) -> bool:
    """Return True for a ```mermaid fence."""
    info = (token.info or "").strip().split()
    return token.type == "fence" and bool(info) and info[0].lower() == "mermaid"


def scan_degraded_content(tokens: list, notes: ContentNotes) -> None:
    """Warn once about mermaid fences and raw HTML anywhere in the token stream."""
    for token in tokens:
        if is_mermaid(token):
            notes.warn(
                "mermaid",
                f"mermaid diagrams are not rendered; the source is kept as a "
                f"'{MERMAID_LABEL}' code block. Draw a PNG and use ![caption](file.png) "
                "to show a picture.",
            )
        children = token.children or []
        if token.type == "html_block" or any(c.type == "html_inline" for c in children):
            notes.warn(
                "html",
                "raw HTML is not supported and was removed; use Markdown tables and text.",
            )


def image_only(token) -> tuple[str, str] | None:
    """Return (src, alt) when an inline token holds one image and nothing else."""
    if token is None:
        return None
    children = [
        child
        for child in token.children or []
        if not (child.type in ("text", "softbreak") and not child.content.strip())
    ]
    if len(children) == 1 and children[0].type == "image":
        image = children[0]
        return image.attrGet("src") or "", image.content or ""
    return None


def image_problem(src: str, base_dir: pathlib.Path) -> str:
    """Return why src is not a usable local image file, or "" when it is."""
    if _URL_SCHEME.match(src):
        return "remote and data URLs are not fetched"
    path = base_dir / urllib.parse.unquote(src)
    if not path.is_file():
        return "file not found"
    if path.stat().st_size == 0:
        return "file is empty"
    return ""


def resolve_image(
    src: str, base_dir: pathlib.Path, notes: ContentNotes
) -> tuple[pathlib.Path, tuple[int, int]] | None:
    """Return (path, (width_px, height_px)) for a local PNG or JPEG, else warn."""
    path = base_dir / urllib.parse.unquote(src)
    reason = image_problem(src, base_dir)
    if not reason:
        try:
            # Pillow ships with reportlab, so every target can validate this way.
            from PIL import Image

            with Image.open(path) as picture:
                image_format, size = picture.format, picture.size
                picture.verify()
            if image_format in IMAGE_FORMATS and size[0] > 0 and size[1] > 0:
                return path, size
            reason = f"format {image_format} is not PNG or JPEG"
        except Exception as exc:
            reason = f"not a readable image ({exc})"
    notes.warn(
        f"image:{src}", f"image '{src}' was not embedded ({reason}); showing its alt text."
    )
    return None


def fit_size(
    size: tuple[int, int], max_width: float, max_height: float
) -> tuple[float, float]:
    """Scale size to fill max_width without passing max_height, keeping its aspect ratio."""
    width, height = size
    scale = min(max_width / width, max_height / height)
    return width * scale, height * scale


@dataclass
class Block:
    """One block-level element of a Markdown document."""

    kind: str  # heading | paragraph | bullet | ordered | table | code | rule | image
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
    # A code block may carry a label (MERMAID_LABEL). An image block keeps its
    # alt text in `text` and the resolved file and pixel size here.
    label: str = ""
    image_path: str = ""
    image_size: tuple[int, int] = (0, 0)


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


def parse_blocks(
    markdown: str,
    base_dir: pathlib.Path | None = None,
    notes: ContentNotes | None = None,
) -> list[Block]:
    """Return the block outline of markdown, in document order.

    Image paths resolve from base_dir, the folder of the input Markdown file.
    """
    parser = MarkdownIt("commonmark").enable("table")
    tokens = parser.parse(markdown)
    base_dir = base_dir or pathlib.Path.cwd()
    notes = notes or ContentNotes()
    scan_degraded_content(tokens, notes)
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
                    image = image_only(next_inline(index))
                    resolved = resolve_image(image[0], base_dir, notes) if image else None
                    if resolved:
                        path, size = resolved
                        blocks.append(
                            Block("image", text, image_path=str(path), image_size=size)
                        )
                    else:
                        blocks.append(Block("paragraph", text))
            index += 3
            continue

        if kind in ("fence", "code_block"):
            code = token.content.rstrip("\n")
            if code:
                label = MERMAID_LABEL if is_mermaid(token) else ""
                blocks.append(Block("code", code, label=label))
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
