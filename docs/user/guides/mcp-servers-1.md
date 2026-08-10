---
title: MCP 서버 설정
---

# MCP 서버 설정

MCP(Model Context Protocol) 서버를 설정하면 AI 에이전트가 외부 도구와 서비스를 사용할 수 있습니다. LibrAgent는 **내장 MCP 서버**(Planning, Knowledge, Browser 등)와 **외부 MCP 서버**를 모두 지원합니다.

---

## 내장 MCP 서버

내장 서버는 설치 없이 바로 사용할 수 있습니다. 새 세션을 시작하면 자동으로 활성화됩니다:

| 서버              | 용도                    |
| ----------------- | ----------------------- |
| **Planning**      | 작업 계획 및 할당       |
| **Knowledge**     | 지식베이스 검색 및 저장 |
| **Browser**       | 웹 자동화 및 스크래핑   |
| **Workspace**     | 파일 시스템 작업        |
| **Content Store** | 콘텐츠 저장 및 검색     |

내장 서버는 Settings나 Extensions에서 활성화할 필요가 없습니다.

---

## 외부 MCP 서버 추가

외부 MCP 서버는 **Extensions** 패널에서 관리합니다.

### 1. Extensions 열기

사이드바에서 **Extensions**를 클릭합니다.

### 2. 서버 추가

**Add Extension** 버튼을 클릭하고 다음 중 하나를 선택합니다:

#### 📦 권장 확장 프로그램

LibrAgent가 테스트하고 검증한 확장 프로그램 목록에서 선택합니다. 가장 안정적인 옵션입니다.

#### 🔧 커스텀 MCP 설치

npm 패키지, GitHub 리포지토리, 또는 직접 설정 JSON을 입력합니다.

**설치 방식:**

| 방식        | 설명                  | 예시                                          |
| ----------- | --------------------- | --------------------------------------------- |
| **npm/npx** | npm 패키지로 설치     | `npx @modelcontextprotocol/server-filesystem` |
| **GitHub**  | GitHub 리포지토리 URL | `github:example/mcp-server`                   |
| **HTTP**    | HTTP URL로 연결       | `https://example.com/mcp`                     |
| **stdio**   | 로컬 실행 파일        | `python -m mcp_server`                        |

### 3. 서버 활성화

설치 후 서버가 Extensions 목록에 나타납니다. 서버명을 클릭하면 상태를 확인할 수 있습니다.

---

## MCP 서버 구성

### API 키 설정

일부 MCP 서버는 API 키가 필요합니다:

1. **Settings** → **AI & Models** 탭
2. 해당 Provider 카드에서 **API Key**에 키 입력
3. **Save Changes** 클릭

### 연결 상태 확인

Extensions 패널에서 각 서버의 상태 표시기를 확인합니다:

- ✅ **연결됨** — 정상 사용 가능
- ⚠️ **경고** — 설정 확인 필요
- ❌ **연결 실패** — 구성 재설정 필요

---

## MCP 서버 문제 해결

| 증상                 | 해결 방법                          |
| -------------------- | ---------------------------------- |
| 서버가 연결되지 않음 | API 키, Base URL, 설치 경로 확인   |
| 도구가 표시되지 않음 | 서버 재시작 후 Extensions 새로고침 |
| 응답이 느림          | 서버 로딩 상태 확인, 네트워크 점검 |
| 권한 오류            | 실행 권한 확인 (Unix: `chmod +x`)  |

자세한 문제 해결은 [트러블슈팅 가이드](troubleshooting.md)를 참조하세요.

---

## 관련 문서

- [Extensions 관리](extensions.md) — 권장 확장 프로그램 목록
- [커스텀 MCP 설치](custom-mcp.md) — 상세 설치 방법
- [API 응답 스키마](https://github.com/fritzprix/libr-agent/blob/main/docs/mcp/API_RESPONSE_SCHEMA_FOR_USER_ACTIVATED_MCPs.md) — 개발자용 API 레퍼런스
- [Rust MCP 마이그레이션](https://github.com/fritzprix/libr-agent/blob/main/docs/mcp/RUST_MCP_CONFIG_MIGRATION_STRATEGY.md) — 개발자용 구현 가이드
