#!/usr/bin/env python3
"""
Workspace Index Builder
========================
워크스페이스를 탐색하여 index.md (파일 색인 + 키워드 맵)를 생성합니다.

Usage:
    python build_index.py [--root ROOT_DIR] [--out OUTPUT_MD] [--keywords KEYWORDS_FILE]
                          [--ignore-dirs DIR1 DIR2 ...] [--dry-run]

Options:
    --root          탐색할 루트 디렉터리 (기본값: 현재 디렉터리)
    --out           출력할 index.md 경로 (기본값: ROOT/index.md)
    --keywords      키워드 목록 파일 (.txt, 한 줄에 하나씩). 없으면 자동 추출.
    --ignore-dirs   무시할 디렉터리 이름 (기본값: .git __pycache__ node_modules .github)
    --dry-run       파일 목록과 키워드만 출력, 파일 저장 안 함
    --max-keyword-files  키워드당 최대 파일 수 (기본값: 20)

Features:
    - 워크스페이스 내 모든 텍스트 파일(md, txt, py 등)과 binary 파일 목록화
    - 마크다운 파일의 # 헤딩에서 키워드 자동 추출
    - 각 키워드가 등장하는 파일 목록 생성 (키워드 맵)
    - 디렉터리별 파일 트리 생성
    - binary 파일 목록 별도 섹션으로 분리

Examples:
    python build_index.py --root . --out index.md
    python build_index.py --root . --keywords my_keywords.txt
    python build_index.py --root . --dry-run
"""

import argparse
import re
import sys
from collections import defaultdict
from datetime import datetime
from pathlib import Path

# 무시할 기본 디렉터리
DEFAULT_SKIP_DIRS = {".git", "__pycache__", "node_modules", ".github", "venv", ".venv", ".mypy_cache"}

# 텍스트로 처리할 확장자
TEXT_EXTS = {".md", ".txt", ".py", ".sh", ".json", ".yaml", ".yml", ".toml", ".csv", ".html", ".js", ".ts"}

# Binary 문서 확장자
BINARY_DOC_EXTS = {".pdf", ".docx", ".pptx", ".xlsx", ".xls", ".ppt", ".doc"}

# 색인에 포함할 확장자 (전체)
INDEX_EXTS = TEXT_EXTS | BINARY_DOC_EXTS | {".ipynb"}


def collect_files(root: Path, skip_dirs: set[str]) -> tuple[list[Path], list[Path]]:
    """텍스트 파일과 binary 파일을 분리하여 반환."""
    text_files, binary_files = [], []
    for p in sorted(root.rglob("*")):
        if p.is_dir():
            continue
        if any(part in skip_dirs for part in p.relative_to(root).parts):
            continue
        ext = p.suffix.lower()
        if ext not in INDEX_EXTS:
            continue
        if ext in BINARY_DOC_EXTS:
            binary_files.append(p)
        else:
            text_files.append(p)
    return text_files, binary_files


def extract_headings(path: Path) -> list[str]:
    """마크다운 파일에서 # 헤딩 텍스트 추출."""
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except Exception:
        return []
    headings = []
    for line in text.splitlines():
        m = re.match(r"^#{1,3}\s+(.+)", line)
        if m:
            heading = m.group(1).strip()
            # 마크다운 링크, 코드 제거
            heading = re.sub(r"\[([^\]]+)\]\([^\)]+\)", r"\1", heading)
            heading = re.sub(r"`[^`]+`", "", heading)
            heading = heading.strip()
            if heading:
                headings.append(heading)
    return headings


def extract_keywords_from_files(text_files: list[Path], root: Path) -> dict[str, list[str]]:
    """
    텍스트 파일에서 키워드(헤딩) 추출.
    반환: {keyword: [rel_path1, rel_path2, ...]}
    """
    keyword_map: dict[str, list[str]] = defaultdict(list)
    for p in text_files:
        if p.suffix.lower() != ".md":
            continue
        headings = extract_headings(p)
        rel = str(p.relative_to(root).as_posix())
        for h in headings:
            keyword_map[h].append(rel)
    return dict(keyword_map)


def load_custom_keywords(keyword_file: Path, text_files: list[Path], root: Path) -> dict[str, list[str]]:
    """사용자 정의 키워드 파일에서 키워드를 읽고, 파일 내 등장 여부 매핑."""
    keywords = [line.strip() for line in keyword_file.read_text(encoding="utf-8").splitlines() if line.strip()]
    keyword_map: dict[str, list[str]] = defaultdict(list)
    for p in text_files:
        try:
            content = p.read_text(encoding="utf-8", errors="ignore").lower()
        except Exception:
            continue
        rel = str(p.relative_to(root).as_posix())
        for kw in keywords:
            if kw.lower() in content:
                keyword_map[kw].append(rel)
    return dict(keyword_map)


def build_dir_tree(all_files: list[Path], root: Path) -> dict[str, list[Path]]:
    """디렉터리별 파일 그룹핑. POSIX 경로를 키로 사용하여 OS 무관하게 일관성 유지."""
    tree: dict[str, list[Path]] = defaultdict(list)
    for p in all_files:
        rel = p.relative_to(root)
        parts = rel.parts
        # str(Path(...))는 Windows에서 백슬래시를 반환하므로 as_posix() 사용
        dir_key = Path(*parts[:-1]).as_posix() if len(parts) > 1 else "."
        tree[dir_key].append(p)
    return dict(tree)


