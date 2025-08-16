# Refactoring Plan — App 최적화

요약: 생성된 `app_*.log`들을 분석한 결과, 에러(예외)보다는 WARN 로그가 다수 발견되었고, 반복적으로 등장하는 경고들이 몇 가지 유형으로 집중되어 있습니다. 아래는 발견된 주요 문제, 영향, 우선순위 권장 조치입니다.

## 체크리스트

- [x] `app_*.log` 파일별로 `[WARN]` 앞뒤 100라인을 추출하여 검토
- [x] 상위 반복 WARN 메시지 분류
- [ ] 각 문제에 대한 코드/설정 수정을 위한 PR 작성 및 테스트

## 주요 발견 사항

1. JSON schema 파싱 실패 (tauri_mcp_agent_lib::mcp)

- 반복 메시지: "Failed to parse JSON schema: missing field `type`, using default" 및 "invalid type: sequence, expected variant identifier, using default"
- 영향: 스키마가 불완전하거나 예상 형식이 아닌 경우 기본값으로 폴백되어 동작이 예상과 달라질 수 있습니다.
- 우선순위: 높음
- 권장 조치:

- 권장 조치:
  - 입력 스키마에 대한 사전 검증 추가(유효성 검사 및 상세 로그)
  - 스키마 파싱 실패 시 더 구체적인 경고/에러 레벨 분리(예: WARN -> ERROR 가능)
  - 문제를 유발하는 스키마 예시를 로그에 포함해 재현성 확보

2. AIService: tool 메시지에서 함수 이름을 찾을 수 없음

- 반복 메시지: "[AIService] Could not find function name for tool message with tool_call_id: ..."
- 영향: 툴 호출이 올바르게 라우팅되지 않아 기능이 정상 수행되지 않을 가능성
- 우선순위: 높음
- 권장 조치:

- 권장 조치:
  - 툴 메시지 포맷 파싱 로직 점검(요청/응답 페이로드에서 function name 추출 실패 원인 분석)
  - 툴 등록/매핑 로직에서 누락 검사 및 방어 코드 추가
  - 해당 `tool_call_id`를 수집해 재현 테스트 케이스 작성

3. AIService: Gemini sequencing 규칙 위반으로 보조 호출 제거

- 반복 메시지: "Removing assistant function call that violates Gemini sequencing rules {...}"
- 영향: 모델/어시스턴트의 툴 호출 순서 제약으로 일부 기능이 차단됨
- 우선순위: 중
- 권장 조치:

- 권장 조치:
  - 모델에 전달되는 함수 호출 시퀀스 검증 로직 검토
  - 시퀀싱 규칙 문서화 및 규칙 위반 시 상세 로그(원인, 이전/현재 역할 등) 추가

4. TimeLocationSystemPrompt: 지리 위치 조회 실패

- 반복 메시지: "[TimeLocationSystemPrompt] Failed to get geolocation {}"
- 영향: 위치 기반 기능(시간대, 지역화 등)에서 기본값 사용 혹은 기능 저하
- 우선순위: 낮~중
- 권장 조치:

- 권장 조치:
  - 위치 조회 실패 시 폴백 전략 도입(환경변수, 사용자 설정, IP 기반 추정 등)
  - 외부 API 호출 시 타임아웃/재시도 정책 명확화 및 로깅 강화

## 추가 권장 작업(운영/개선)

- 로그 스팸 감소: 동일 WARN이 초당/초기 반복으로 쌓이면 로그 필터링 또는 샘플링 적용
- 로그 레벨 정비: 재현 불가능한 입력 문제는 WARN, 서비스 중단 관련은 ERROR로 구분
- 모니터링/알림: 특정 워닝 유형(예: schema 파싱 실패, tool mapping 실패)에 대해 알림 룰 생성
- 반복 WARN 자동 집계 스크립트: 현재 만든 `scripts/extract_by_occurrence.sh`를 개선하여 중복 병합(겹치는 영역 합치기) 옵션 추가

## 다음 단계 (권장 우선순위)

1. `tauri_mcp_agent_lib::mcp`의 스키마 파싱 로직에서 입력 예외 케이스 하나를 재현하고 수정(로그에 문제 스키마 인라인 출력) — 담당자: 백엔드
2. `AIService`의 툴 메시지 파싱/매핑 경로에 방어 코드 추가 및 테스트 케이스 작성 — 담당자: 프론트엔드/서비스 레이어
3. 위치 조회 실패 폴백 전략 구현(환경변수 기반) — 담당자: 서비스 레이어
4. `scripts/extract_by_occurrence.sh`에 `--merge-overlap` 옵션 추가(원하시면 제가 구현해 드립니다)

