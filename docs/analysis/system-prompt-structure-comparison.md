# System Prompt 구조 비교 분석: Legacy vs Agent V2

## 📋 분석 목적

Legacy Agent와 Agent V2의 System Prompt 전체 구조를 비교하여 섹션 배치, 정보 흐름, 우선순위 차이를 분석합니다.

---

## 🔍 Legacy System Prompt 구조

```text
1. Agent Identity & Strategy (상단)
   └─ "You are the Libr Assistant..."
   └─ Strategy 가이드라인

2. Assistant Context (메타 정보)
   └─ Assistant ID
   └─ Assistant Name

3. Built-in Tools 안내
   └─ 툴 사용 지침
   └─ "builtin_" prefix 경고

4. Service Contexts (도구별 상태) ⭐
   └─ Browser Sessions
   └─ Content Store
   └─ Workspace
   └─ Planning
   └─ Knowledge Base

5. Current Context Information (하단)
   └─ Date and Time
   └─ Location
```

**특징**:
- **도구 상태 우선**: 시간/위치 정보보다 도구 상태를 먼저 배치
- **실용적 순서**: 에이전트가 즉시 사용할 정보를 상단에 배치
- **간결한 컨텍스트 정보**: 시간/위치는 마지막에 간략히 표시

---

## 🆚 Agent V2 System Prompt 구조

```text
1. Current Context Information (상단) ⭐
   └─ Date and Time
   └─ Timezone
   └─ 설명: "This information is automatically updated..."

2. Agent Identity & Strategy
   └─ "You are the Libr Assistant..."
   └─ Strategy 가이드라인

3. Available Tools & Current State (하단)
   └─ Playbook
   └─ Content Store
   └─ Planning
   └─ Workspace
```

**특징**:
- **시간 정보 우선**: 시간/위치를 최상단에 배치
- **도구 상태 후순위**: 서비스 컨텍스트를 하단에 배치
- **장황한 설명**: 각 섹션마다 설명 추가

---

## 📊 섹션별 상세 비교

### 1. Agent Identity & Strategy

#### Legacy (상단 배치)

```text
You are the Libr Assistant: a general-purpose knowledge and automation agent.

Strategy:
- Analyze Intent: Upon receiving a request, deeply analyze the user's intent. Ask clarifying questions only if absolutely necessary.
- Plan & Execute: Always start by setting a goal and plan, then execute them systematically.
- Record Memories: Since memory is limited, periodically record your thoughts and important information.
- Think Deeper: If a problem becomes difficult, always take a step back and think deeper to find a solution.
```

**위치**: 1번 섹션 (최상단)  
**토큰**: ~80 tokens

#### Agent V2 (중간 배치)

```text
You are the Libr Assistant: a general-purpose knowledge and automation agent.

Strategy:
- Analyze Intent: Upon receiving a request, deeply analyze the user's intent. Ask clarifying questions only if absolutely necessary.
- Plan & Execute: Always start by setting a goal and plan, then execute them systematically.
- Record Memories: Since memory is limited, periodically record your thoughts and important information.
- Think Deeper: If a problem becomes difficult, always take a step back and think deeper to find a solution.
```

**위치**: 2번 섹션 (시간 정보 다음)  
**토큰**: ~80 tokens

**차이점**: 동일한 내용이지만 배치 위치가 다름

---

### 2. Assistant Context (메타 정보)

#### Legacy (상단 배치)

```text
# Assistant Context
- **Assistant ID**: d4xpngwchxuc4tiai4kwwmtf
- **Assistant Name**: Libr Assistant

*This identifier is provided for tooling/routing purposes.*
```

**위치**: 2번 섹션 (Identity 바로 다음)  
**토큰**: ~25 tokens

#### Agent V2

**❌ 없음** - V2에서는 Assistant Context 섹션이 제거됨

**분석**:
- Legacy: Multi-agent 환경 대비 (Assistant ID로 라우팅)
- V2: Single-agent 가정으로 단순화

---

### 3. Built-in Tools 안내

#### Legacy (중간 배치)

```text
# Available Built-in Tools

You have access to built-in tools for file operations, code execution, and web-based processing.
Tool details and usage instructions are provided separately.

**Important Instruction:** When calling built-in tools, you MUST use the tool name exactly as it appears in the available tools list. Do not add or remove the "builtin_" prefix - use it "as is".
```

