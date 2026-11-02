# Document to Markdown (Extraction)

Extracts content from existing documents (PDF, DOCX, PPTX, XLSX, HTML, images) into structured Markdown using MarkItDown.

## Command Form

```powershell
& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs-source-doc>" --to md --out "<abs-target.md>"
```

## Supported Source Formats

- Documents: PDF (`.pdf`), Microsoft Word (`.docx`), Microsoft PowerPoint (`.pptx`), Microsoft Excel (`.xlsx`)
- Web and Text: HTML (`.html`), Plain Text (`.txt`), CSV (`.csv`), JSON (`.json`)

## Rules
- The extracted Markdown preserves document hierarchy (headings, tables, lists) without summarizing or altering content.
- Always invoke `convert.py` directly; do not improvise custom document text extraction scripts.