def render_index(
    root: Path,
    text_files: list[Path],
    binary_files: list[Path],
    keyword_map: dict[str, list[str]],
    max_keyword_files: int,
) -> str:
    now = datetime.now().strftime("%Y-%m-%d %H:%M")
    lines = [
        f"# Workspace Index",
        f"",
        f"> 생성일시: {now}  ",
        f"> 루트: `{root}`  ",
        f"> 텍스트 파일: **{len(text_files)}개** | Binary 문서: **{len(binary_files)}개**",
        f"",
        f"---",
        f"",
        f"## 목차",
        f"",
        f"1. [파일 색인 (텍스트)](#파일-색인-텍스트)",
        f"2. [Binary 문서 목록](#binary-문서-목록)",
        f"3. [키워드 맵](#키워드-맵)",
        f"",
        f"---",
        f"",
    ]

    # --- 텍스트 파일 색인 ---
    lines += [f"## 파일 색인 (텍스트)", f""]
    tree = build_dir_tree(text_files, root)
    for dir_key in sorted(tree.keys()):
        if dir_key == ".":
            lines.append(f"### 📁 (루트)")
        else:
            lines.append(f"### 📁 {dir_key}")
        for p in sorted(tree[dir_key]):
            rel = str(p.relative_to(root).as_posix())
            size_kb = p.stat().st_size / 1024
            lines.append(f"- [`{p.name}`]({rel}) ({size_kb:.1f} KB)")
        lines.append("")

    # --- Binary 문서 ---
    lines += [f"---", f"", f"## Binary 문서 목록", f""]
    if binary_files:
        lines.append("| 파일명 | 경로 | 크기 | 변환 파일 |")
        lines.append("| --- | --- | --- | --- |")
        for p in sorted(binary_files):
            rel = str(p.relative_to(root).as_posix())
            size_kb = p.stat().st_size / 1024
            md_path = p.with_suffix(".md")
            md_rel = str(md_path.relative_to(root).as_posix()) if md_path.exists() else "-"
            converted = f"[MD]({md_rel})" if md_path.exists() else "❌ 미변환"
            lines.append(f"| `{p.name}` | `{rel}` | {size_kb:.1f} KB | {converted} |")
    else:
        lines.append("_(Binary 문서 없음)_")
    lines.append("")

    # --- 키워드 맵 ---
    lines += [f"---", f"", f"## 키워드 맵", f""]
    if keyword_map:
        lines.append(f"총 **{len(keyword_map)}개** 키워드")
        lines.append("")
        for kw in sorted(keyword_map.keys(), key=lambda x: x.lower()):
            file_list = keyword_map[kw][:max_keyword_files]
            overflow = len(keyword_map[kw]) - max_keyword_files
            lines.append(f"### {kw}")
            for rel in file_list:
                lines.append(f"- [`{Path(rel).name}`]({rel})")
            if overflow > 0:
                lines.append(f"- _외 {overflow}개 파일_")
            lines.append("")
    else:
        lines.append("_(키워드 없음)_")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="Workspace Index Builder")
    parser.add_argument("--root", default=".", help="탐색 루트 디렉터리")
    parser.add_argument("--out", default=None, help="출력 index.md 경로")
    parser.add_argument("--keywords", default=None, help="커스텀 키워드 파일 (.txt)")
    parser.add_argument("--ignore-dirs", nargs="+", default=[], help="추가로 무시할 디렉터리")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--max-keyword-files", type=int, default=20)
    args = parser.parse_args()

    root = Path(args.root).resolve()
    skip_dirs = DEFAULT_SKIP_DIRS | set(args.ignore_dirs)
    out_path = Path(args.out).resolve() if args.out else root / "index.md"

    print(f"🔍 탐색 중: {root}")
    text_files, binary_files = collect_files(root, skip_dirs)
    print(f"   텍스트: {len(text_files)}개 | Binary: {len(binary_files)}개")

    if args.keywords:
        kw_file = Path(args.keywords)
        if not kw_file.exists():
            print(f"❌ 키워드 파일 없음: {kw_file}", file=sys.stderr)
            sys.exit(1)
        keyword_map = load_custom_keywords(kw_file, text_files, root)
        print(f"   커스텀 키워드: {len(keyword_map)}개 매핑됨")
    else:
        keyword_map = extract_keywords_from_files(text_files, root)
        print(f"   자동 추출 키워드: {len(keyword_map)}개")

    if args.dry_run:
        print("\n=== 텍스트 파일 ===")
        for f in text_files[:20]:
            print(f"  {f.relative_to(root)}")
        if len(text_files) > 20:
            print(f"  ... 외 {len(text_files)-20}개")
        print("\n=== Binary 파일 ===")
        for f in binary_files[:20]:
            print(f"  {f.relative_to(root)}")
        print("\n=== 키워드 샘플 (상위 20개) ===")
        for kw in list(sorted(keyword_map.keys()))[:20]:
            print(f"  '{kw}': {len(keyword_map[kw])}개 파일")
        return

    content = render_index(root, text_files, binary_files, keyword_map, args.max_keyword_files)
    out_path.write_text(content, encoding="utf-8")
    print(f"\n✅ 저장됨: {out_path}")
    print(f"   크기: {len(content):,} bytes")


if __name__ == "__main__":
    main()
