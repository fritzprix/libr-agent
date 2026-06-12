---
name: to-md
description: "Convert files (PDF, DOCX, PPTX, XLSX, HTML, images, audio, ZIP) to structured Markdown via MarkItDown. Use for text extraction, RAG ingestion, or format normalization. NOT for document editing — use docx or pptx skills instead."
---

# To Markdown

This skill defines the unified policy for converting files to structured Markdown without summarizing or altering the original content. It uses Microsoft's **MarkItDown** Python package.

## Skill routing

| Task | Use this skill | NOT |
|------|----------------|-----|
| Single-file text extraction | **to-md** | docx, pptx, workspace-indexer |
| Workspace batch conversion + index.md | **workspace-indexer** | to-md directly |
| DOCX tracked-changes review | **docx** (pandoc) | to-md |
| DOCX/PPTX creation or editing | **docx** / **pptx** | to-md |

## Supported formats

- **Documents**: PDF (.pdf), Word (.docx), PowerPoint (.pptx), Excel (.xlsx)
- **Data & Web**: HTML, CSV, JSON, XML
- **Media**: Images (EXIF metadata and OCR), Audio (speech transcription)
- **Archives**: ZIP files

## Dependency check

Before converting, verify the MarkItDown **pip package** is available:

```bash
python -c "import markitdown" 2>/dev/null || pip install "markitdown[all]"
```

On Windows PowerShell:

```powershell
python -c "import markitdown" 2>$null; if ($LASTEXITCODE -ne 0) { pip install "markitdown[all]" }
```

If installation fails, report the error and stop — do not attempt manual binary parsing.

## Conversion workflow

Use the MarkItDown CLI or Python API for all supported formats.

Read [references/conversion_guide.md](references/conversion_guide.md) for detailed usage instructions.

## Core execution principles

1. **Unified tooling**: Always use MarkItDown for file-to-Markdown extraction. Do not parse binary files manually or use legacy tools (Pandoc, PyMuPDF, python-docx, python-pptx, openpyxl) unless a specialized skill explicitly requires it (e.g. DOCX tracked changes in **docx**).
2. **Preserve original text**: Do not hallucinate, summarize, or alter extracted text. Rely strictly on parser output.
3. **Structure & readability**: MarkItDown preserves headings, tables, and lists. Use its output directly for LLM ingestion.
