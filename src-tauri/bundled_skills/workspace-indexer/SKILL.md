---
name: workspace-indexer
description: |
  워크스페이스를 탐색하여 (1) binary 형식 문서(PDF, PPTX, DOCX, XLSX)를 텍스트/마크다운으로
  변환하고, (2) index.md를 통해 키워드 맵 및 파일 색인을 구축하는 워크플로우.
  다음 상황에서 사용: "binary 파일 변환", "문서 색인 만들어줘", "index.md 생성",
  "키워드 맵 구축", "워크스페이스 파일 목록화", "pdf/pptx/docx/xlsx 텍스트 변환".
---

# Workspace Indexer

## Overview

두 가지 핵심 기능을 제공합니다:

1. **Binary → Text 변환** — PDF, PPTX, DOCX, XLSX를 마크다운(.md)으로 변환
2. **Index 구축** — 워크스페이스 파일 색인 + 키워드 맵을 `index.md`로 생성

## Path conventions

이 skill에서 언급하는 내부 경로는 모두 **이 `SKILL.md`가 있는 디렉터리 기준 상대 경로**이며, **workspace의 현재 디렉터리(`./`)와는 다릅니다.**

- 스크립트: `scripts/...`
- 기타 리소스: skill 디렉터리 내부 상대 경로
- `python scripts/...` 같은 표기는 실제 실행 시 이 skill의 절대 Base Directory 기준으로 해석
- 아래 예시에서 `<skill-base-dir>`는 이 skill이 실제 배포된 절대 경로를 뜻함
- 워크스페이스 루트 같은 외부 경로는 별도로 명시된 인자(`--root`, `--out`)로 전달

## 스크립트 목록

| 스크립트 | 역할 |
| --- | --- |
| `run.py` | 변환 + 색인을 한 번에 실행하는 **통합 진입점** |
| `convert_binary_docs.py` | binary 문서 → 마크다운 변환 |
| `build_index.py` | 파일 색인 + 키워드 맵 → `index.md` 생성 |
| `check_status.py` | 변환 완료/미완료 현황 리포트 |
| `install_deps.py` | 필요 패키지 확인 및 자동 설치 |

## 사전 요구사항

```bash
# 자동 설치
python <skill-base-dir>/scripts/install_deps.py

# 수동 설치
pip install pymupdf python-docx python-pptx openpyxl
```

## Workflow

### Task A: Binary 문서 → 마크다운 변환

스크립트: `scripts/convert_binary_docs.py`

```bash
# dry-run으로 대상 파일 미리 확인
python <skill-base-dir>/scripts/convert_binary_docs.py --root . --dry-run

# 전체 변환 (원본 파일 옆에 .md 생성)
python <skill-base-dir>/scripts/convert_binary_docs.py --root .

# 특정 형식만 변환
python <skill-base-dir>/scripts/convert_binary_docs.py --root . --formats pdf docx

# 별도 디렉터리에 출력
python <skill-base-dir>/scripts/convert_binary_docs.py --root . --out ./converted

# 기존 변환 파일 덮어쓰기
python <skill-base-dir>/scripts/convert_binary_docs.py --root . --overwrite
```

**형식별 변환 방식:**

| 형식 | 라이브러리 | 출력 구조 |
| --- | --- | --- |
| PDF | pymupdf (fitz) | `## Page N` 섹션 |
| DOCX | python-docx | 헤딩 → `#`/`##`/`###`, 테이블 → MD 테이블 |
| PPTX | python-pptx | `## Slide N` 섹션 |
| XLSX | openpyxl | `## Sheet: 시트명` + MD 테이블 |

---

### Task B: index.md 구축

스크립트: `scripts/build_index.py`

```bash
# 기본 실행 (루트에 index.md 생성)
python <skill-base-dir>/scripts/build_index.py --root .

# dry-run으로 파일 목록·키워드 미리 확인
python <skill-base-dir>/scripts/build_index.py --root . --dry-run

# 출력 위치 지정
python <skill-base-dir>/scripts/build_index.py --root . --out docs/index.md

# 커스텀 키워드 파일 사용 (한 줄에 키워드 하나)
python <skill-base-dir>/scripts/build_index.py --root . --keywords keywords.txt

# 특정 디렉터리 무시
python <skill-base-dir>/scripts/build_index.py --root . --ignore-dirs tmp out
```

**index.md 구조:**

```
# Workspace Index
> 생성일시 / 루트 / 파일 수

## 파일 색인 (텍스트)
### 📁 디렉터리명
- [파일명](경로) (크기 KB)

## Binary 문서 목록
| 파일명 | 경로 | 크기 | 변환 파일 |
(변환된 .md 있으면 링크, 없으면 미변환 표시)

## 키워드 맵
### 키워드
- [등장 파일 목록]
```

**키워드 추출 방식:**
- 기본: `.md` 파일의 `#`, `##`, `###` 헤딩에서 자동 추출
- 커스텀: `--keywords keywords.txt` 파일 내 키워드가 포함된 파일 매핑

---

### Task C: 전체 워크플로우 (통합 실행기)

`run.py`가 변환 → 색인을 순서대로 자동 실행합니다.

```bash
# 전체 워크플로우 (권장)
python <skill-base-dir>/scripts/run.py --root .

# 변환만 (색인 건너뜀)
python <skill-base-dir>/scripts/run.py --root . --skip-index

# 색인만 갱신 (변환 건너뜀)
python <skill-base-dir>/scripts/run.py --root . --skip-convert

# dry-run으로 전체 미리 확인
python <skill-base-dir>/scripts/run.py --root . --dry-run
```

### Task D: 변환 현황 확인

```bash
# 콘솔에서 미변환 파일 확인
python <skill-base-dir>/scripts/check_status.py --root .

# 변환 완료 파일 포함 전체 현황
python <skill-base-dir>/scripts/check_status.py --root . --show-converted

# 마크다운 리포트로 저장
python <skill-base-dir>/scripts/check_status.py --root . --export conversion_status.md
```

## 주의사항

- `.git`, `__pycache__`, `node_modules`, `.github`, `venv` 디렉터리는 기본으로 무시
- 이미 변환된 파일은 `--overwrite` 없이는 재변환하지 않음
- 스캔된 PDF(이미지 기반)는 텍스트 추출 불가 — OCR 별도 필요
- 대용량 워크스페이스에서 키워드 맵 과다 시 `--max-keyword-files` 파라미터로 제한
