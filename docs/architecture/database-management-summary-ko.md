# Database Management Documentation - 추가 완료

## 생성된 문서

`docs/architecture/database-management.md` 파일이 생성되었습니다.

## 문서 내용 요약

이 문서는 LibrAgent 프로젝트의 데이터베이스 관리 아키텍처를 상세히 설명합니다:

### 주요 섹션

1. **아키텍처 컴포넌트**
   - Database Stack 다이어그램
   - Repository Pattern 설명
   - 파일 구조

2. **핵심 컴포넌트**
   - Error Handling (`error.rs`)
   - Message Repository (`message_repository.rs`)
     - 트레이트 정의
     - 데이터베이스 스키마 (`messages`, `message_index_meta` 테이블)
     - 주요 작업 (페이지네이션, Upsert, 벌크 삽입, 검색 인덱스 추적)
   - Content Store Repository (`content_store_repository.rs`)
     - 스키마 (`stores`, `contents`, `chunks` 테이블)
     - Cascade 삭제 작업
   - Session Repository (`session_repository.rs`)

3. **데이터베이스 초기화**
   - `lib.rs`에서의 초기화 시퀀스
   - 데이터베이스 파일 자동 생성

4. **연결 풀 관리**
   - SQLx Connection Pool 설정
   - 효율적인 연결 재사용

5. **트랜잭션 관리**
   - 단일 트랜잭션 패턴
   - 에러 처리

6. **백그라운드 인덱싱 시스템**
   - IndexingWorker 동작 방식
   - BM25 검색 인덱스 자동 유지보수

7. **하이브리드 스토리지 전략**
   - Content Store의 이중 백엔드 (SQLite + In-Memory)
   - 성능 최적화 전략

8. **데이터 플로우 예제**
   - 메시지 생성 플로우
   - 세션 삭제 플로우
   - Content Store 업로드 플로우

9. **마이그레이션 전략**
   - 현재 접근 방식 (CREATE TABLE IF NOT EXISTS)
   - 장단점
   - 향후 권장사항 (SQLx Migrations)

10. **성능 최적화**
    - 인덱싱 전략
    - 쿼리 최적화
    - Upsert 패턴

11. **에러 처리 모범 사례**
    - 구조화된 에러 타입
    - 트랜잭션 에러 처리
    - Tauri 커맨드 호환성

12. **테스트 전략**
    - Repository 테스트
    - 통합 테스트
    - 트랜잭션 테스트

13. **전역 상태 관리**
    - Repository 접근 패턴
    - Singleton 패턴

## 기술적 상세사항

### Repository Pattern 구현

프로젝트는 Repository Pattern을 사용하여 데이터베이스 작업을 추상화합니다:

- **Trait 기반 추상화**: 각 Repository는 trait으로 정의되어 테스트 가능성과 유연성 제공
- **SQLite 구현**: 각 trait에 대한 구체적인 SQLite 구현 제공
- **비동기 작업**: `async_trait`을 사용한 완전한 비동기 지원

### 데이터베이스 스키마

#### Messages 테이블

- 채팅 메시지 저장
- 도구 호출, 스트리밍, 사고 과정(thinking) 등 다양한 메시지 타입 지원
- 세션별 페이지네이션을 위한 복합 인덱스

#### Content Store 테이블

- `stores`: 세션당 1:1 관계의 콘텐츠 저장소
- `contents`: 업로드된 파일 메타데이터 및 전체 내용
- `chunks`: 검색 및 임베딩을 위한 텍스트 청크

#### 검색 인덱스 메타데이터

- BM25 인덱스 빌드 정보 추적
- 더티 체크를 통한 자동 재빌드

### 성능 특징

1. **연결 풀링**: SQLx를 통한 효율적인 연결 관리
2. **복합 인덱스**: 빈번한 쿼리 패턴에 최적화된 인덱스
3. **Upsert 패턴**: 단일 쿼리로 삽입/업데이트 처리
4. **하이브리드 스토리지**: 읽기는 메모리에서, 쓰기는 SQLite로
5. **백그라운드 작업**: UI를 차단하지 않는 인덱싱

## 참고 문서

문서는 다음과 관련 문서들과 연결되어 있습니다:

- Chat Feature Architecture
- Content Store MCP Integration
- Search & Indexing
- Tauri Commands API

## 요약

이 문서는 LibrAgent의 데이터베이스 관리 시스템이 다음을 제공함을 보여줍니다:

1. ✅ **Clean Architecture**: Repository 패턴과 trait 기반 추상화
2. ✅ **Type Safety**: Rust의 타입 시스템으로 런타임 에러 방지
3. ✅ **Performance**: 연결 풀링, 인덱싱, 캐싱 전략
4. ✅ **Maintainability**: 명확한 관심사 분리와 테스트 가능한 설계
5. ✅ **Reliability**: 트랜잭션 지원과 에러 처리
6. ✅ **Scalability**: 백그라운드 워커와 하이브리드 스토리지 접근
7. ✅ **Zero Configuration**: 자동 데이터베이스 및 스키마 초기화