## 생성된 파일들

- `scripts/extract_by_occurrence.sh` — WARN 발생 지점별로 `app_N.log`를 생성 (이미 실행됨)
- 분석한 산출물(임시): `/tmp/log_summary.txt`

요약 완료: 위 내용을 바탕으로 PR/수정 작업을 진행하겠습니다. 원하는 경우 1개 항목을 선택해 바로 코드/테스트 수정을 시작합니다。

--- 자동 로그 분석 요약 추가 (2025-08-16)

## 로그 파일 분석 요약 (자동 집계)

- 분석 대상: 생성된 `app_*.log` 파일들 (스크립트 `scripts/extract_by_occurrence.sh`로 추출). 전체 집계 결과는 `/tmp/log_summary.txt`에 저장되어 있습니다.
- 전반적 소결: ERROR는 거의 관찰되지 않았고 WARN이 다수(파일당 3~12건 수준) 발견되었습니다. WARN 패턴은 크게 네 가지로 집중되어 있습니다.

### 전역 상위 WARN 유형 (빈도, 자동집계 기준)

- Failed to parse JSON schema: missing field `type`, using default — 28건
- Could not find function name for tool message with tool_call_id: <various ids> — 다수(상위 항목들: 24,24,23,20,...) — 여러 고유 `tool_call_id`에 걸쳐 발생
- Removing assistant function call that violates Gemini sequencing rules {...} — 21건(다양한 인덱스/카운트)
- Failed to get geolocation {} — 10건

### 파일별 요약(요점)

- 대부분 파일(app_1..app_53)은 WARN 0~12건, ERROR 0건.
- `Failed to parse JSON schema` 계열 WARN은 주로 `app_1`~`app_9`, `app_4`, `app_5` 등 초기 블록에 집중되어 있습니다.
- `Could not find function name for tool message`는 다수의 파일(app_21..app_33 등)에 광범위하게 나타났고, 각기 다른 `tool_call_id`들로 반복 발생합니다.
- `Removing assistant function call that violates Gemini sequencing rules`는 여러 파일에서 관찰되어 툴 호출 시퀀스 검증 문제를 시사합니다.

### 단기 권장 조치 (우선순위 높음 → 낮음)

1. 스키마 파싱 문제(높음)

- 재현: `app_*`에서 `Failed to parse JSON schema`가 발생한 블록을 찾아 문제 스키마(입력 페이로드)를 캡처해 재현 케이스를 만드세요. (/tmp/log_summary.txt 참조)
- 조치: 입력 스키마 사전 검증 로직 추가, 실패 시 문제 스키마를 로그에 포함(민감정보 제외), 필요시 에러 레벨 상향.

2. 툴 매핑(함수 이름) 실패(높음)

- 원인 추정: 툴 메시지 페이로드에서 function name 추출 실패 또는 툴 등록시 id→이름 매핑 누락.
- 조치: 수집된 `tool_call_id`들을 기준으로 미매핑 샘플을 수집(이미 생성된 `app_*.log`에 샘플 존재). 방어 코드 추가(미매핑시 기본 처리 경로/로깅).

3. Gemini 시퀀싱 규칙 위반(중간)

- 조치: 호출 시퀀스 검증 로직 검토, 규칙 위반 시 상세 로그(이전/현재 역할, toolCallsCount 등) 추가해 왜 제거됐는지 정확히 파악.

4. 지리 위치 조회 실패(낮음)

- 조치: 폴백 정책(환경변수, 사용자 설정, IP 기반 추정), 외부 호출 재시도/타임아웃 정책 검토.

### 빠른 기술적 검사(권장 커맨드 예)

- 집계 파일 확인: cat /tmp/log_summary.txt
- 스키마 실패 블록 추출(예):
  - grep -n '\[WARN\]._Failed to parse JSON schema' app\__.log | cut -d: -f1,2
  - 이후 해당 app_N.log에서 문제 블록(라인)을 추출해 재현 케이스 생성

### 메모

- 현재 자동 분석은 단순 텍스트 집계 기반입니다. 재현/수정 작업 전에는 각 WARN 발생 시점의 전체 컨텍스트(입력 페이로드, 관련 요청 id 등)을 확보하는 것이 중요합니다.

위 항목을 `refactoring.md`에 추가했습니다. 원하시면 1) 스키마 실패 샘플을 자동으로 추출하는 스크립트를 추가하거나 2) `scripts/extract_by_occurrence.sh`에 `--merge-overlap` 옵션을 구현해 겹친 블록을 합치는 작업을 바로 진행하겠습니다.
