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
- **Noto Sans KR** (bundled Korean font): SIL OFL 1.1

## Routing by Direction

Select the conversion guide matching the desired direction:

| Direction | Guide Reference | Supported Outputs / Templates |
|-----------|-----------------|-------------------------------|
| Markdown -> PDF | [references/md-to-pdf.md](references/md-to-pdf.md) | `report`, `brief`, `memo` |
| Markdown -> PPTX | [references/md-to-pptx.md](references/md-to-pptx.md) | `slides`, `dark`, `minimal` |
| Markdown -> DOCX | [references/md-to-docx.md](references/md-to-docx.md) | `standard`, `clean` |
| Document -> Markdown | [references/to-md.md](references/to-md.md) | Text extraction via MarkItDown |

## Fixed Command Form

Before first conversion or when troubleshooting dependencies, run the doctor command:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File "<skill-base-dir>/scripts/doctor.ps1"
```
*(On Linux/macOS: `uv run --no-project --python 3.12 "<skill-base-dir>/scripts/convert.py" --doctor`)*

The doctor discovers a working Python environment, provisions a persistent virtualenv outside the managed skill directory (at `~/.libragent/skills/docs/venv`), verifies dependencies, and prints `Doctor: OK` along with the `VenvPython:` executable path.

Then run document conversion using the interpreter reported under `VenvPython:`. Always use the fixed command form. Give file paths only. Do not use inline code. Do not use pipes. The command must run in Windows PowerShell 5.1:

```powershell
& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs.md>" --to pdf --template report --out "<abs.pdf>"
```
*(On Linux/macOS: `"<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs.md>" --to pdf --template report --out "<abs.pdf>"`)*

## Output Contract & Verification

The script enforces an explicit output contract on every execution:
- On success, it prints the absolute output path, the size in bytes, `ExitCode: 0`, and `OK`.
- On failure (including syntax/argument errors, missing files, or pending sub-issue engines), it prints the error description, `ExitCode: <n>`, a single `NEXT:` line guiding the next action, and exits non-zero without printing `OK`.
- **No OK line, no success report**: Never claim conversion succeeded unless `convert.py` printed `ExitCode: 0` and `OK`, and the output file exists with non-zero size.
