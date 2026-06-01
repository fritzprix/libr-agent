#!/usr/bin/env python3
"""
Workspace Binary Document Converter
====================================
Binary 형식 파일(pptx, pdf, docx, xlsx)을 텍스트/마크다운으로 변환합니다.

Usage:
    python convert_binary_docs.py [--root ROOT_DIR] [--out OUT_DIR] [--formats pdf pptx docx xlsx]

Options:
    --root      탐색할 루트 디렉터리 (기본값: 현재 디렉터리)
    --out       변환된 파일을 저장할 디렉터리 (기본값: 원본 파일과 동일한 위치)
    --formats   변환할 파일 형식 목록 (기본값: pdf pptx docx xlsx)
    --overwrite 기존 변환 파일 덮어쓰기 (기본값: False)
    --dry-run   실제 변환 없이 대상 파일 목록만 출력

의존성:
    pip install pymupdf python-docx python-pptx openpyxl

Examples:
    # 전체 워크스페이스 변환
    python convert_binary_docs.py --root . --out ./converted

    # PDF만 변환
    python convert_binary_docs.py --root . --formats pdf

    # dry-run으로 대상 파일 확인
    python convert_binary_docs.py --root . --dry-run
"""

import argparse
import sys
from pathlib import Path

SKIP_DIRS = {".git", "__pycache__", "node_modules", ".github", "venv", ".venv"}


def find_binary_files(root: Path, formats: list[str]) -> list[Path]:
    """지정된 형식의 binary 파일을 재귀 탐색합니다."""
    exts = {f".{fmt.lstrip('.')}" for fmt in formats}
    result = []
    for p in root.rglob("*"):
        if any(part in SKIP_DIRS for part in p.parts):
            continue
        if p.suffix.lower() in exts and p.is_file():
            result.append(p)
    return sorted(result)


def convert_pdf(src: Path) -> str:
    """PDF → Markdown 텍스트 변환."""
    try:
        import fitz  # pymupdf
    except ImportError:
        return f"[ERROR] pymupdf not installed. Run: pip install pymupdf\nSource: {src}"

    doc = fitz.open(str(src))
    pages = []
    for i, page in enumerate(doc, 1):
        text = page.get_text("text").strip()
        if text:
            pages.append(f"## Page {i}\n\n{text}")
    doc.close()
    return "\n\n---\n\n".join(pages) if pages else "(no text content)"


def convert_docx(src: Path) -> str:
    """DOCX → Markdown 텍스트 변환."""
    try:
        from docx import Document
    except ImportError:
        return f"[ERROR] python-docx not installed. Run: pip install python-docx\nSource: {src}"

    doc = Document(str(src))
    lines = []
    for para in doc.paragraphs:
        style = para.style.name.lower()
        text = para.text.strip()
        if not text:
            lines.append("")
            continue
        if style.startswith("heading 1"):
            lines.append(f"# {text}")
        elif style.startswith("heading 2"):
            lines.append(f"## {text}")
        elif style.startswith("heading 3"):
            lines.append(f"### {text}")
        else:
            lines.append(text)

    # 테이블
    for table in doc.tables:
        if not table.rows:
            continue
        header = [cell.text.strip() for cell in table.rows[0].cells]
        lines.append("\n| " + " | ".join(header) + " |")
        lines.append("| " + " | ".join(["---"] * len(header)) + " |")
        for row in table.rows[1:]:
            cells = [cell.text.strip() for cell in row.cells]
            lines.append("| " + " | ".join(cells) + " |")
        lines.append("")

    return "\n".join(lines)


