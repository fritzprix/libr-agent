# MarkItDown Conversion Guide

MarkItDown is a lightweight Python utility developed by Microsoft for converting various files to Markdown. It focuses on preserving important document structure and content (headings, lists, tables, links, etc.) specifically for LLMs and text analysis pipelines.

## Prerequisites

Follow the **Dependency check** in `SKILL.md` first (interpreter → `python -m pip` → fallback CLIs). When Python is available:

```bash
PY="$(command -v python3 || command -v python)"
"$PY" -m pip install "markitdown[all]"
```

Then prefer:

```bash
"$PY" -m markitdown <input_file_path> -o <output_file_path.md>
```

## Command-line usage (recommended)

Prefer the module form with the same interpreter used in the dependency check:

```bash
PY="$(command -v python3 || command -v python)"
"$PY" -m markitdown <input_file_path> -o <output_file_path.md>
```

If the `markitdown` console script is on `PATH`, this is equivalent:

```bash
markitdown <input_file_path> -o <output_file_path.md>
```

**Examples:**

```bash
"$PY" -m markitdown report.pdf -o report.md
"$PY" -m markitdown presentation.pptx -o presentation.md
"$PY" -m markitdown spreadsheet.xlsx -o spreadsheet.md
```

## Python API usage

If you need to process the output within a Python script before saving:

```python
from markitdown import MarkItDown

md = MarkItDown()
result = md.convert("input_file.docx")

# Save to file
with open("output_file.md", "w", encoding="utf-8") as f:
    f.write(result.text_content)
```

## Features and capabilities

- **PDFs**: Extracts text while preserving paragraphs and layout flow.
- **Word (.docx) & PowerPoint (.pptx)**: Preserves headings, tables, bullet points, and slide text.
- **Excel (.xlsx) & CSV**: Converts tabular data into Markdown tables.
- **Images & Audio**: Extracts EXIF data, performs OCR on images, and transcribes audio files (requires the `[all]` installation flag).
- **ZIP Archives**: Extracts and converts supported files within the archive.
