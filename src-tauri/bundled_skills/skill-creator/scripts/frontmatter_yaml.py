#!/usr/bin/env python3
"""SKILL.md frontmatter parsing that works without PyYAML.

Uses PyYAML when it is installed. Otherwise a small built-in parser handles the
subset SKILL.md frontmatter uses: ``key: value`` scalars (plain, single- or
double-quoted), ``|`` / ``>`` block scalars, flow lists, and one nested mapping
or ``- item`` list. Constructs outside that subset raise ``YAMLError`` instead of
being guessed at, mirroring the Rust skill scanner, which skips invalid skills.
"""

from __future__ import annotations

import re
from typing import Any

try:
    import yaml as _yaml
except ImportError:  # pragma: no cover - exercised via _fallback_load in tests
    _yaml = None

KEY_PATTERN = re.compile(r"^([A-Za-z0-9_][A-Za-z0-9_.-]*)\s*:(?:\s+(.*)|)$")
PLAIN_FORBIDDEN_START = set("@`%&*!|>{")


class FallbackYAMLError(ValueError):
    """Frontmatter is invalid or uses YAML the fallback parser does not support."""


def _indent(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def _strip_comment(text: str) -> str:
    return re.sub(r"\s+#.*$", "", text).rstrip()


def _unquote(text: str) -> str:
    quote = text[0]
    if len(text) < 2 or text[-1] != quote:
        raise FallbackYAMLError(f"Unterminated quoted string: {text}")
    body = text[1:-1]
    if quote == "'":
        if re.search(r"(?<!')'(?!')", body):
            raise FallbackYAMLError(f"Unescaped quote in single-quoted string: {text}")
        return body.replace("''", "'")
    if re.search(r'(?<!\\)"', body):
        raise FallbackYAMLError(f"Unescaped quote in double-quoted string: {text}")
    escapes = {"n": "\n", "t": "\t", '"': '"', "\\": "\\", "/": "/"}
    return re.sub(r"\\(.)", lambda m: escapes.get(m.group(1), m.group(0)), body)


def _resolve_plain(text: str) -> Any:
    if text in ("", "~", "null", "Null", "NULL"):
        return None
    if text in ("true", "True", "TRUE"):
        return True
    if text in ("false", "False", "FALSE"):
        return False
    if re.fullmatch(r"[-+]?\d+", text):
        return int(text)
    if re.fullmatch(r"[-+]?(\d+\.\d*|\.\d+)", text):
        return float(text)
    return text


def _scalar(text: str) -> Any:
    text = text.strip()
    if text[:1] in ('"', "'"):
        return _unquote(text)
    if text.startswith("["):
        if not text.endswith("]"):
            raise FallbackYAMLError(f"Unterminated flow list: {text}")
        inner = text[1:-1].strip()
        return [_scalar(part) for part in inner.split(",")] if inner else []
    if text[:1] in PLAIN_FORBIDDEN_START:
        raise FallbackYAMLError(f"Unsupported or invalid value: {text}")
    if ": " in text or text.endswith(":"):
        raise FallbackYAMLError(
            f"Plain value contains ': ' (quote it): {text[:60]}"
        )
    return _resolve_plain(_strip_comment(text))


def _block_scalar(style: str, lines: list[str]) -> str:
    dedent = min((_indent(line) for line in lines if line.strip()), default=0)
    body = [line[dedent:] if line.strip() else "" for line in lines]
    if style.startswith("|"):
        text = "\n".join(body)
    else:
        paragraphs: list[list[str]] = [[]]
        for line in body:
            if line:
                paragraphs[-1].append(line)
            elif paragraphs[-1]:
                paragraphs.append([])
        text = "\n".join(" ".join(p) for p in paragraphs if p)
    text = text.rstrip("\n")
    return text if "-" in style else text + "\n"


def _parse_mapping(lines: list[str], base: int, eof_open: bool) -> dict[str, Any]:
    result: dict[str, Any] = {}
    i = 0
    while i < len(lines):
        line = lines[i]
        if not line.strip() or line.lstrip().startswith("#"):
            i += 1
            continue
        if _indent(line) != base:
            raise FallbackYAMLError(f"Unexpected indentation: {line.strip()[:60]}")
        match = KEY_PATTERN.match(line.strip())
        if not match:
            raise FallbackYAMLError(f"Expected 'key: value': {line.strip()[:60]}")
        key, raw = match.group(1), (match.group(2) or "").strip()
        if key in result:
            raise FallbackYAMLError(f"Duplicate key: {key}")
        i += 1
        children: list[str] = []
        while i < len(lines) and (not lines[i].strip() or _indent(lines[i]) > base):
            children.append(lines[i])
            i += 1
        while children and not children[-1].strip():
            children.pop()

        if re.fullmatch(r"[|>][+-]?", raw):
            value = _block_scalar(raw, children)
            if eof_open and i >= len(lines) and raw.startswith(("|", ">")) and "+" not in raw:
                value = value.rstrip("\n")  # no final line break to clip at end of input
            result[key] = value
        elif raw:
            if children and raw[:1] in ('"', "'", "["):
                raise FallbackYAMLError(f"Multi-line flow/quoted value is unsupported: {key}")
            if children:  # plain scalar folded over continuation lines
                raw = " ".join([raw, *(c.strip() for c in children if c.strip())])
            result[key] = _scalar(raw)
        elif not children:
            result[key] = None
        else:
            result[key] = _parse_nested(children, eof_open and i >= len(lines))
    return result


def _parse_nested(children: list[str], eof_open: bool) -> Any:
    content = [line for line in children if line.strip()]
    inner = min(_indent(line) for line in content)
    if content[0].lstrip().startswith("- "):
        items: list[Any] = []
        for line in content:
            stripped = line.strip()
            if _indent(line) != inner or not stripped.startswith("- "):
                raise FallbackYAMLError(f"Unsupported list item: {stripped[:60]}")
            items.append(_scalar(stripped[2:]))
        return items
    return _parse_mapping(children, inner, eof_open)


def _fallback_load(text: str) -> Any:
    lines = text.replace("\r\n", "\n").split("\n")
    if all(not line.strip() or line.lstrip().startswith("#") for line in lines):
        return None
    return _parse_mapping(lines, 0, not text.endswith("\n"))


if _yaml is not None:
    safe_load = _yaml.safe_load
    YAMLError = _yaml.YAMLError
else:
    safe_load = _fallback_load
    YAMLError = FallbackYAMLError
