# Markdown to DOCX

Converts a Markdown file to a Microsoft Word document (.docx) using python-docx (MIT).

## Command Form

```powershell
& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs-source.md>" --to docx --template report --out "<abs-target.docx>"
```

## Templates

Use the same three templates as every other output format:

| Template | Cover | Contents page | Accent |
|----------|-------|---------------|--------|
| `report` | yes | yes | blue |
| `proposal` | yes | yes | green |
| `minutes` | no | no | grey |

## How the Markdown maps to Word

| Markdown | Word |
|----------|------|
| `#` .. `####` | Heading 1 .. Heading 4 (real Word styles, so the navigation pane works) |
| Paragraph | Body text |
| `-` item | List Bullet, nested items indent one level per depth |
| `1.` item | List Number; each separate list restarts, and `5.` starts at 5 |
| Table | Word table with a shaded header row and gridlines |
| Code fence | Monospaced paragraph on a light shaded background |
| `---` | Horizontal rule |

The document also gets a header with the title and a footer with the title and a
Word page-number field, so the footer number is correct after editing.

## Numbered lists keep their own counters

Each Markdown ordered list becomes its own Word list definition, so a second
list starts again at 1 instead of continuing the first. A list that starts at a
number other than 1 keeps that number. Ordered lists nest up to three levels,
and each level uses its own counter.

## Word counts pages itself

Word decides pagination when the file is opened, so no page count is printed.
`convert.py` prints `Sections:` instead. The writer creates exactly one Word
section, so this value is always `1` and means "one continuous section". Read it
as a signature that the file was written, not as a measurement: check the
appearance in Word for real layout. Use `--to pdf` when you need a page count.

## Rules
- Heading levels map to Word Heading styles. Do not expect fixed page breaks;
  Word reflows the document when it is opened.
- Korean text is written into the Word east-Asian font slot and names Malgun
  Gothic. That font is not bundled. Windows has it; macOS and Linux do not, so
  Word, Pages and LibreOffice substitute another Hangul face there. The text
  stays readable, but it looks different from the PDF, which draws its own
  bundled font. Set the font in Word after opening if the exact face matters.
- Always run `convert.py`. Do not write a custom python-docx script.