**위치**: 3번 섹션 (Assistant Context 다음)  
**토큰**: ~50 tokens

#### Agent V2

**❌ 없음** - V2에서는 일반 툴 안내가 제거됨 (도구별 상태만 표시)

**분석**:
- Legacy: 명시적 툴 사용 지침 (특히 `builtin_` prefix 경고)
- V2: LLM이 자동으로 툴 사용법을 안다고 가정

---

### 4. Service Contexts (핵심 차이점!)

#### Legacy (중간 배치, 시간 정보보다 우선)

```text
## Browser Sessions
Session f227ac7b-c8ca-49cc-ae8a-d5feebd93b18: https://www.google.com/search?q=AI+datacenter+news+global+January+2+2026&tbm=nws (Untitled)

## Content Store
Active, 5 tools, no files

## Workspace
Active, 14 tools, dir: C:\Users\SKTelecom\AppData\Roaming\com.fritzprix.libragent\workspaces\xbnvpo8ft63fbodcrte8kg8p, 0 running processes, platform: windows/x86_64

## Planning
**Current Goal:** "2026년 1월 2일 기준 AI 데이터센터 관련 글로벌 주요 뉴스 5건 선정 및 심층 분석 리포트 작성"
*Goal is active. Track progress with todos below.*

**Todos:** 2 unchecked / 0 checked (2 total)
**Unchecked Items:**
  [0] ID:24 | 글로벌 뉴스 검색 | Priority:high
  [1] ID:25 | 주요 뉴스 5건 선정 | Priority:medium

## Knowledge Base
**No knowledge entries yet.**
*Use saveKnowledge to store important information for future reference.*
```

**위치**: 4번 섹션 (시간 정보보다 먼저!)  
**토큰**: ~200 tokens  
**순서**: Browser → Content Store → Workspace → Planning → Knowledge Base

#### Agent V2 (하단 배치, 시간 정보보다 후순위)

```text
## Available Tools & Current State

## Playbook
**Description**: Playbook management for reusable workflows

## Content Store
**Status**: No active session

## Planning
**Current Goal:** "AI 데이터센터 관련 주요 뉴스 (2026년 1월 2일 기준) 검색 및 심층 분석 리포트 작성"
*Goal is active. Track progress with todos below.*

**Todos:** 2 unchecked / 0 checked (2 total)
**Unchecked Items:**
  [0] ID:16 | 글로벌 뉴스 검색 | Priority:high
  [1] ID:17 | 뉴스 선정 | Priority:medium

## Workspace
**Directory**: C:\Users\SKTelecom\AppData\Roaming\com.fritzprix.libragent\workspaces\te3tyfel012tt2fx9y4ay2y6
**Running Processes**: 0
**Platform**: windows/x86_64
```

**위치**: 3번 섹션 (시간 정보 다음, 최하단)  
**토큰**: ~250 tokens  
**순서**: Playbook → Content Store → Planning → Workspace

**차이점**:
- Legacy: Browser 포함, Knowledge Base 별도 섹션
- V2: Browser 없음 (빈 문자열), Knowledge Base 없음
- Legacy: 도구 상태를 시간보다 우선
- V2: 시간 정보를 도구 상태보다 우선

---

### 5. Current Context Information (시간/위치)

#### Legacy (하단 배치, 간결)

```text
# Current Context Information

## Date and Time
- **Current Date**: Friday, January 2, 2026
- **Current Time**: 02:39:39 PM GMT+9

## Location
- **User Location**: Seoul, South Korea (Asia/Seoul)

*This information is automatically updated and provided to help you understand the user's current context.*
```

**위치**: 5번 섹션 (최하단)  
**토큰**: ~60 tokens  
**스타일**: 간결, 핵심 정보만

#### Agent V2 (상단 배치, 장황)

```text
# Current Context Information

## Date and Time
- **Current Date**: Friday, January 2, 2026
- **Current Time**: 14:06:34 +09:00
- **Timezone**: +09:00

*This information is automatically updated to help you understand the user's current temporal context.*
```

**위치**: 1번 섹션 (최상단!)  
**토큰**: ~70 tokens  
**스타일**: 더 상세 (Timezone 별도 필드), Location 제거

