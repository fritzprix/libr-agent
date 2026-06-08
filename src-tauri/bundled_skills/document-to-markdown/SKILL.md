---
name: document-to-markdown
description: "Extracts and converts various document formats (PDF, DOCX, PPTX, XLSX, HTML, Images, Audio, ZIP) into structured, machine-readable Markdown using Microsoft's MarkItDown. Use this skill when you need to extract the raw text content of a document without distortion, preserving its original structure (headings, tables, lists, slides) for analysis or RAG purposes."
---

# Document to Markdown

This skill provides a unified, deterministic workflow for converting various document formats into structured Markdown text without summarizing or altering the original content. It leverages Microsoft's **MarkItDown** library.

## Supported Formats
- **Documents**: PDF (.pdf), Word (.docx), PowerPoint (.pptx), Excel (.xlsx)
- **Data & Web**: HTML, CSV, JSON, XML
- **Media**: Images (EXIF metadata and OCR), Audio (Speech transcription)
- **Archives**: ZIP files

## Dependency Check

Before converting, verify markitdown is available:

```bash
python -c "import markitdown" 2>/dev/null || pip install "markitdown[all]"
```

On Windows PowerShell:

```powershell
python -c "import markitdown" 2>$null; if ($LASTEXITCODE -ne 0) { pip install "markitdown[all]" }
```

If installation fails, report the error and stop—do not attempt manual binary parsing.

## Conversion Workflow

Instead of format-specific scripts, use the unified markitdown CLI or Python API for all supported formats.

Read [references/conversion_guide.md](references/conversion_guide.md) for detailed usage instructions.

## Core Execution Principles

1. **Unified Tooling**: Always use markitdown for document extraction. Do not attempt to parse binary files manually or use legacy tools like Pandoc or PyMuPDF unless specifically requested.
2. **Preserve Original Text**: Do not allow the LLM to hallucinate, summarize, or alter the extracted text. Rely strictly on the parser output.
3. **Structure & Readability**: MarkItDown natively preserves headings, tables, and lists. Use its output directly for LLM ingestion.
