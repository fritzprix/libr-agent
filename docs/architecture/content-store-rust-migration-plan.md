# Content Store Rust Backend Migration Plan

## 📋 프로젝트 개요

**목표**: WebMCP의 content-store 모듈을 Rust Tauri backend로 이전하여 file:// URL 파싱 문제 해결 및 전반적인 성능과 안정성 향상

**배경**: 현재 WebMCP 환경에서 file:// URL 접근 제한과 PDF 등 바이너리 파일 파싱 오류로 인한 사용자 경험 저하

---

## 🔍 현재 문제 분석

### 1. 주요 이슈

- **file:// URL 접근 불가**: 브라우저 환경에서 fetch API가 file:// 프로토콜을 지원하지 않음
- **바이너리 파일 파싱 오류**: PDF 등 바이너리 파일을 텍스트로 파싱할 때 UTF-8 오류 발생
- **성능 제약**: JavaScript 환경에서의 대용량 파일 처리 한계
- **보안 제약**: 브라우저 환경의 파일 시스템 접근 제한

### 2. 영향도

- **사용자 경험**: 로컬 파일 업로드 실패로 인한 워크플로우 중단
- **기능 제약**: PDF, DOCX 등 다양한 문서 형식 지원 불가
- **개발 복잡성**: 브라우저 제약을 우회하기 위한 추가 코드 필요

---

## 🎯 솔루션 아키텍처

### 1. Rust Backend 이전 범위

#### A. Content Store Core

- **파일 파싱 엔진**: PDF, DOCX, TXT 등 다양한 형식 지원
- **텍스트 청킹**: 문서를 검색 가능한 단위로 분할
- **메타데이터 추출**: 파일 정보, 생성일, 크기 등

#### B. 검색 시스템

- **BM25 검색 엔진**: 고성능 전문 검색 알고리즘
- **인덱싱 시스템**: 실시간 문서 색인화
- **쿼리 처리**: 복합 검색 조건 및 필터링

#### C. 데이터 저장소

- **SQLite 기반 DB**: 파일 기반 경량 데이터베이스
- **세션 관리**: 사용자 세션별 파일 관리
- **캐시 시스템**: 빈번한 검색 결과 캐싱

### 2. 시스템 아키텍처

```
┌─────────────────────────────────────────────────────────┐
│                   Frontend (React)                     │
├─────────────────────────────────────────────────────────┤
│  - ResourceAttachmentContext                           │
│  - UI Components                                       │
│  - Service Layer                                       │
└─────────────────┬───────────────────────────────────────┘
                  │ Tauri Commands
                  ▼
┌─────────────────────────────────────────────────────────┐
│              Rust Backend (Tauri)                      │
├─────────────────────────────────────────────────────────┤
│  Content Store Module                                  │
│  ├── File Parser (PDF, DOCX, TXT)                     │
│  ├── Text Chunker                                     │
│  └── Metadata Extractor                               │
│                                                        │
│  Search Engine Module                                  │
│  ├── BM25 Implementation                              │
│  ├── Index Manager                                    │
│  └── Query Processor                                  │
│                                                        │
│  Database Module                                       │
│  ├── SQLite Connection                                │
│  ├── Schema Management                                │
│  └── Data Access Layer                                │
└─────────────────────────────────────────────────────────┘
```

---

## 🚀 구현 계획

### Phase 1: 기반 구조 설계 (1-2주)

#### 1.1 Rust 모듈 구조 설계

```rust
src-tauri/src/
├── content_store/
│   ├── mod.rs
│   ├── parser/
│   │   ├── mod.rs
│   │   ├── pdf.rs
│   │   ├── docx.rs
│   │   └── text.rs
│   ├── chunker.rs
│   ├── indexer.rs
│   └── storage.rs
├── search/
│   ├── mod.rs
│   ├── bm25.rs
│   └── query.rs
└── database/
    ├── mod.rs
    ├── models.rs
    └── migrations.rs
```

#### 1.2 Tauri 커맨드 인터페이스 설계

```rust
#[tauri::command]
async fn add_content(
    store_id: String,
    file_path: String,
    metadata: Option<FileMetadata>
) -> Result<ContentReference, String>

#[tauri::command]
async fn search_content(
    store_id: String,
    query: String,
    options: SearchOptions
) -> Result<Vec<SearchResult>, String>
```

### Phase 2: 파일 파싱 시스템 구현 (2-3주)

#### 2.1 파일 파서 구현

- **PDF Parser**: `pdf` 또는 `lopdf` 크레이트 활용
- **DOCX Parser**: `docx-rs` 크레이트 활용
- **Plain Text**: 직접 구현
- **이미지 OCR**: `tesseract` 바인딩 (옵션)

#### 2.2 텍스트 청킹 알고리즘

- 문장 단위 분할
- 의미적 경계 보존
- 오버랩 처리

### Phase 3: 검색 엔진 구현 (2-3주)

#### 3.1 BM25 검색 엔진

- **Tantivy 크레이트 활용**: 고성능 full-text search
- **커스텀 스코어링**: 문서 유형별 가중치
- **실시간 인덱싱**: 새 파일 추가 시 즉시 색인