**차이점**:
- Legacy: 하단 배치 (도구 상태보다 후순위)
- V2: 최상단 배치 (모든 정보보다 우선)
- Legacy: Location 정보 포함 (Seoul, South Korea)
- V2: Location 정보 제거

---

## 🎯 핵심 차이점 분석

### 1. **정보 우선순위 철학의 차이**

#### Legacy 철학: "도구 중심" (Tool-First)

```
1. Who am I? (Identity)
2. What can I do? (Tools & States) ⭐ 핵심
3. When is it? (Time)
```

**논리**:
- 에이전트는 **즉시 행동**해야 함
- 현재 사용 가능한 **도구와 상태**가 가장 중요
- 시간 정보는 컨텍스트로 참고만

**장점**:
- ✅ 에이전트가 즉시 사용 가능한 정보 우선
- ✅ 실용적 순서 (행동 지향적)
- ✅ 토큰 효율적 (핵심부터 읽음)

#### Agent V2 철학: "시간 중심" (Time-First)

```
1. When is it? (Time) ⭐ 최우선
2. Who am I? (Identity)
3. What can I do? (Tools & States)
```

**논리**:
- 모든 작업은 **시간 컨텍스트**가 필요
- 시간 정보를 먼저 제공해야 올바른 판단
- 도구 상태는 참고 정보

**문제점**:
- ❌ 시간 정보는 대부분 작업에서 부차적
- ❌ 도구 상태가 하단에 묻힘
- ❌ 에이전트가 핵심 정보를 나중에 읽음

---

### 2. **섹션 배치 순서 비교**

| 순서 | Legacy | Agent V2 | 차이점 |
|------|--------|----------|--------|
| 1 | Agent Identity & Strategy | **Current Context (Time)** | ⚠️ V2는 시간을 최우선 |
| 2 | Assistant Context (ID) | Agent Identity & Strategy | V2에서 ID 제거 |
| 3 | Built-in Tools 안내 | **Tools & Current State** | V2에서 안내 제거 |
| 4 | **Service Contexts** | - | ⚠️ Legacy는 도구 우선 |
| 5 | Current Context (Time) | - | ⚠️ Legacy는 시간 후순위 |

**시각적 비교**:

```
Legacy:                          Agent V2:
┌─────────────────────┐         ┌─────────────────────┐
│ 1. Identity         │         │ 1. Time (상단!)     │ ⚠️
├─────────────────────┤         ├─────────────────────┤
│ 2. Assistant ID     │         │ 2. Identity         │
├─────────────────────┤         ├─────────────────────┤
│ 3. Tools 안내       │         │ 3. Tools & State    │
├─────────────────────┤         │    (하단!)          │ ⚠️
│ 4. Browser          │         └─────────────────────┘
│ 5. Content Store    │
│ 6. Workspace        │  ⭐ 핵심
│ 7. Planning         │
│ 8. Knowledge        │
├─────────────────────┤
│ 9. Time (하단)      │
└─────────────────────┘
```

---

### 3. **토큰 배치 효율성**

#### Legacy: "역피라미드" 구조

```
상단 (가장 중요) ───────┐
                       ├─ Identity (80 tokens)
                       ├─ Assistant Context (25 tokens)
                       ├─ Tools 안내 (50 tokens)
                       ├─ Service Contexts (200 tokens) ⭐
하단 (참고 정보) ───────┴─ Time (60 tokens)

총: ~415 tokens
```

**특징**: 핵심 정보(Service Contexts)를 중간에 배치, 즉시 접근 가능

#### Agent V2: "정피라미드" 구조

```
상단 (참고 정보) ───────┐
                       ├─ Time (70 tokens) ⚠️
                       ├─ Identity (80 tokens)
하단 (중요 정보) ───────┴─ Tools & State (250 tokens) ⭐

총: ~400 tokens
```

**특징**: 핵심 정보(Tools & State)를 하단에 배치, 접근성 저하

---

### 4. **에이전트 읽기 흐름 분석**

#### Legacy 읽기 흐름 (효율적)

