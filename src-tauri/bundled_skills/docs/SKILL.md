---
name: docs
description: "Use when the user needs to convert Markdown to PDF, PPTX, DOCX, or extract documents to Markdown. Uses fixed CLI commands with bundled scripts and ReportLab/MarkItDown. Do not write custom renderers."
---

# Docs

> [!CAUTION]
> **Do not write your own renderer.**
> Do not use fpdf, reportlab, or puppeteer code directly. Do not use `python -c` or `node -e`.
> If `convert.py` fails, report the error. Do not improvise.

This skill provides unified document conversion between Markdown and presentation/document formats (PDF, DOCX, PPTX) as well as extracting existing documents back into structured Markdown.

## Permissive Library Licenses

All tools and runtime dependencies used by this skill are permissively licensed:

- **ReportLab** (PDF generation): BSD-3-Clause
- **python-docx** (DOCX generation): MIT
- **python-pptx** (PPTX generation): MIT
- **MarkItDown** (document to Markdown extraction): MIT
- **matplotlib** (charts saved as PNG): Matplotlib License (BSD-style)
- **Noto Sans KR** (bundled Korean font): SIL OFL 1.1

## Routing by Direction

Select the conversion guide matching the desired direction:

All three output formats share the same templates, so one `--template` value
means the same document identity everywhere: `report`, `proposal`, `minutes`.

| Direction | Guide Reference | Outputs |
|-----------|-----------------|---------|
| Markdown -> PDF | [references/md-to-pdf.md](references/md-to-pdf.md) | `.pdf` |
| Markdown -> DOCX | [references/md-to-docx.md](references/md-to-docx.md) | `.docx` |
| Markdown -> PPTX | [references/md-to-pptx.md](references/md-to-pptx.md) | `.pptx` |
| Document -> Markdown | [references/to-md.md](references/to-md.md) | Text extraction via MarkItDown |

## Fixed Command Form

Before first conversion or when troubleshooting dependencies, run the doctor command:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File "<skill-base-dir>/scripts/doctor.ps1"
```
*(On Linux/macOS: `uv run --no-project --python 3.12 "<skill-base-dir>/scripts/convert.py" --doctor`)*

The doctor discovers a working Python environment, provisions a persistent virtualenv outside the managed skill directory (at `~/.libragent/skills/docs/venv`), verifies dependencies, and prints `Doctor: OK` along with the `VenvPython:` executable path. It installs the Word, PowerPoint and Excel converters too, so `--to md` works on `.docx`, `.pptx` and `.xlsx` files after a single doctor run.

Then run document conversion using the interpreter reported under `VenvPython:`. Always use the fixed command form. Give file paths only. Do not use inline code. Do not use pipes. The command must run in Windows PowerShell 5.1:

```powershell
& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs.md>" --to pdf --template report --out "<abs.pdf>"
```
*(On Linux/macOS: `"<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs.md>" --to pdf --template report --out "<abs.pdf>"`)*

## Images, Charts and Diagrams

- **No raw HTML.** HTML is removed from every output, with a warning. Use Markdown tables and text.
- **Images**: put `![caption](file.png)` alone in its own paragraph. Use a local PNG or JPEG. A relative path is read from the folder of the `.md` file. The image is scaled to the page or slide width. A missing file shows its caption instead, with a warning.
- **Charts**: write a small matplotlib script **file** and run it with the doctor's `VenvPython:`. Save the chart as PNG next to the `.md` file and reference it with `![caption](chart.png)`.
- **Mermaid** stays a code block labeled "Diagram (Mermaid)". To show a diagram as a picture, draw it as a PNG and use image syntax.
- Check the image references before converting: `& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/check.py" --md "<abs.md>"`. This reports missing or empty images.

## Output Contract & Verification

The script enforces an explicit output contract on every execution:
- On success, it prints the absolute output path, `ExitCode: 0`, and `OK`. Every format also prints its size as `Bytes: <n>`, plus one count for the format: `Pages:` for pdf, `Sections:` for docx, `Slides:` for pptx.
- Only the PDF count is a page count. Word and PowerPoint lay out pages when the file is opened, so they cannot be counted here. `Sections:` is always `1` because the writer creates one Word section: read it as a signature that the DOCX was written, not as a measurement. Use `--to pdf` when you need a page count.
- On failure (including syntax/argument errors, missing files, or pending sub-issue engines), it prints the error description, `ExitCode: <n>`, a single `NEXT:` line guiding the next action, and exits non-zero without printing `OK`.
- **No OK line, no success report**: Never claim conversion succeeded unless `convert.py` printed `ExitCode: 0` and `OK`, and the output file exists with non-zero size.
