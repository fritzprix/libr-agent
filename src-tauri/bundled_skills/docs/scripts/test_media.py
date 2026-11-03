#!/usr/bin/env python3
"""Media test for the docs skill (#2530), shared by every platform runner.

Builds a matplotlib PNG chart and a document with that chart, a missing image, a
mermaid fence and raw HTML, converts it to pdf, docx and pptx, and prints one
NAME=1/0 line per check. Exits 1 when any check fails.

Usage: <docs-venv-python> test_media.py <empty-work-dir> <path/to/convert.py>
Runners: test_convert.ps1 (Windows) and test_convert.sh (Linux/macOS).
"""
import pathlib
import subprocess
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

work, convert = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
check = convert.parent / "check.py"
failed = []

fig, ax = plt.subplots(figsize=(6, 3), dpi=100)
ax.bar(["A", "B", "C"], [3, 5, 2])
fig.savefig(work / "chart.png")
plt.close(fig)

source = work / "doc.md"
source.write_text(
    "# Media\n\n## Chart\n\n![Quarterly chart](chart.png)\n\n"
    "## Missing\n\n![Missing picture](nope.png)\n\n"
    "## Diagram\n\n```mermaid\ngraph TD; Start-->Finish\n```\n\n"
    "## HTML\n\n<div class=\"box\">RAWBLOCK</div>\n\nText with <span>inner</span> words.\n",
    encoding="utf-8",
)


def report(name, ok):
    print("%s=%d" % (name, 1 if ok else 0))
    if not ok:
        failed.append(name)


for target in ("pdf", "docx", "pptx"):
    out = work / ("doc." + target)
    proc = subprocess.run(
        [sys.executable, str(convert), "--in", str(source), "--to", target,
         "--template", "report", "--out", str(out)],
        capture_output=True, text=True, encoding="utf-8",
    )
    err = proc.stderr
    report(target + "_EXIT0", proc.returncode == 0)
    report(target + "_WARNINGS",
           "nope.png" in err and "mermaid" in err and "raw HTML" in err)
    if proc.returncode != 0:
        print(proc.stdout + err)
        continue
    if target == "pdf":
        import pdfplumber

        with pdfplumber.open(out) as pdf:
            text = "".join(page.extract_text() or "" for page in pdf.pages)
            boxes = [(img, page) for page in pdf.pages for img in page.images]
            inside = all(
                img["x0"] >= 0 and img["x1"] <= page.width
                and img["top"] >= 0 and img["bottom"] <= page.height
                for img, page in boxes
            )
            count = len(boxes)
    elif target == "docx":
        import docx

        document = docx.Document(str(out))
        section = document.sections[0]
        limit = section.page_width - section.left_margin - section.right_margin
        count = len(document.inline_shapes)
        inside = all(shape.width <= limit + 1 for shape in document.inline_shapes)
        text = "\n".join(p.text for p in document.paragraphs)
    else:
        from pptx import Presentation

        deck = Presentation(str(out))
        pictures = [s for slide in deck.slides for s in slide.shapes if s.shape_type == 13]
        count = len(pictures)
        inside = all(
            s.left + s.width <= deck.slide_width and s.top + s.height <= deck.slide_height
            for s in pictures
        )
        text = "\n".join(
            s.text_frame.text for slide in deck.slides for s in slide.shapes if s.has_text_frame
        )
    report(target + "_ONE_IMAGE_INSIDE", count == 1 and inside)
    report(target + "_MERMAID", "Diagram (Mermaid)" in text and "Start-->Finish" in text)
    report(target + "_NO_HTML", "RAWBLOCK" not in text and "<span>" not in text and "<div" not in text)
    report(target + "_ALT_TEXT", "Missing picture" in text)

proc = subprocess.run([sys.executable, str(check), "--md", str(source)],
                      capture_output=True, text=True, encoding="utf-8")
report("CHECK_MD_REPORTS_MISSING", proc.returncode != 0 and "nope.png" in proc.stderr)

sys.exit(1 if failed else 0)