```
1. "나는 Libr Assistant다" → Identity 확인
2. "내 ID는 d4xpngw..." → Multi-agent 라우팅 정보
3. "builtin_ 사용법 주의" → 툴 사용 가이드
4. "브라우저 세션: https://google.com/..." → 즉시 사용 가능! ⭐
5. "Content Store: Active, no files" → 즉시 사용 가능! ⭐
6. "Planning: Goal 있음, Todo 2개" → 즉시 작업 시작! ⭐
7. "현재 시간: 2026-01-02 14:06" → 참고 정보
```

**총 인지 시간**: 짧음 (핵심 정보가 앞에)

#### Agent V2 읽기 흐름 (비효율적)

```
1. "현재 시간: 2026-01-02 14:06" → 참고 정보 (불필요?) ⚠️
2. "나는 Libr Assistant다" → Identity 확인
3. "Playbook: Description만..." → 정보 부족 ⚠️
4. "Content Store: No active session" → 오류! ⚠️
5. "Planning: Goal 있음, Todo 2개" → 작업 시작
6. "Workspace: 디렉토리 정보" → 참고 정보
```

**총 인지 시간**: 김 (핵심 정보가 뒤에, 오류 있음)

---

## 💡 Legacy가 우수한 이유

### 1. **인지 부하 최적화**

**Legacy**: 에이전트가 즉시 필요한 정보부터 제공
```
Identity → Tools → Browser → Content Store → Planning → Time
```
→ "내가 누구고, 뭘 할 수 있고, 현재 상태는 어떤지" 순서대로

**Agent V2**: 참고 정보부터 제공
```
Time → Identity → Tools & State
```
→ "시간이 뭔지 알고, 내가 누구고, 그 다음 상태 확인"

### 2. **실용적 정보 배치**

**Legacy**: "지금 당장 사용할 정보"를 우선 배치
- Browser Session: 현재 열린 페이지 (즉시 작업 가능)
- Content Store: 파일 상태 (즉시 저장/검색 가능)
- Planning: 현재 Goal과 Todo (즉시 실행 가능)

**Agent V2**: "배경 정보"를 우선 배치
- Time: 대부분 작업에서 직접 사용 안 함
- Tools & State: 중요하지만 하단에 배치

### 3. **에러 처리 관점**

**Legacy**: 도구 상태를 먼저 읽음
- 문제 발견 시 즉시 대응 가능
- 예: "Content Store: Active, no files" → 정상 작동 확인

**Agent V2**: 시간부터 읽음
- 도구 오류는 나중에 발견
- 예: "Content Store: No active session" → 오류를 하단에서 발견

---

## 🎯 권장사항: Legacy 구조 차용

### Phase 1: 섹션 순서 재배치 (P0)

```rust
// src-tauri/src/agent/llm.rs:543
pub async fn build_system_prompt(...) -> Result<String, String> {
    let mut parts = Vec::new();

    // ❌ Before (V2 - 시간 우선)
    // parts.push(build_time_location_context());  // 1순위
    // parts.push(agent_config.system_prompt);     // 2순위
    // parts.push(service_contexts);               // 3순위

    // ✅ After (Legacy - 도구 우선)
    parts.push(agent_config.system_prompt);     // 1순위: Identity
    parts.push(service_contexts);               // 2순위: Tools & State ⭐
    parts.push(build_time_location_context()); // 3순위: Time

    Ok(parts.join("\n"))
}
```

**예상 출력**:
```text
You are the Libr Assistant: a general-purpose knowledge and automation agent.

Strategy:
- Analyze Intent: ...

## Available Tools & Current State

## Browser Sessions
Session f227ac7b: https://google.com/... (Google Search)

## Content Store
Active, 5 tools, no files

## Planning
**Current Goal:** "..."
**Todos:** 2 unchecked / 0 checked

## Workspace
Active, 14 tools, dir: ..., 0 running processes

# Current Context Information
## Date and Time
- **Current Date**: Friday, January 2, 2026
- **Current Time**: 14:06:34 +09:00
```

---

### Phase 2: Assistant Context 추가 (선택사항)

Multi-agent 환경 대비:

```rust
let mut parts = Vec::new();
parts.push(agent_config.system_prompt);

// Assistant Context 추가
parts.push(format!(
    "# Assistant Context\n- **Assistant ID**: {}\n- **Assistant Name**: {}",
    session.assistant_id,
    agent_config.name
));

parts.push(service_contexts);
parts.push(build_time_location_context());
```

