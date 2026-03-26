# [설계 문서] Knowledge Server v2: Local-First Intelligent Memory Engine

**버전:** 2.1 (Updated)  
**상태:** 승인 대기 (Awaiting Approval)  
**주요 변경 사항:** `sqlite-vss` → `sqlite-vec` 교체, Windows 하드웨어 가속 전략 추가

---

## 1. 개요 (Overview)
본 문서는 LibrAgent의 에이전트에게 고성능 로컬 장기 기억 능력을 부여하기 위한 지식 서버의 재설계안을 다룹니다. 클라우드 의존성을 제거하고, 사용자의 로컬 리소스를 활용하여 보안과 성능을 동시에 확보하는 하이브리드 지식 관리 시스템을 지향합니다.

---

## 2. 최적화된 기술 스택 (Technical Stack)

데스크톱 앱(Tauri) 환경에서 최상의 이식성과 성능을 제공하는 도구들로 구성되었습니다.

| 구분 | 기술 / 라이브러리 | 선택 이유 및 장점 |
| :--- | :--- | :--- |
| **Vector DB** | **`sqlite-vec`** | **최우선 권장**: Pure C 구현으로 의존성 없음. Windows/Tauri 배포 최적화. `INSERT/UPDATE` 성능이 `vss`보다 월등함. |
| **Embedding** | **`fastembed-rs`** | ONNX Runtime(`ort`) 기반. `all-MiniLM-L6-v2` 등 경량 모델을 로컬 CPU/GPU(DirectML)에서 고속 실행. |
| **Graph Logic** | **Triple Store (SQL)** | SQLite 내 `entities`, `relationships` 테이블 설계. `SQLiteGraph`의 패턴을 차용하여 복잡한 관계 추론 구현. |
| **Search Engine** | **FTS5 + KNN** | 키워드(FTS5)와 의미(KNN via `sqlite-vec`) 검색 결과를 RRF(Reciprocal Rank Fusion) 알고리즘으로 병합. |

---

## 3. 상세 구현 전략 (Implementation Details)

### 3.1. SQLite-vec & SeaORM 통합 (Backend)
LibrAgent의 기존 DB 프레임워크인 `SeaORM`과 다음과 같이 연동합니다.

1.  **Extension Load**: 
    *   `sqlx::sqlite::sqlite3_auto_extension`을 사용하여 `sqlite-vec`의 초기화 함수(`sqlite3_vec_init`)를 등록합니다.
2.  **Schema Design**:
    *   **Metadata**: `SeaORM` Entity로 정의 (일반 테이블).
    *   **Vector**: `vec0` 가상 테이블(Virtual Table) 생성 (Raw SQL 활용).
3.  **Search Pattern**:
    ```sql
    SELECT m.*, v.distance 
    FROM metadata m 
    JOIN vec_items v ON m.id = v.rowid 
    WHERE v.embedding MATCH ? AND k = 10 
    ORDER BY v.distance;
    ```

### 3.2. 로컬 임베딩 파이프라인 (Embedding)
*   **하드웨어 가속**: Windows 환경(`win32`)에서는 `ort`의 `DirectML` 또는 `CPU` 실행 공급자를 활성화하여 사용자 리소스를 최적으로 사용합니다.
*   **모델 캐싱**: 첫 실행 시 모델을 로컬에 다운로드하고, 이후에는 오프라인 모드로 즉시 로드합니다.

---

## 4. MCP 도구(Tool) 파라미터 설계

에이전트가 지식을 조작하고 조회하기 위한 확장된 API 사양입니다.

### 4.1. `record_knowledge` (기록)
*   `content` (string, req): 저장할 지식 전문.
*   `tags` (string[]): 카테고리 태그.
*   `auto_extract` (boolean): LLM을 활용한 엔티티/관계 자동 추출 여부.

### 4.2. `search_knowledge` (하이브리드 검색)
*   `query` (string, req): 자연어 질문 또는 키워드.
*   `limit` (int, default: 5): 반환할 결과 개수 (KNN의 `k` 값).
*   `mode` (enum): `keyword`, `semantic`, `hybrid` (권장: `hybrid`).

### 4.3. `explore_context` (관계 탐색)
*   `entity_name` (string): 중심 주제 이름.
*   `depth` (int): 그래프 탐색 깊이 (최대 3).
*   **Returns**: 연결된 지식 조각들의 상호 관계 맵(Markdown 또는 JSON).

---

## 5. 단계별 구현 로드맵 (Roadmap)

1.  **1단계: 인프라 구축**
    *   `sqlite-vec` 라이브러리 프로젝트 통합 및 Tauri 빌드 환경 테스트.
    *   `fastembed-rs`를 통한 로컬 임베딩 벤치마크 수행.
2.  **2단계: 하이브리드 검색 활성화**
    *   FTS5와 벡터 검색 병합 로직 구현.
    *   기본적인 MCP 도구(`record`, `search`) 배포.
3.  **3단계: 지식 관계망(Graph) 도입**
    *   SQLite 기반 그래프 테이블 스키마 적용.
    *   LLM 프롬프트를 통한 엔티티/관계 추출 자동화.

---

## 6. 개발팀 핵심 권고사항 (Summary)
> "기존의 복잡한 `sqlite-vss` 대신 **`sqlite-vec`**을 도입하여 Windows 환경에서의 배포 안정성을 확보하십시오. **`fastembed-rs`**는 에이전트의 응답 속도를 저해하지 않으면서도 강력한 검색 품질을 보장할 것입니다. 이 하이브리드 접근법은 LibrAgent를 단순한 챗봇이 아닌, **'사용자의 맥락을 기억하는 지능형 비서'**로 진화시킬 것입니다."