#### 3.2 검색 최적화

- **인덱스 압축**: 디스크 사용량 최적화
- **쿼리 캐싱**: 빈번한 검색 결과 캐시
- **병렬 처리**: 멀티스레드 검색

### Phase 4: 데이터베이스 시스템 (1-2주)

#### 4.1 SQLite 스키마 설계

```sql
-- 파일 스토어
CREATE TABLE file_stores (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    session_id TEXT,
    created_at DATETIME,
    updated_at DATETIME
);

-- 파일 콘텐츠
CREATE TABLE file_contents (
    id TEXT PRIMARY KEY,
    store_id TEXT,
    filename TEXT,
    mime_type TEXT,
    content_hash TEXT,
    created_at DATETIME,
    FOREIGN KEY (store_id) REFERENCES file_stores(id)
);

-- 텍스트 청크
CREATE TABLE content_chunks (
    id TEXT PRIMARY KEY,
    content_id TEXT,
    chunk_text TEXT,
    start_line INTEGER,
    end_line INTEGER,
    FOREIGN KEY (content_id) REFERENCES file_contents(id)
);
```

#### 4.2 마이그레이션 시스템

- 스키마 버전 관리
- 자동 마이그레이션 실행

### Phase 5: 프론트엔드 통합 (1주)

#### 5.1 기존 WebMCP 인터페이스 유지

- `ContentStoreServer` 인터페이스 호환성 보장
- 기존 React 컴포넌트 수정 최소화

#### 5.2 에러 핸들링 개선

- Rust Result 타입을 JavaScript Promise로 변환
- 사용자 친화적 에러 메시지

---

## 📊 예상 효과

### 1. 성능 향상

- **파일 파싱 속도**: 2-5배 향상 (Rust 네이티브 성능)
- **검색 응답 시간**: 50-80% 단축 (Tantivy 엔진)
- **메모리 사용량**: 30-50% 감소 (효율적 메모리 관리)

### 2. 기능 확장

- **파일 형식 지원**: PDF, DOCX, PPT 등 20+ 형식
- **대용량 파일**: 100MB+ 파일 안정적 처리
- **동시 처리**: 멀티스레드 병렬 파싱

### 3. 사용자 경험 개선

- **file:// URL 지원**: 로컬 파일 직접 접근
- **안정적 파싱**: 바이너리 파일 오류 해결
- **실시간 검색**: 즉각적인 검색 결과

---

## ⚠️ 위험 요소 및 대응 방안

### 1. 기술적 위험

- **크레이트 의존성**: 안정적인 크레이트 선택, 대안 준비
- **플랫폼 호환성**: Windows/macOS/Linux 테스트 강화
- **메모리 관리**: 대용량 파일 처리 시 메모리 제한 설정

### 2. 마이그레이션 위험

- **데이터 손실**: 기존 IndexedDB 데이터 마이그레이션 도구 개발
- **호환성 문제**: 점진적 마이그레이션, 롤백 계획 수립
- **사용자 중단**: 무중단 배포, 기능 플래그 활용

---

## 📈 성공 지표

### 1. 기술 지표

- 파일 파싱 성공률: 95% 이상
- 검색 응답 시간: 100ms 이하
- 시스템 안정성: 99.9% 가동률

### 2. 사용자 지표

- 파일 업로드 오류율: 5% 이하
- 사용자 만족도: 4.5/5.0 이상
- 기능 사용률: 30% 증가

---

## 🛣️ 로드맵

| 구분    | 기간      | 주요 작업                | 산출물                      |
| ------- | --------- | ------------------------ | --------------------------- |
| Phase 1 | Week 1-2  | 아키텍처 설계, 모듈 구조 | 설계 문서, 기본 구조        |
| Phase 2 | Week 3-5  | 파일 파싱 시스템         | PDF/DOCX 파서               |
| Phase 3 | Week 6-8  | 검색 엔진 구현           | BM25 검색 시스템            |
| Phase 4 | Week 9-10 | 데이터베이스 시스템      | SQLite 스키마, 마이그레이션 |
| Phase 5 | Week 11   | 프론트엔드 통합          | 완전한 시스템               |
| Testing | Week 12   | 통합 테스트, 성능 최적화 | 배포 준비 완료              |

---

## 💰 리소스 요구사항

### 1. 개발 리소스

- **백엔드 개발자**: 1명 (Rust 전문)
- **프론트엔드 개발자**: 0.5명 (통합 작업)
- **QA 엔지니어**: 0.5명 (테스트 및 검증)

### 2. 기술 스택

- **Rust 크레이트**: tantivy, pdf, docx-rs, sqlx
- **개발 도구**: Cargo, Clippy, Rustfmt
- **테스트 도구**: 통합 테스트 프레임워크

---

## 📝 결론

이 마이그레이션은 SynapticFlow의 핵심 기능인 파일 처리와 검색을 근본적으로 개선하여, 사용자 경험과 시스템 안정성을 크게 향상시킬 것입니다. Rust의 성능과 안정성을 활용하여 현재의 기술적 제약을 해결하고, 향후 확장 가능한 아키텍처를 구축할 수 있습니다.
