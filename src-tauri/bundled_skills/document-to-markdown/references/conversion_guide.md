# MarkItDown Conversion Guide

MarkItDown is a lightweight Python utility developed by Microsoft for converting various files to Markdown. It focuses on preserving important document structure and content (headings, lists, tables, links, etc.) specifically for LLMs and text analysis pipelines.

## Prerequisites
Install the package with all optional dependencies (for OCR, audio, etc.):
```bash
pip install "markitdown[all]"
```

## Command-Line Usage (Recommended)

The simplest way to convert any supported file to Markdown is using the CLI:

```bash
markitdown <input_file_path> -o <output_file_path.md>
```

**Example:**
```bash
markitdown report.pdf -o report.md
markitdown presentation.pptx -o presentation.md
markitdown spreadsheet.xlsx -o spreadsheet.md
```

## Python API Usage

If you need to process the output within a Python script before saving:

```python
from markitdown import MarkItDown

md = MarkItDown()
result = md.convert("input_file.docx")

# Save to file
with open("output_file.md", "w", encoding="utf-8") as f:
    f.write(result.text_content)
```

## Features and Capabilities
- **PDFs**: Extracts text while preserving paragraphs and layout flow.
- **Word (.docx) & PowerPoint (.pptx)**: Preserves headings, tables, bullet points, and slide text.
- **Excel (.xlsx) & CSV**: Converts tabular data into Markdown tables.
- **Images & Audio**: Extracts EXIF data, performs OCR on images, and transcribes audio files (requires the [all] installation flag).
- **ZIP Archives**: Extracts and converts supported files within the archive.
