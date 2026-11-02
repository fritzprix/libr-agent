# Markdown to PPTX

Converts structured Markdown files to presentation slides using python-pptx.

## Command Form

```powershell
python "<skill-base-dir>/scripts/convert.py" --in "<abs-source.md>" --to pptx --template slides --out "<abs-target.pptx>"
```

## Available Templates

- `slides`: 16:9 widescreen presentation with clean title and content layouts.
- `dark`: High-contrast dark theme suitable for technical briefs.
- `minimal`: Clean light theme with minimal decoration.

## Rules
- Each top-level section (`#` or `---` slide divider) starts a new slide.
- Second-level headings (`##`) form card or column headers.
- Always invoke `convert.py` directly; do not write custom python-pptx generation scripts.
