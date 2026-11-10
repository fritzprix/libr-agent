# Markdown to PDF

Converts structured Markdown files to professional PDF documents using ReportLab and the bundled Noto Sans KR font.

## Command Form

```powershell
& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs-source.md>" --to pdf --template report --out "<abs-target.pdf>"
```

## Available Templates

- `report`: Standard document format with cover, table of contents, a header and footer on every body page, and page numbers.
- `proposal`: Same structure as `report` with a narrower margin and a green accent, suited to proposals and plans.
- `minutes`: Single-page meeting format with no cover and no table of contents. Starts directly with the content.

The cover page carries the title only. Body pages repeat the document title in the header, with the title and page number in the footer.

The default template is `report`. An unknown template name fails the command instead of guessing.

## Rules

- All text rendering uses the bundled Noto Sans KR font (`fonts/NotoSansKR-Regular.ttf`) to support Korean and multi-language glyphs without system font dependencies.
- Tables, headers, lists, and code blocks are styled according to the selected template.
- Symbols are checked one character at a time against the bundled font, never by Unicode range. Characters the font can draw stay as they are, including `★ ☆ ♥ ♡ ☎ ☞ ♨ ✓ ⚠`. Characters it cannot draw become plain markers: `✅` becomes `[OK]`, `⭐` becomes `[*]`, `⭕` becomes `[o]`. Any other undrawable character becomes `[?]` with a warning on stderr, so a gap is never silent.
- Wide tables get equal column widths, and long cell text wraps inside its column. Choose `minutes` or split the table if a table has more than eight columns.
- Long code lines wrap inside the block instead of running past the right margin. Indentation on continuation lines is kept.
- Ordered lists honour the first number. A list written as `3.` continues from three.
- A Markdown file with no body text fails the command instead of producing an empty document.
- Do not attempt custom ReportLab scripting; always invoke `convert.py`.

## Verification

`convert.py --to pdf` runs `scripts/check.py` automatically after rendering. `check.py` reads text positions with pdfplumber and rejects these defects:

- a 0-byte output file
- a table row that overlaps the next row
- cell text that reaches outside its cell
- text drawn with a fallback font, which means a glyph is missing
- a character the bundled font cannot draw, which would print as a blank box

On success it prints `OK <abs path> <pages>`. To re-verify an existing PDF on its own:

```powershell
& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/check.py" --pdf "<abs-target.pdf>"
```
