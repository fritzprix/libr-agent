#!/usr/bin/env python3
"""
Conversion Status Checker
===========================
워크스페이스 내 binary 문서의 변환 현황을 리포트합니다.
변환 완료 / 미완료 파일을 구분하고, 미변환 파일을 우선순위 순으로 출력합니다.

Usage:
    python check_status.py [--root ROOT] [--formats pdf pptx docx xlsx]
                           [--ignore-dirs DIR1 DIR2] [--show-converted]
                           [--export STATUS_MD]

Options:
    --root          탐색 루트 (기본값: 현재 디렉터리)
    --formats       확인할 형식 (기본값: pdf pptx docx xlsx)
    --ignore-dirs   추가로 무시할 디렉터리
    --show-converted 변환 완료 파일도 출력
    --export        현황 리포트를 .md 파일로 저장

Examples:
    python check_status.py --root .
    python check_status.py --root . --show-converted
    python check_status.py --root . --export conversion_status.md
"""

import argparse
from datetime import datetime
from pathlib import Path

SKIP_DIRS = {".git", "__pycache__", "node_modules", ".github", "venv", ".venv"}


def collect_binary_files(root: Path, formats: list[str], skip_dirs: set[str]) -> list[Path]:
    exts = {f".{fmt.lstrip('.')}" for fmt in formats}
    result = []
    for p in sorted(root.rglob("*")):
        if p.is_dir():
            continue
        if any(part in skip_dirs for part in p.relative_to(root).parts):
            continue
        if p.suffix.lower() in exts:
            result.append(p)
    return result


def check_conversion(files: list[Path]) -> tuple[list[Path], list[Path]]:
    """변환 완료 / 미완료 파일 분리."""
    converted, not_converted = [], []
    for f in files:
        md = f.with_suffix(".md")
        if md.exists():
            converted.append(f)
        else:
            not_converted.append(f)
    return converted, not_converted


def format_size(path: Path) -> str:
    size = path.stat().st_size
    if size >= 1024 * 1024:
        return f"{size / (1024*1024):.1f} MB"
    return f"{size / 1024:.1f} KB"


def render_report(
    root: Path,
    converted: list[Path],
    not_converted: list[Path],
    show_converted: bool,
) -> str:
    skill_base_dir = Path(__file__).resolve().parent.parent
    convert_script = (skill_base_dir / "scripts" / "convert_binary_docs.py").as_posix()
    run_script = (skill_base_dir / "scripts" / "run.py").as_posix()
    now = datetime.now().strftime("%Y-%m-%d %H:%M")
    total = len(converted) + len(not_converted)
    rate = (len(converted) / total * 100) if total else 0

    lines = [
        f"# 변환 현황 리포트",
        f"",
        f"> 생성일시: {now}  ",
        f"> 루트: `{root}`",
        f"",
        f"## 요약",
        f"",
        f"| 항목 | 수 |",
        f"| --- | --- |",
        f"| 전체 binary 문서 | **{total}개** |",
        f"| ✅ 변환 완료 | **{len(converted)}개** |",
        f"| ❌ 미변환 | **{len(not_converted)}개** |",
        f"| 변환율 | **{rate:.1f}%** |",
        f"",
    ]

    if not_converted:
        lines += [
            f"---",
            f"",
            f"## ❌ 미변환 파일 ({len(not_converted)}개)",
            f"",
            f"| # | 파일명 | 경로 | 크기 |",
            f"| --- | --- | --- | --- |",
        ]
        for i, p in enumerate(not_converted, 1):
            rel = str(p.relative_to(root).as_posix())
            lines.append(f"| {i} | `{p.name}` | `{rel}` | {format_size(p)} |")
        lines.append("")

        # 변환 명령어 제안
        lines += [
            f"### 변환 실행 명령어",
            f"",
            f"```bash",
            f"# 전체 미변환 파일 변환",
            f'python "{convert_script}" --root .',
            f"",
            f"# 또는 통합 실행기 사용",
            f'python "{run_script}" --root .',
            f"```",
            f"",
        ]
    else:
        lines += [f"✅ 모든 binary 문서가 변환되어 있습니다.", f""]

    if show_converted and converted:
        lines += [
            f"---",
            f"",
            f"## ✅ 변환 완료 파일 ({len(converted)}개)",
            f"",
            f"| 파일명 | 원본 크기 | 변환 파일 |",
            f"| --- | --- | --- |",
        ]
        for p in converted:
            rel = str(p.relative_to(root).as_posix())
            md_rel = str(p.with_suffix(".md").relative_to(root).as_posix())
            lines.append(f"| `{p.name}` | {format_size(p)} | [`{p.stem}.md`]({md_rel}) |")
        lines.append("")

    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description="Binary 문서 변환 현황 확인")
    parser.add_argument("--root", default=".", help="탐색 루트 디렉터리")
    parser.add_argument("--formats", nargs="+", default=["pdf", "pptx", "docx", "xlsx"])
    parser.add_argument("--ignore-dirs", nargs="+", default=[])
    parser.add_argument("--show-converted", action="store_true", help="변환 완료 파일도 출력")
    parser.add_argument("--export", default=None, help="리포트를 .md 파일로 저장")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    skip_dirs = SKIP_DIRS | set(args.ignore_dirs)

    files = collect_binary_files(root, args.formats, skip_dirs)
    converted, not_converted = check_conversion(files)

    total = len(files)
    rate = (len(converted) / total * 100) if total else 0

    # 콘솔 출력
    print(f"\n📊 변환 현황 리포트")
    print(f"   루트: {root}")
    print(f"   전체: {total}개  |  ✅ 완료: {len(converted)}개  |  ❌ 미변환: {len(not_converted)}개  |  변환율: {rate:.1f}%\n")

    if not_converted:
        print(f"❌ 미변환 파일 ({len(not_converted)}개):")
        for p in not_converted:
            print(f"   {p.relative_to(root)}")
    else:
        print("✅ 모든 binary 문서가 변환되어 있습니다.")

    if args.show_converted and converted:
        print(f"\n✅ 변환 완료 ({len(converted)}개):")
        for p in converted:
            md = p.with_suffix(".md")
            print(f"   {p.relative_to(root)}  →  {md.relative_to(root)}")

    # 마크다운 리포트 저장
    if args.export:
        report = render_report(root, converted, not_converted, args.show_converted)
        out = Path(args.export)
        out.write_text(report, encoding="utf-8")
        print(f"\n📄 리포트 저장: {out}")


if __name__ == "__main__":
    main()
