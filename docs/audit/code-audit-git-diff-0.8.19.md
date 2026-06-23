# Code Audit Report: Git Diff Review

## 1. 변경 사항 개요

| Category              | Files | Type           | Impact   |
| --------------------- | ----- | -------------- | -------- |
| README 포맷팅         | 8개   | Formatting     | Low      |
| 버전 업데이트         | 1개   | Metadata       | Low      |
| Windows 경로 처리     | 1개   | **Core Logic** | **High** |
| Generated 파일 포맷팅 | 2개   | Formatting     | Low      |

---

## 2. 핵심 변경 분석: `windows.rs` — `simplify_path()`

### 2.1 변경 내용

```rust
fn simplify_path(path: &std::path::Path) -> std::path::PathBuf {
    if let Ok(stripped) = path.strip_prefix(r"\\?\") {
        if !stripped.starts_with(r"UNC\") {
            return stripped.to_path_buf();
        }
    }
    path.to_path_buf()
}
```

**적용 위치:**

- `cmd.current_dir(&clean_workspace)` — 작업 디렉토리 설정
- `cmd.env("TEMP", clean_workspace.join(".libragent/tmp"))` — TEMP 환경변수
- `cmd.env("TMP", clean_workspace.join(".libragent/tmp"))` — TMP 환경변수
- `clean_workspace.join(".libragent/tmp")` — 스크립트 템프 디렉토리 생성

### 2.2 로직 정확성 평가

| 입력 경로                  | strip_prefix 결과      | UNC\ 체크 | 최종 결과                             |
| -------------------------- | ---------------------- | --------- | ------------------------------------- |
| `\\?\C:\Users\dev\project` | `C:\Users\dev\project` | N/A       | `C:\Users\dev\project` ✅             |
| `\\?\UNC\server\share`     | `UNC\server\share`     | True      | `\\?\UNC\server\share` (원본 유지) ✅ |
| `C:\Users\dev\project`     | Err                    | —         | `C:\Users\dev\project` ✅             |

**판단: 로직 정확함. 세 가지 케이스 모두 올바르게 처리됨.**

### 2.3 일관성 평가

`simplify_path()`의 호출이 `create_basic_isolated_command()` 함수 내에서 **단 한 번** 수행되고, 그 결과(`clean_workspace`)가 모든 경로 연산에 재사용된다. 이는 **DRY 원칙 준수** 및 **일관성 보장에 우수**.

### 2.4 위험 요소

| 위험도     | 항목                  | 설명                                                                                                       |
| ---------- | --------------------- | ---------------------------------------------------------------------------------------------------------- |
| **Medium** | Long path 지원 손실   | `\\?\` prefix 제거로 260자 초과 경로 지원 불가. 깊게 중첩된 워크스페이스에서 경로 길이 제한 에러 발생 가능 |
| **Low**    | 테스트 누락           | `simplify_path()` 자체에 대한 unit test 없음. 현재 테스트는 script content와 cleanup wrapper만 검증        |
| **Low**    | UNC 경로 보존 시 동작 | `\\?\UNC\server\share`가 원본 그대로 전달되지만, 이 경로가 외부 도구에서 여전히 파싱 실패할 가능성         |

### 2.5 개선 제안

```rust
// 현재: 단순 strip (long path 지원 포기)
fn simplify_path(path: &std::path::Path) -> std::path::PathBuf {
    // ...
}

// 대안: long path 지원을 유지하면서 UNC 문제를 우회하는 방식 고려
// (예: workspace 경로를 short path로 변환 후 사용)
```

**권장 사항:** `simplify_path()`에 대한 unit test 추가. 특히 `\\?\UNC\` 케이스와 일반 케이스 모두 커버.

---

## 3. 부수 변경 분석

### 3.1 README 파일 (8개)

**변경:** `<!-- RELEASE_DOWNLOADS_START -->` HTML comment 뒤에 빈 줄 추가

**판단: 적절함.** Markdown 렌더링을 위해 HTML 블록과 리스트 사이에 빈 줄이 필요함.

### 3.2 Cargo.lock

**변경:** `version = "0.8.18"` → `"0.8.19"`

**판단: 정상적인 버전 업데이트.**

### 3.3 Generated TypeScript 파일 (2개)

| 파일                  | 변경 내용                                                               | 판단                  |
| --------------------- | ----------------------------------------------------------------------- | --------------------- |
| `builtin-services.ts` | `(typeof BUILTIN_SERVICE_CANONICAL_NAMES)[number]` — typeof parentheses | Prettier 포맷팅. 무해 |
| `execution-mode.ts`   | 배열을 단일 라인으로 collapse                                           | Prettier 포맷팅. 무해 |

**판단:** Auto-generated 파일의 포맷팅 변경. Generator 스크립트(`scripts/sync-*.cjs`)의 Prettier 설정 변경으로 보임. 기능적 영향 없음.

---

## 4. 종합 평가

| 항목                 | 점수       | 비고                                                                           |
| -------------------- | ---------- | ------------------------------------------------------------------------------ |
| **문제 해결 적절성** | ⭐⭐⭐⭐⭐ | UNC 경로로 인한 외부 도구 크래시를 정확히 지적하고 해결                        |
| **구현 설계**        | ⭐⭐⭐⭐   | 단일 호출 + 재사용으로 일관성 우수. long path 지원 포기 트레이드오프 명시 필요 |
| **테스트 커버리지**  | ⭐⭐       | `simplify_path()` 자체 테스트 누락. script/wrapper 테스트는 기존 유지          |
| **부수 변경 관리**   | ⭐⭐⭐⭐⭐ | README, generated 파일 변경은 모두 포맷팅/메타데이터로 안전                    |

### 결론: **Merge 권장 (단, 테스트 추가 권고)**

`simplify_path()` 변경은 **실제 문제를 정확히 해결**하며, 구현도 깔끔하고 일관성 있게 적용됨.

**Merge 전 권고 사항:**

1. `simplify_path()` unit test 추가 (P0)
2. `\\?\` prefix 제거로 인한 long path 제한에 대한 문서화 (P1)
