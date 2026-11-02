# Markdown to PPTX

Converts a Markdown file to a PowerPoint presentation (.pptx) using python-pptx (MIT).
Slides are 16:9 widescreen.

## Command Form

```powershell
& "<venv-python-from-doctor>" "<skill-base-dir>/scripts/convert.py" --in "<abs-source.md>" --to pptx --template report --out "<abs-target.pptx>"
```

## Templates

Use the same three templates as every other output format:

| Template | Background | Accent | Agenda slide |
|----------|-----------|--------|--------------|
| `report` | light | blue | yes |
| `proposal` | light | green | yes |
| `minutes` | dark | grey | no |

A report and a proposal are read as arguments, so they open with an agenda. The
agenda lists the `##` section titles with numbers. It appears only when the deck
has at least two named sections, because a one-item agenda says nothing. Minutes
are read after the fact and skip the agenda.

## How the Markdown becomes slides

1. The first slide is a title slide built from the first `#` heading.
2. Each `#` or `##` heading starts a new slide. Content under the heading fills
   that slide.
3. A heading with nothing under it starts no slide, so a `#` used only to group
   `##` sections does not produce an empty slide.
4. A table always gets a slide of its own, because a table needs real cells.
5. A slide takes content until the estimated text height reaches the body
   area, and the rest continues on a `<heading> (continued)` slide instead of
   shrinking the text.

Inside a slide: `-` items become bullets, `1.` items keep their numbers, nested
items indent, and code fences appear as one monospaced paragraph per line.

## Content that needs more than one slide

Every slide is 16:9, so the body area has a fixed height. The writer estimates
each paragraph and table row from its text, font size and width before it places
it, and starts a new slide when the next item would pass the bottom edge:

- Long lists and long paragraphs continue on a `(continued)` slide. The cut point
  follows the height budget, not a fixed bullet count.
- A long code block counts every line, so a 40-line block spans several slides
  instead of running off one.
- A tall table splits into groups of rows. The header row is repeated at the top
  of each group, so the continuation slide still says what the columns mean.

The same estimate is used by `check.py`, so a deck that the writer accepted does
not then fail the check on a false overflow.

## Slide furniture

The first slide is the cover and shows the document title. Every slide after the
cover carries a footer: the deck title on the left and the slide number on the
right. Numbering therefore starts at 2, so the number matches the position in
the deck without repeating the cover.

## Rules
- One idea per `##` heading gives the cleanest deck. A long section without
  sub-headings becomes one dense slide plus continuation slides.
- Text is written in Malgun Gothic, which is not bundled. Windows has this font;
  macOS and Linux do not, so PowerPoint, Keynote and LibreOffice substitute
  another Hangul face there. The deck stays readable, but the face differs from
  the PDF, which draws its own bundled font. If the exact face matters, apply a
  theme font in PowerPoint before sharing the deck.
- Always run `convert.py`. Do not write a custom python-pptx script.
