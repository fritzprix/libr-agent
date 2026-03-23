# PDF to Markdown Conversion Guide

PDF conversion focuses on maintaining visual layout and logical text blocks. This skill utilizes the `pymupdf` (fitz) library for deterministic text extraction.

## Prerequisites
Ensure the library is installed in the environment:
```bash
pip install pymupdf
```

## Usage
Run the `scripts/pdf_to_md.py` script to extract PDF content into Markdown. This script automatically distinguishes between headings and body text based on font size heuristics.

```bash
python scripts/pdf_to_md.py <input_file.pdf> <output_file.md>
```

## Features
- **Page Segmentation**: Explicitly marks pages with `## Page N` to preserve document flow.
- **Heading Detection**: Identifies text with a font size of 14 or larger as a heading (`###`) to maintain visual hierarchy.
- **Noise Reduction**: Filters out empty lines and redundant whitespace, focusing strictly on meaningful text blocks.
- **Korean Support**: Handles UTF-8 encoded Korean characters without corruption.