---

### Phase 3: Built-in Tools 안내 추가 (선택사항)

```rust
parts.push(r#"
# Available Built-in Tools

You have access to built-in tools for file operations, code execution, and web-based processing.

**Important Instruction:** When calling built-in tools, you MUST use the tool name exactly as it appears in the available tools list.
"#);
```

---

## 📊 최종 권장 구조

```text
┌─────────────────────────────────┐
│ 1. Agent Identity & Strategy    │  80 tokens
├─────────────────────────────────┤
│ 2. Assistant Context (optional) │  25 tokens
├─────────────────────────────────┤
│ 3. Tools 안내 (optional)        │  50 tokens
├─────────────────────────────────┤
│ 4. Service Contexts ⭐           │ 200 tokens
│    - Browser (Legacy style)     │
│    - Content Store (Legacy)     │
│    - Workspace                  │
│    - Planning                   │
│    - Playbook (Planning style)  │
├─────────────────────────────────┤
│ 5. Current Context (Time)       │  60 tokens
└─────────────────────────────────┘

총: ~415 tokens (Legacy와 동일)
```

**특징**:
- ✅ Legacy의 검증된 구조
- ✅ 도구 상태 우선 (실용적)
- ✅ 시간 정보 후순위 (참고 정보)
- ✅ 에이전트 인지 부하 최소화

---

## 🔧 구현 체크리스트

### 섹션 순서 변경 (P0 - 즉시)

- [ ] `build_system_prompt()` 함수 수정
- [ ] Identity → Tools → Time 순서로 변경
- [ ] `build_time_location_context()` 호출을 마지막으로 이동

### Service Context 개선 (P1)

- [ ] Browser: Legacy 스타일 구현 (Session + URL + Title)
- [ ] Content Store: Legacy 스타일 변경 (Active, X tools, X files)
- [ ] Playbook: Planning 스타일 구현 (최근 3개 goal)

### 선택적 섹션 추가 (P2)

- [ ] Assistant Context 추가 (Multi-agent 대비)
- [ ] Built-in Tools 안내 추가 (명시적 가이드)

---

## 📈 예상 효과

### 인지 효율성

**Before (V2)**:
```
1. Time (70 tokens) → 참고 정보
2. Identity (80 tokens) → Identity 확인
3. Tools & State (250 tokens) → 핵심 정보 (하단!)
```
→ 핵심 정보 접근까지 150 tokens 소비

**After (Legacy 구조)**:
```
1. Identity (80 tokens) → Identity 확인
2. Tools & State (200 tokens) → 핵심 정보 (즉시!)
3. Time (60 tokens) → 참고 정보
```
→ 핵심 정보 접근까지 80 tokens 소비 (47% 개선)

### 에이전트 성능

- ✅ 즉시 사용 가능한 정보 우선 배치
- ✅ 도구 오류 조기 발견
- ✅ 불필요한 시간 정보 읽기 지연
- ✅ 에이전트 의사결정 속도 향상

---

## 🎓 결론

### Legacy 구조의 핵심 강점

1. **도구 중심 철학** (Tool-First)
   - 에이전트는 행동을 위한 존재
   - 사용 가능한 도구가 최우선 정보

2. **실용적 정보 배치**
   - Browser, Content Store, Planning을 중간에 배치
   - 시간 정보는 참고용으로 하단 배치

3. **인지 부하 최적화**
   - 핵심 정보부터 읽음
   - 불필요한 정보는 나중에

### Agent V2의 개선 방향

1. **섹션 순서 변경** (P0)
   - Time을 하단으로 이동
   - Service Contexts를 중간으로 상승

2. **Service Context 개선** (P1)
   - Browser: Legacy 스타일 구현
   - Content Store: 세션 인식 + Legacy 스타일
   - Playbook: Planning 스타일

3. **일관성 확보** (P1)
   - Legacy의 검증된 구조 차용
   - 각 도구별 적절한 상세도 선택

---

**분석일**: 2026-01-02  
**비교 대상**: Legacy Agent System Prompt vs Agent V2 System Prompt  
**핵심 발견**: Legacy의 "도구 우선" 구조가 에이전트 성능에 더 적합
