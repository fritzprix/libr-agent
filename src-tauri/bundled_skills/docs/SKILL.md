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

Always use the fixed command form. Give file paths only. Do not use inline code. Do not use pipes. The command must run in Windows PowerShell 5.1.

```powershell
python "<skill-base-dir>/scripts/convert.py" --in "<abs.md>" --to pdf --template report --out "<abs.pdf>"
```

Before first conversion or when troubleshooting dependencies, run the doctor command:

```powershell
python "<skill-base-dir>/scripts/convert.py" --doctor
```

## Output Contract & Verification

The script enforces an explicit output contract:
- On success, it prints the absolute output path, the size in bytes, and `ExitCode: 0`.
- On failure, it prints the error description, a single `NEXT:` line guiding the next action, and exits with a non-zero exit code.
- **No OK line, no success report**: Never claim conversion succeeded unless `convert.py` printed `ExitCode: 0` and the output file exists with non-zero size.
