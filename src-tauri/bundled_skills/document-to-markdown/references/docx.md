# DOCX to Markdown Conversion Guide

Word documents (`.docx`) are structured XML files. Use **Pandoc** CLI for accurate conversion instead of manual Python parsing.

## Prerequisites
Ensure Pandoc is installed on the system:
```bash
# Check installation
pandoc --version
```

## Commands (CLI)

### Basic Extraction
Execute the following command to convert the DOCX file directly to Markdown:
```bash
pandoc "input_file.docx" -t markdown -o "output_file.md"
```

### Extraction with Media
Use the `--extract-media` flag when images within the Word document must be preserved. This stores images in a specified folder and links them in the Markdown file.
```bash
pandoc "input_file.docx" -t markdown --extract-media=media_folder -o "output_file.md"
```

## Core Execution Principles
- **Maintain Original Content**: Do not summarize or alter text. Rely on Pandoc's mapping logic.
- **Support UTF-8**: Ensure the shell or environment handles UTF-8 for non-English character support.
