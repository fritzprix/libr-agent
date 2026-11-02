# Markdown to PPTX

Converts a Markdown file to a PowerPoint presentation (.pptx) using python-pptx (MIT).
Slides are 16:9 widescreen.

## Command Form

```powershell
& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs-source.md>" --to pptx --template report --out "<abs-target.pptx>"
```

## Templates

Use the same three templates as every other output format:

| Template | Background | Accent |
|----------|-----------|--------|
| `report` | light | blue |
| `proposal` | light | green |
| `minutes` | dark | grey |

## How the Markdown becomes slides

1. The first slide is a title slide built from the first `#` heading.
2. Each `#` or `##` heading starts a new slide. Content under the heading fills
   that slide.
3. A heading with nothing under it starts no slide, so a `#` used only to group
   `##` sections does not produce an empty slide.
4. A table always gets a slide of its own, because a table needs real cells.
5. When a slide holds more than 8 bullets, the rest continues on a
   `<heading> (continued)` slide instead of shrinking the text.

Inside a slide: `-` items become bullets, `1.` items keep their numbers, nested
items indent, and code fences appear as monospaced lines.

## Rules
- One idea per `##` heading gives the cleanest deck. A long section without
  sub-headings becomes one dense slide plus continuation slides.
- Always run `convert.py`. Do not write a custom python-pptx script.
