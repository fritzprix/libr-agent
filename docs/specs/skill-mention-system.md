# Chat Input Command & Reference Framework

채팅 입력창에서 두 가지 trigger를 통해 **Reference 주입**과 **Command 실행**을 지원하는 확장 가능한 framework.

## Trigger 구분

| Trigger      | 타입      | 의미론                        | 동작                                      |
| ------------ | --------- | ----------------------------- | ----------------------------------------- |
| `@type:path` | Reference | "이 내용을 컨텍스트에 포함해" | Late Binding — LLM 전송 직전 내용 prepend |
| `/command`   | Command   | "이 동작을 실행해"            | 즉시 실행 — message 전송 전 처리          |

**두 trigger를 분리하는 근거:**

- **관례**: `/command`는 Slack/Discord/Claude Code 등에서 이미 확립된 UX 패턴. 머글 친화적.
- **의미론적 명확성**: `@`는 "무언가를 참조한다", `/`는 "무언가를 실행한다" — 성격이 근본적으로 다름.
- **내부 구현은 단일 엔진**: `InputToken` 인터페이스로 통일, popup 컴포넌트도 재사용. 외부 UX만 구분.

## `@reference` — Content Injection (Late Binding)

`@`로 시작하는 reference는 **치환하지 않는다**. 원본 텍스트가 DB에 그대로 저장되고 UI에도 그대로 표시된다. Rust 백엔드의 LLM 전송 파이프라인(`agent/llm.rs`)에서 매번 파싱하여 해당 내용을 user message 앞에 prepend한다. 완전 **idempotent** — 스킬/파일이 업데이트되면 다음 호출부터 자동으로 최신 내용이 반영된다. 존재하지 않는 reference는 조용히 무시한다.

여러 reference를 한 메시지에 동시에 사용할 수 있다: `@skill:docx 참고해서 @file:src/main.ts 분석해줘`

**기본 제공 resolver:**

- `@skill:name` — 스킬 파일 내용 (`~skills/name.md`)
- `@file:path` — 워크스페이스 파일 내용 (보안 검증 필요)
- `@tool:tool_name` — 특정 도구에 대한 soft attention hint 주입 (강제 아님)
  - inject 내용: `<tool-hint>The "{name}" tool is available and may be relevant. {description}</tool-hint>`
  - AI가 이미 tool list를 알고 있지만 우선순위/주의를 높이는 signal
  - 강제 실행이 필요하면 미래에 `/use-tool:name` command로 별도 구현 가능

**확장 예정:**

- `@web/url` — URL 페이지 내용 fetch
- `@db/table` — 로컬 DB 쿼리 결과

### Syntax

```
@skill:skill_name
@file:path/to/file
```

> separator는 `:` 사용 (경로 구분자 `/`와 혼동 없음).  
> `@` 입력 시 popup에 `skill:`, `file:` 등 타입이 `:` 포함된 상태로 표시되어 사용자가 syntax를 자연스럽게 학습.

### `@file:` 파일 탐색 UX — Progressive Search Depth

파일 수가 많을 경우를 대비해 **query 길이에 비례하여 search scope를 확장**하는 방식을 채택.

- 항상 최대 **10개**만 popup에 표시 (candidate가 많아도 overflow 없음)
- query 길이가 짧을수록 → 짧고 shallow한 경로 우선 match
- query 길이가 길어질수록 → 전체 경로 + 긴 파일명까지 fuzzy match 확장

```
"@file:ma"      (2글자) → main.ts, manager.rs  (루트 근처, 짧은 이름)
"@file:main"    (4글자) → src/main.ts, AgentMain.tsx  (중첩 경로 포함)
"@file:AgentSes" (8글자) → src/features/agent/AgentSession.tsx  (깊고 긴 이름)
```

VS Code Ctrl+P 스타일: 입력이 짧으면 near match만, 길어질수록 전체 트리 fuzzy search.  
candidate pool을 query 길이에 비례해 넓히되 표시는 항상 10개로 고정.

## `/command` — Immediate Action (One-shot)

`/`로 시작하는 command는 message 전송 전 즉시 처리된다. DB에는 저장되지 않는다 (또는 system message로 기록).

Command는 인자 유무에 따라 두 종류로 구분:

| 종류           | 예시                 | popup                            | 동작              |
| -------------- | -------------------- | -------------------------------- | ----------------- |
| **No-arg**     | `/clear`, `/compact` | command 목록만                   | 선택 즉시 실행    |
| **Arg-select** | `/model`             | command 선택 → 인자 목록 (2단계) | 인자 선택 후 실행 |

**`/model` UX:**

- `/model` 선택 → provider 그룹핑된 모델 목록 popup
- "claude" 타이핑 → Anthropic 모델만 filter
- 선택 → 현재 session 모델 변경 (머글이 모델 ID 외울 필요 없음)

**`/compact` 동작:**

- 현재 대화 히스토리 → LLM으로 요약 → 요약본 하나의 system message로 교체
- UI: "Context compacted" 표시 후 이전 메시지들 collapse

**기본 제공 command:**

- `/clear` — 현재 session 대화 초기화 (no-arg)
- `/compact` — 대화 요약 후 context 압축 (no-arg)
- `/model` — provider/model 선택 popup → session 모델 변경 (arg-select)

**확장 예정:**

- 사용자 정의 command (slash command registry)

## 내부 구현 설계 — 단일 엔진, 이중 실행 경로

외부 UX는 `@`/`/`로 구분되지만, 내부 구현은 단일 `InputToken` 인터페이스로 통일.

```rust
// 모든 token type이 구현하는 공통 인터페이스
trait InputTokenHandler: Send + Sync {
    fn type_name(&self) -> &str;          // "skill", "file", "model", "clear" 등
    // popup candidate 제공 (None이면 인자 없는 command)
    async fn candidates(&self, query: &str, ctx: &SessionContext) -> Option<Vec<Candidate>>;
}

// Reference 전용 trait (Late Binding)
trait ReferenceHandler: InputTokenHandler {
    async fn resolve(&self, arg: &str, ctx: &SessionContext) -> Option<String>;
}

// Command 전용 trait (즉시 실행)
trait CommandHandler: InputTokenHandler {
    async fn execute(&self, arg: Option<&str>, ctx: &mut SessionContext);
}
```

등록만 하면 popup UI + 실행이 자동으로 따라온다. 새 타입 추가 = 새 struct 하나 구현 + registry에 push.

## 프론트엔드 설계

- `@` 입력 시 → `skill:`, `file:` 등 타입 목록 표시 (`:` 포함된 상태로 보여줌)
- 타입 선택 후 → 해당 타입의 항목을 query 길이 기반 progressive search로 filter, 최대 10개 표시
- `/` 입력 시 → command 목록 popup
- Popup은 동일한 overlay 컴포넌트가 trigger와 소스 데이터만 바꿔서 재사용
- 원본 텍스트 유지 (Late Binding), UI에 `@skill:docx` 그대로 표시
