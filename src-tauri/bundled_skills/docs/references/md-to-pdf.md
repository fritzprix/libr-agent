# Markdown to PDF

Converts structured Markdown files to professional PDF documents using ReportLab and the bundled Noto Sans KR font.

## Command Form

```powershell
& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs-source.md>" --to pdf --template report --out "<abs-target.pdf>"
```

## Available Templates

- `report`: Standard document format with cover, table of contents, headers, footers, and page numbers.
- `brief`: Compact executive summary without page break headers.
- `memo`: Minimalist internal memo format.

## Rules
- All text rendering uses the bundled Noto Sans KR font (`fonts/NotoSansKR-Regular.ttf`) to support Korean and multi-language glyphs without system font dependencies.
- Tables, headers, lists, and code blocks are styled according to the selected template.
- Do not attempt custom ReportLab scripting; always invoke `convert.py`.
