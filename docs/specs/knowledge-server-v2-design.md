# [설계 문서] Knowledge Server v2: Local-First Intelligent Memory Engine

**버전:** 2.3 (LLM-First Contract Update)  
**상태:** 구현 반영 + 차기 계약 정리됨  
**주요 변경 사항:** `sqlite-vss` → `sqlite-vec` 교체, text-first MCP 응답 반영, `record_knowledge`를 LLM-first graph persistence 방향으로 명시

---

## 1. 개요 (Overview)

본 문서는 LibrAgent의 에이전트에게 고성능 로컬 장기 기억 능력을 부여하기 위한 지식 서버의 재설계안을 다룹니다. 클라우드 의존성을 제거하고, 사용자의 로컬 리소스를 활용하여 보안과 성능을 동시에 확보하는 하이브리드 지식 관리 시스템을 지향합니다.

---

## 2. 최적화된 기술 스택 (Technical Stack)

데스크톱 앱(Tauri) 환경에서 최상의 이식성과 성능을 제공하는 도구들로 구성되었습니다.

| 구분              | 기술 / 라이브러리      | 선택 이유 및 장점                                                                                                                             |
| :---------------- | :--------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------- |
| **Vector DB**     | **`sqlite-vec`**       | **최우선 권장**: Pure C 구현으로 의존성 없음. Windows/Tauri 배포 최적화. `INSERT/UPDATE` 성능이 `vss`보다 월등함.                             |
| **Embedding**     | **`fastembed-rs`**     | ONNX Runtime(`ort`) 기반. `all-MiniLM-L6-v2`를 로컬에서 실행하며, 현재 빌드는 기본 실행 공급자 선택과 명시적 로컬 캐시 디렉터리를 사용합니다. |
| **Graph Logic**   | **Triple Store (SQL)** | SQLite 내 `entities`, `relationships` 테이블 설계. `SQLiteGraph`의 패턴을 차용하여 복잡한 관계 추론 구현.                                     |
| **Search Engine** | **FTS5 + KNN**         | 키워드(FTS5)와 의미(KNN via `sqlite-vec`) 검색 결과를 RRF(Reciprocal Rank Fusion) 알고리즘으로 병합.                                          |

---

## 3. 상세 구현 전략 (Implementation Details)

### 3.1. SQLite-vec & SeaORM 통합 (Backend)

LibrAgent의 기존 DB 프레임워크인 `SeaORM`과 다음과 같이 연동합니다.

1.  **Extension Load**:
    - `sqlx::sqlite::sqlite3_auto_extension`을 사용하여 `sqlite-vec`의 초기화 함수(`sqlite3_vec_init`)를 등록합니다.
2.  **Schema Design**:
    - **Metadata**: `SeaORM` Entity로 정의 (일반 테이블).
    - **Vector**: `vec0` 가상 테이블(Virtual Table) 생성 (Raw SQL 활용).
3.  **Search Pattern**:
    ```sql
    SELECT m.*, v.distance
    FROM metadata m
    JOIN vec_items v ON m.id = v.rowid
    WHERE v.embedding MATCH ? AND k = 10
    ORDER BY v.distance;
    ```

### 3.2. 로컬 임베딩 파이프라인 (Embedding)

- **실행 공급자 전략**: 현재 빌드는 `fastembed-rs`의 ONNX Runtime 기본 실행 공급자 선택을 사용합니다. Windows 전용 `DirectML` 강제 선택은 향후 최적화 항목이며, 현재 코드는 이를 강제하지 않습니다.
- **모델 캐싱**: 첫 실행 시 모델을 로컬 캐시 디렉터리(`cache/libragent/fastembed`)에 다운로드하고 이후 재사용합니다.

---

## 4. MCP 도구(Tool) 파라미터 설계

에이전트가 지식을 조작하고 조회하기 위한 확장된 API 사양입니다.

### 4.1. `record_knowledge` (기록)

- `content` (string, req): 저장할 지식 전문.
- `tags` (string[]): 카테고리 태그.
- `entities` (object[]): 에이전트가 이미 추론한 엔티티 목록. 각 항목은 `name`(req), `entity_type`(opt), `description`(opt)를 가집니다.
- `relationships` (object[]): 에이전트가 이미 추론한 관계 목록. 각 항목은 `source`(req), `target`(req), `relation_type`(req)를 가집니다.
- `auto_extract` (boolean): `entities`/`relationships`가 비어 있거나 일부 누락되었을 때만 로컬 heuristic 추출을 수행할지 여부. 기본값은 `true`.

#### 설계 원칙: LLM-First, Heuristic-Fallback

- `record_knowledge`의 **주 경로(primary path)** 는 에이전트가 직접 구조화한 `entities`와 `relationships`를 서버에 전달하는 방식입니다.
- Knowledge 서버의 역할은 재추론이 아니라 **검증(validation), 정규화(normalization), 영속화(persistence)** 입니다.
- 로컬 heuristic 추출은 다음 경우에만 **보조 경로(fallback)** 로 동작합니다:
  - 구버전 클라이언트가 `content`만 전달하는 경우
  - 에이전트가 구조화 필드를 생략한 경우
  - 비-LLM 수집 경로(import/script/sync job)에서 최소한의 그래프 보강이 필요한 경우
- 즉, heuristic은 품질의 중심이 아니라 **하위 호환성과 누락 복구 장치**입니다.

### 4.2. `search_knowledge` (하이브리드 검색)

- `query` (string, req): 자연어 질문 또는 키워드.
- `limit` (int, default: 5): 반환할 결과 개수 (KNN의 `k` 값).
- `mode` (enum): `keyword`, `semantic`, `hybrid` (권장: `hybrid`).

### 4.3. `explore_context` (관계 탐색)

- `entity_name` (string): 중심 주제 이름.
- `depth` (int): 그래프 탐색 깊이 (최대 3).
- **Returns**: 에이전트가 바로 읽을 수 있는 텍스트 요약 + 구조화된 JSON(`nodes`, `edges`, `linked_chunks`).

---

## 5. 단계별 구현 로드맵 (Roadmap)

1.  **1단계: 인프라 구축**
    - `sqlite-vec` 라이브러리 프로젝트 통합 및 Tauri 빌드 환경 테스트.
    - `fastembed-rs`를 통한 로컬 임베딩 벤치마크 수행.
2.  **2단계: 하이브리드 검색 활성화**
    - FTS5와 벡터 검색 병합 로직 구현.
    - 기본적인 MCP 도구(`record`, `search`) 배포.
3.  **3단계: 지식 관계망(Graph) 도입**
    - SQLite 기반 그래프 테이블 스키마 적용.
    - LLM이 제공한 엔티티/관계의 직접 영속화.
    - heuristic 기반 추출은 fallback/legacy compatibility 용도로만 유지.
    - linked chunk 탐색 및 text-first graph 응답 제공.

---

## 6. 개발팀 핵심 권고사항 (Summary)

> "Knowledge v2의 핵심 방향은 명확합니다. **에이전트가 이미 알고 있는 그래프는 에이전트가 직접 구조화해서 전달하고, 서버는 그것을 검증·저장해야 합니다.** `sqlite-vec`와 `fastembed-rs`는 검색 기반을 맡고, heuristic 추출은 어디까지나 fallback으로 남겨두는 것이 맞습니다."
