#!/usr/bin/env python3
"""
Dependency Installer for workspace-indexer
==========================================
필요한 Python 패키지를 확인하고 없으면 자동 설치합니다.

Usage:
    python install_deps.py           # 전체 설치
    python install_deps.py --check   # 설치 여부만 확인 (설치 안 함)
    python install_deps.py --fmt pdf docx  # 특정 형식 필요 패키지만 설치
"""

import argparse
import importlib
import io
import subprocess
import sys

# Windows에서 cp949 인코딩으로 인한 UnicodeEncodeError 방지
if sys.platform.startswith("win"):
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding="utf-8")

# 형식별 필요 패키지
FORMAT_DEPS: dict[str, list[tuple[str, str]]] = {
    "pdf":  [("fitz", "pymupdf")],
    "docx": [("docx", "python-docx")],
    "pptx": [("pptx", "python-pptx")],
    "xlsx": [("openpyxl", "openpyxl")],
}

ALL_FORMATS = list(FORMAT_DEPS.keys())


def check_package(import_name: str) -> bool:
    """패키지가 임포트 가능한지 확인."""
    try:
        importlib.import_module(import_name)
        return True
    except ImportError:
        return False


def install_package(pip_name: str) -> bool:
    """pip으로 패키지 설치. 성공 여부 반환."""
    result = subprocess.run(
        [sys.executable, "-m", "pip", "install", pip_name],
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def main():
    parser = argparse.ArgumentParser(description="workspace-indexer 의존성 설치")
    parser.add_argument("--check", action="store_true", help="확인만 (설치 안 함)")
    parser.add_argument(
        "--fmt", nargs="+", choices=ALL_FORMATS,
        default=ALL_FORMATS,
        help="특정 형식만 처리 (기본: 전체)",
    )
    args = parser.parse_args()

    print("=== workspace-indexer 의존성 확인 ===\n")

    all_ok = True
    for fmt in args.fmt:
        for import_name, pip_name in FORMAT_DEPS[fmt]:
            installed = check_package(import_name)
            status = "✅ 설치됨" if installed else "❌ 없음"
            print(f"  [{fmt:4s}] {pip_name:15s}  {status}")

            if not installed:
                all_ok = False
                if not args.check:
                    print(f"         → pip install {pip_name} 실행 중...")
                    ok = install_package(pip_name)
                    if ok:
                        print(f"         ✅ 설치 완료")
                    else:
                        print(f"         ❌ 설치 실패 — 수동 실행: pip install {pip_name}")

    print()
    if all_ok:
        print("✅ 모든 의존성이 충족되어 있습니다.")
    elif args.check:
        print("⚠️  누락된 패키지가 있습니다.")
        print("   설치하려면: python install_deps.py")
        sys.exit(1)
    else:
        # 설치 후 재확인
        still_missing = [
            pip_name
            for fmt in args.fmt
            for import_name, pip_name in FORMAT_DEPS[fmt]
            if not check_package(import_name)
        ]
        if still_missing:
            print(f"❌ 설치 실패 패키지: {', '.join(still_missing)}")
            sys.exit(1)
        else:
            print("✅ 모든 의존성 설치 완료.")


if __name__ == "__main__":
    main()
