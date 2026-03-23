# PPTX to Markdown Conversion Guide

PowerPoint files consist of disjointed text boxes (Shapes). This skill utilizes the `python-pptx` library to logically organize content by slide.

## Prerequisites
Ensure the library is installed:
```bash
pip install python-pptx
```

## Usage
Execute the `scripts/pptx_to_md.py` script to extract PowerPoint content. This script preserves the sequence of slide elements.

```bash
python scripts/pptx_to_md.py <input_file.pptx> <output_file.md>
```

## Features
- **Slide Segmentation**: Explicitly separates slides with `## Slide N`.
- **Title Extraction**: Identifies the primary title shape of each slide and formats it as a heading (`### Title`).
- **Logical Flow**: Iterates through all text boxes in each slide to extract body content in sequential order.
- **Korean Encoding**: Handles UTF-8 encoded Korean text without corruption.
