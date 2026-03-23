---
name: document-to-markdown
description: "Extracts and converts various document formats (PDF, DOCX, PPTX, HTML) into structured, machine-readable Markdown. Use this skill when you need to extract the raw text content of a document without distortion, preserving its original structure (headings, paragraphs, slides) for analysis or RAG purposes."
---

# Document to Markdown

This skill provides deterministic workflows for converting various document formats into structured Markdown text without summarizing or altering the original content.

## Supported Formats & Required Tools

Use the following tools and libraries based on the document extension:
- **PDF (`.pdf`)**: Use `pymupdf` via provided Python scripts.
- **Word (`.docx`)**: Use `pandoc` (CLI).
- **PowerPoint (`.pptx`)**: Use `python-pptx` via provided Python scripts.

---

## Conversion Workflows

Execute the specific conversion workflows by reading the reference files below. Do not attempt to parse binary files directly; always rely on these deterministic scripts and tools.

- **PDF Conversion**: Read [references/pdf.md](references/pdf.md) and execute `scripts/pdf_to_md.py`.
- **DOCX Conversion**: Read [references/docx.md](references/docx.md) for Pandoc commands.
- **PPTX Conversion**: Read [references/pptx.md](references/pptx.md) and execute `scripts/pptx_to_md.py`.

## Core Execution Principles

1. **Enforce Encoding**: Always explicitly set `utf-8` encoding in scripts and commands to prevent character corruption (especially for Korean and special characters).
2. **Preserve Original Text**: Do not allow the LLM to hallucinate, summarize, or alter the extracted text. Rely strictly on the script/parser output.
3. **Maintain Structure & Readability**: Insert markdown horizontal rules (`---`) when transitioning between pages or slides to help LLMs and humans easily identify document segments.