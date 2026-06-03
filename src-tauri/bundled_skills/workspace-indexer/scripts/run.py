#!/usr/bin/env python3
"""
Workspace Indexer — Unified Runner
====================================
binary 변환 + index.md 구축을 한 번에 실행하는 통합 진입점.

Usage:
    python run.py [--root ROOT] [--out OUT] [--formats pdf pptx docx xlsx]
                  [--overwrite] [--skip-convert] [--skip-index]
                  [--index-out INDEX_MD] [--keywords KEYWORDS_FILE]
                  [--ignore-dirs DIR1 DIR2] [--dry-run]

주요 옵션:
    --root          워크스페이스 루트 (기본값: 현재 디렉터리)
    --out           변환 파일 출력 디렉터리 (기본값: 원본 파일 위치)
    --formats       변환할 형식 (기본값: pdf pptx docx xlsx)
    --overwrite     기존 변환 파일 덮어쓰기
    --skip-convert  binary 변환 건너뜀 (색인만 갱신)
    --skip-index    index.md 생성 건너뜀 (변환만 수행)
    --index-out     index.md 출력 경로 (기본값: ROOT/index.md)
    --keywords      커스텀 키워드 파일 (.txt)
    --ignore-dirs   추가로 무시할 디렉터리
    --dry-run       실제 파일 저장 없이 동작만 시뮬레이션

Examples:
    # 전체 워크플로우
    python run.py --root .

    # 변환만 (PDF/DOCX)
    python run.py --root . --skip-index --formats pdf docx

    # 색인만 갱신
    python run.py --root . --skip-convert

    # dry-run으로 미리 확인
    python run.py --root . --dry-run
"""

import argparse
import io
import os
import subprocess
import sys
from pathlib import Path

# Windows에서 cp949 인코딩으로 인한 UnicodeEncodeError 방지
if sys.platform.startswith("win"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8")

SCRIPT_DIR = Path(__file__).parent
CONVERT_SCRIPT = SCRIPT_DIR / "convert_binary_docs.py"
INDEX_SCRIPT = SCRIPT_DIR / "build_index.py"


def run_step(label: str, cmd: list[str]) -> bool:
    """서브프로세스로 스크립트 실행. 성공 여부 반환."""
    print(f"\n{'='*60}")
    print(f"  {label}")
    print(f"{'='*60}")
    
    # Windows에서 하위 프로세스의 UTF-8 출력을 강제하기 위해 환경 변수 상속 및 설정
    env = os.environ.copy()
    env["PYTHONIOENCODING"] = "utf-8"
    
    result = subprocess.run(cmd, env=env)
    if result.returncode != 0:
        print(f"\n❌ {label} 실패 (exit code {result.returncode})")
        return False
    return True


def build_convert_cmd(args) -> list[str]:
    cmd = [sys.executable, str(CONVERT_SCRIPT), "--root", args.root]
    if args.out:
        cmd += ["--out", args.out]
    if args.formats:
        cmd += ["--formats"] + args.formats
    if args.overwrite:
        cmd.append("--overwrite")
    if args.dry_run:
        cmd.append("--dry-run")
    return cmd


def build_index_cmd(args) -> list[str]:
    cmd = [sys.executable, str(INDEX_SCRIPT), "--root", args.root]
    if args.index_out:
        cmd += ["--out", args.index_out]
    if args.keywords:
        cmd += ["--keywords", args.keywords]
    if args.ignore_dirs:
        cmd += ["--ignore-dirs"] + args.ignore_dirs
    if args.dry_run:
        cmd.append("--dry-run")
    return cmd


def main():
    parser = argparse.ArgumentParser(
        description="workspace-indexer 통합 실행기",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--root", default=".", help="워크스페이스 루트 디렉터리")
    parser.add_argument("--out", default=None, help="변환 파일 출력 디렉터리")
    parser.add_argument("--formats", nargs="+", default=["pdf", "pptx", "docx", "xlsx"])
    parser.add_argument("--overwrite", action="store_true")
    parser.add_argument("--skip-convert", action="store_true", help="binary 변환 건너뜀")
    parser.add_argument("--skip-index", action="store_true", help="index.md 생성 건너뜀")
    parser.add_argument("--index-out", default=None, help="index.md 출력 경로")
    parser.add_argument("--keywords", default=None, help="커스텀 키워드 파일")
    parser.add_argument("--ignore-dirs", nargs="+", default=[], help="무시할 디렉터리")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    print(f"🗂️  Workspace Indexer")
    print(f"   루트: {root}")
    print(f"   모드: {'dry-run' if args.dry_run else 'live'}")
    steps = []
    if not args.skip_convert:
        steps.append("binary 변환")
    if not args.skip_index:
        steps.append("index.md 구축")
    print(f"   단계: {' → '.join(steps) if steps else '(없음)'}\n")

    success = True

    # Step 1: binary 변환
    if not args.skip_convert:
        cmd = build_convert_cmd(args)
        ok = run_step("Step 1: Binary 문서 → Markdown 변환", cmd)
        if not ok:
            success = False

    # Step 2: index.md 구축
    if not args.skip_index:
        cmd = build_index_cmd(args)
        ok = run_step("Step 2: index.md 구축", cmd)
        if not ok:
            success = False

    # 최종 결과
    print(f"\n{'='*60}")
    if success:
        if not args.dry_run:
            index_path = args.index_out or str(root / "index.md")
            print(f"✅ 완료!")
            if not args.skip_index:
                print(f"   index.md → {index_path}")
        else:
            print(f"✅ dry-run 완료 (파일 저장 없음)")
    else:
        print(f"⚠️  일부 단계에서 오류가 발생했습니다.")
        sys.exit(1)


if __name__ == "__main__":
    main()
