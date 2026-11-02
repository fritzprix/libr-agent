# Markdown to DOCX

Converts structured Markdown files to Microsoft Word (.docx) documents using python-docx.

## Command Form

```powershell
& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs-source.md>" --to docx --template standard --out "<abs-target.docx>"
```

## Available Templates

- `standard`: Formatted document with standard margins, heading styles, tables, and callout boxes.
- `clean`: Plain formatted document with minimal styling for easy editing.

## Rules
- Heading levels (`#`, `##`, `###`) map to Word heading styles.
- Tables are formatted with header shading and gridlines.
- Always invoke `convert.py` directly; do not write custom python-docx scripts.