def convert_pptx(src: Path) -> str:
    """PPTX → Markdown 텍스트 변환."""
    try:
        from pptx import Presentation
    except ImportError:
        return f"[ERROR] python-pptx not installed. Run: pip install python-pptx\nSource: {src}"

    prs = Presentation(str(src))
    slides = []
    for i, slide in enumerate(prs.slides, 1):
        texts = []
        for shape in slide.shapes:
            if shape.has_text_frame:
                for para in shape.text_frame.paragraphs:
                    line = para.text.strip()
                    if line:
                        texts.append(line)
        if texts:
            slides.append(f"## Slide {i}\n\n" + "\n\n".join(texts))
    return "\n\n---\n\n".join(slides) if slides else "(no text content)"


def convert_xlsx(src: Path) -> str:
    """XLSX → Markdown 테이블 변환."""
    try:
        import openpyxl
    except ImportError:
        return f"[ERROR] openpyxl not installed. Run: pip install openpyxl\nSource: {src}"

    wb = openpyxl.load_workbook(str(src), read_only=True, data_only=True)
    sections = []
    for sheet_name in wb.sheetnames:
        ws = wb[sheet_name]
        rows = list(ws.iter_rows(values_only=True))
        # 빈 시트 스킵
        non_empty = [r for r in rows if any(c is not None for c in r)]
        if not non_empty:
            continue
        lines = [f"## Sheet: {sheet_name}\n"]
        for idx, row in enumerate(non_empty):
            cells = [str(c) if c is not None else "" for c in row]
            lines.append("| " + " | ".join(cells) + " |")
            if idx == 0:
                lines.append("| " + " | ".join(["---"] * len(cells)) + " |")
        sections.append("\n".join(lines))
    wb.close()
    return "\n\n---\n\n".join(sections) if sections else "(no content)"


CONVERTERS = {
    ".pdf": convert_pdf,
    ".docx": convert_docx,
    ".pptx": convert_pptx,
    ".xlsx": convert_xlsx,
}


def convert_file(src: Path, out_dir: Path | None, overwrite: bool) -> tuple[Path, str]:
    """단일 파일 변환. 반환값: (출력경로, 상태메시지)"""
    ext = src.suffix.lower()
    converter = CONVERTERS.get(ext)
    if not converter:
        return src, "SKIP (no converter)"

    dest_dir = out_dir if out_dir else src.parent
    dest = dest_dir / (src.stem + ".md")

    if dest.exists() and not overwrite:
        return dest, "SKIP (exists)"

    dest_dir.mkdir(parents=True, exist_ok=True)

    content = converter(src)
    header = f"# {src.name}\n\n> 원본: `{src}`  \n> 변환일시: {_now()}\n\n---\n\n"
    dest.write_text(header + content, encoding="utf-8")
    return dest, "OK"


def _now() -> str:
    from datetime import datetime
    return datetime.now().strftime("%Y-%m-%d %H:%M")


def main():
    parser = argparse.ArgumentParser(description="Binary 문서 → Markdown 변환기")
    parser.add_argument("--root", default=".", help="탐색 루트 디렉터리")
    parser.add_argument("--out", default=None, help="출력 디렉터리 (없으면 원본 위치)")
    parser.add_argument("--formats", nargs="+", default=["pdf", "pptx", "docx", "xlsx"])
    parser.add_argument("--overwrite", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    out_dir = Path(args.out).resolve() if args.out else None

    files = find_binary_files(root, args.formats)
    print(f"발견된 파일: {len(files)}개\n")

    if args.dry_run:
        for f in files:
            print(f"  {f.relative_to(root)}")
        return

    results = {"OK": 0, "SKIP (exists)": 0, "SKIP (no converter)": 0, "ERROR": 0}
    for src in files:
        try:
            dest, status = convert_file(src, out_dir, args.overwrite)
            results[status] = results.get(status, 0) + 1
            icon = "✅" if status == "OK" else "⏭️"
            print(f"  {icon} [{status}] {src.relative_to(root)}")
            if status == "OK":
                print(f"       → {dest}")
        except Exception as e:
            results["ERROR"] += 1
            print(f"  ❌ [ERROR] {src.relative_to(root)}: {e}")

    print(f"\n완료: {results}")


if __name__ == "__main__":
    main()
