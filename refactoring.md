# 🔧 Native Tools Integration 구현 계획

SynapticFlow에 외부 MCP 서버 없이 바로 사용 가능한 Native Tools 시스템을 구축하는 단계적 계획입니다.

## 🎯 목표

- Google Drive, Slack, Gmail 등 주요 서비스와의 OAuth 기반 연동
- Chat UI에서 직관적인 도구 활성화/비활성화 인터페이스
- 확장 가능한 플러그인 아키텍처
- 안전한 토큰 관리 및 자동 갱신

---

## 📋 구현 단계

### Phase 1: 기반 아키텍처 구축

#### 1.1 ChatTools 컴포넌트 Chat.tsx에 추가

**목표**: Chat UI에 Native Tools 관리 메뉴 통합

**작업 내용**:

- `src/features/chat/ChatTools.tsx` 컴포넌트 생성
- Chat.tsx의 ChatStatusBar에서 Tools 메뉴 클릭 시 ChatTools 표시
- 각 도구별 토글/상태 표시 UI 구성
- ToolsModal 내부에 ChatTools 통합

**파일 수정**:

- `src/features/chat/Chat.tsx` - ChatStatusBar에 ChatTools 통합
- `src/features/chat/ChatTools.tsx` - 새로 생성
- `src/features/tools/ToolsModal.tsx` - ChatTools 섹션 추가

**예상 코드 구조**:

```tsx
// ChatTools.tsx
export default function ChatTools() {
  return (
    <div className="space-y-2">
      <h3 className="text-sm font-semibold">Native Tools</h3>
      {/* 각 도구별 컴포넌트들이 여기에 렌더링 */}
    </div>
  );
}
```

---

#### 1.2 OAuth Context Provider & Hook 구현

**목표**: 서비스 인증 토큰의 중앙 집중식 관리

**작업 내용**:

- IndexedDB 스키마에 oauth_sessions 테이블 추가
- OAuthContext Provider 구현
- useOAuth Hook 제공
- 토큰 만료/갱신 로직 구현
- 앱 시작 시 저장된 세션 자동 복원

**파일 수정**:

- `src/lib/db.ts` - OAuthSession 인터페이스 및 CRUD 추가
- `src/context/OAuthContext.tsx` - 새로 생성
- `src/app/App.tsx` - OAuthProvider로 래핑
- `src/models/oauth.ts` - OAuth 관련 타입 정의 (새로 생성)

**데이터 구조**:

```typescript
interface OAuthSession {
  id: string;
  serviceType: 'google-drive' | 'slack' | 'gmail';
  accessToken: string;
  refreshToken?: string;
  expiresAt?: Date;
  userInfo?: { email: string; name: string; avatar?: string };
  createdAt: Date;
  updatedAt: Date;
}
```

---

#### 1.3 Tauri OAuth Framework 통합

**목표**: 안전한 OAuth 인증 플로우 구현

**작업 내용**:

- Rust 백엔드에 OAuth 인증 명령어 구현
- 브라우저 팝업 기반 OAuth 플로우
- PKCE (Proof Key for Code Exchange) 보안 적용
- 각 서비스별 OAuth 설정 관리

**파일 수정**:

- `src-tauri/src/oauth.rs` - 새로 생성
- `src-tauri/src/lib.rs` - oauth 모듈 및 명령어 등록
- `src-tauri/Cargo.toml` - OAuth 관련 의존성 추가
- `src/lib/tauri-oauth.ts` - 프론트엔드 OAuth 클라이언트 (새로 생성)

**Rust 의존성 추가**:

```toml
[dependencies]
oauth2 = "4.4"
reqwest = { version = "0.11", features = ["json"] }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

---

### Phase 2: Google Drive Integration 구현

#### 2.1 Google Drive Tool 기본 구조

**목표**: 첫 번째 Native Tool로 Google Drive 연동 구현

**작업 내용**:

- `src/tools/google-drive/` 폴더 구조 생성
- GoogleTool React 컴포넌트 (OAuth 상태 + 토글)
- GoogleDriveService LocalService 구현
- 기본 도구: 파일 목록, 업로드, 다운로드

**파일 생성**:

- `src/tools/google-drive/index.ts` - 엔트리 포인트
- `src/tools/google-drive/GoogleTool.tsx` - UI 컴포넌트
- `src/tools/google-drive/GoogleDriveService.ts` - Service 구현
- `src/tools/google-drive/types.ts` - Google Drive 관련 타입

**기본 도구 목록**:

- `listFiles` - 파일/폴더 목록 조회
- `uploadFile` - 파일 업로드
- `downloadFile` - 파일 다운로드
- `createFolder` - 폴더 생성

---

#### 2.2 Google API 클라이언트 구현

**목표**: Google Drive API와의 실제 통신 구현

**작업 내용**:

- Google Drive API v3 클라이언트 구현
- 파일 메타데이터 처리
- 에러 처리 및 재시도 로직
- Rate limiting 대응

**파일 생성**:

- `src/lib/google-api/drive-client.ts` - Drive API 클라이언트
- `src/lib/google-api/auth.ts` - Google OAuth 헬퍼
- `src/lib/google-api/types.ts` - Google API 타입 정의

---

### Phase 3: 확장 및 최적화

#### 3.1 추가 서비스 통합

**작업 내용**:

- Slack API 연동 (`src/tools/slack/`)
- Gmail API 연동 (`src/tools/gmail/`)
- 각 서비스별 독립적인 OAuth 설정

#### 3.2 성능 최적화 및 UX 개선

**작업 내용**:

- 토큰 자동 갱신 백그라운드 작업
- API 호출 캐싱 전략
- 로딩 상태 및 에러 메시지 개선
- 사용자 설정 UI (API 키 관리 등)

---

## 🔄 각 단계별 검증 포인트

### Phase 1 완료 후

- [ ] ChatTools가 Chat UI에서 정상 표시
- [ ] OAuth Context가 앱 전역에서 사용 가능
- [ ] 테스트용 OAuth 플로우 동작 확인

### Phase 2 완료 후

- [ ] Google Drive OAuth 인증 성공
- [ ] 기본 파일 작업 (목록, 업로드, 다운로드) 정상 동작
- [ ] Chat에서 Google Drive 도구 활성화/비활성화 가능

### Phase 3 완료 후

- [ ] 다중 서비스 동시 사용 가능
- [ ] 토큰 갱신 및 에러 복구 정상 동작
- [ ] 확장 가능한 아키텍처 검증

---

## 🚨 주의사항

1. **보안**: OAuth 토큰은 반드시 암호화하여 저장
2. **타입 안전성**: TypeScript `any` 사용 금지, 모든 API 응답 타입 정의
3. **로깅**: `getLogger`를 통한 체계적 로깅 적용
4. **에러 처리**: 네트워크 오류, API 제한 등 예외 상황 대비
5. **사용자 경험**: OAuth 인증 실패 시 명확한 안내 메시지 제공

---

*이 계획은 구현 과정에서 세부사항이 조정될 수 있습니다.*
