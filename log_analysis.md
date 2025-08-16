## 요구사항 체크리스트

- [ ] 각 `logs/app_*.log` 파일을 하나씩 처리한다.
- [ ] 각 파일에서 발견된 WARN/ERROR 패턴을 식별하고 우선순위화한다.
- [ ] 각 경고 메시지를 발생시킨 소스 파일(파일 경로 및 관련 코드 위치)을 찾아 연결한다.
- [ ] 각 경고에 대해 근본 원인(root cause)과 재현 방법을 서술한다.
- [ ] 가능한 코드 수준의 권고(간단한 패치 또는 개선 방안)를 제시한다.
- [ ] 결과를 `log_analysis.md`에 상세히 기록한다.

---

## 요약(우선순위)

1. Schema 파싱 관련 WARN: `Failed to parse JSON schema` (높음, 다수 발생)
2. Tool 매핑/이름 누락 WARN: `Could not find function name for tool message with tool_call_id: ...` (높음)
3. Tool 메시지에 `tool_call_id` 누락 WARN: `Tool message missing tool_call_id` (중간)
4. Gemini sequencing 관련 제거 WARN: `Removing assistant function call that violates Gemini sequencing rules` (중간)
5. Geolocation 실패: `Failed to get geolocation {}` (낮음)

---

## 분석 템플릿 (각 로그 파일별로 아래 항목을 작성)

- 파일: `logs/app_N.log`
- 요약: WARN/ERROR 개수, 상위 메시지
- 문제 발생 샘플(발생 위치와 주변 5줄)
- 매핑된 소스 파일 및 위치(파일 경로 + 설명)
- 근본 원인 추정
- 권고 및 잠정 패치 제안

---

## 사례 분석 1 — `logs/app_13.log`

- 파일: `logs/app_13.log` (lines: 201; WARN:6; ERROR:0)
- 상위 WARN 메시지:
  - `Could not find function name for tool message with tool_call_id: <id>` (여러 ID)
  - `Removing assistant function call that violates Gemini sequencing rules {...}`

샘플 컨텍스트 (로그에서 발췌):
- 로그에서는 모델 호출 및 도구(tool) 목록이 포함된 Gemini 요청/응답이 보입니다. 모델에서 반환된 tool 메시지에 대해 `tool_call_id`가 포함되어 있으나, 이 ID를 사용해 선언된 도구 목록에서 일치하는 함수 이름을 찾지 못해 경고가 찍힙니다.

매핑된 소스 위치:
- `src/lib/ai-service/gemini.ts` — 경고 문자열(`Could not find function name for tool message with tool_call_id`)이 이 파일의 약 277행에서 찍힙니다. 해당 코드는 수신된 모델 응답 내 `tool_calls` 또는 `tool` 메시지를 처리하면서, 이전에 전송된 도구 선언 목록(`toolCalls` 등)과 `tool_call_id`로 매칭을 시도합니다.
- 관련 발신 쪽 코드: `src/features/chat/ToolCaller.tsx` (tool_call_id 생성/전달), `src/lib/ai-service/openai.ts` 및 `ollama.ts` (tool_call_id를 메시지로 전파) 역시 연관됩니다.

근본 원인(가설):
- 모델 응답에 포함된 `tool_call_id`가 클라이언트/서버 양쪽에서 일관되게 관리되지 못해, 매칭이 실패합니다. 원인으로는 다음이 있을 수 있습니다:
  1. 도구 선언을 전송할 때 내부 ID(예: `toolCall.id`)가 모델에게 전달되는 방식과 응답에서 사용하는 ID가 불일치.
  2. 모델 내부 재시도/재구성 과정에서 `tool_call_id`가 변형되거나 잘려서 돌아옴.
  3. 모델 응답이 스트리밍 도중 중간 조각으로 들어왔고, 완전한 `tool_calls` 배열이 아직 누적되지 않은 상태에서 매칭 로직이 실행됨(경합 조건).

권고(단기):
- 매칭 로직을 방어적으로 개선: `tool_call_id`가 발견되어도 일치하는 항목이 없으면(1) 로그에 더 많은 진단 정보(예: 모델 응답 전체의 tool_calls 목록, 전송한 도구 목록 요약)를 포함시키고, (2) 잠시 대기/누적 후 재시도 검토.
- 도구 선언 시 `id` 필드의 생성과 전달 경로를 점검해 일관된 UUID 사용을 강제.

권고(중장기, 코드수정 제안):
- `src/lib/ai-service/gemini.ts`의 매칭 함수에 다음 개선을 적용:
  - 매칭 실패 시 `tool_call_id` 대신 도구 이름으로도 추적할 수 있게 보조 매칭 로직 추가.
  - 스트리밍 응답 처리 시, `tool_calls` 배열의 완전성(종료 표시 또는 finish_reason)을 확인한 후 매칭을 시도.

---

## 사례 분석 2 — Schema 파싱 경고 (예: 일부 `app_*.log`에서 다수 발견)

- 관련 로그 메시지: `Failed to parse JSON schema: missing field `type`, using default` (여러 파일에서 발견)

관찰된 매핑(레퍼런스):
- `src-tauri/src/mcp.rs` 라인 약 386에서 `warn!("Failed to parse JSON schema: {e}, using default");` 호출이 존재합니다 — 이는 Rust 백엔드가 수신한 JSON schema를 파싱하다가 실패했음을 의미합니다.
- 프런트엔드/변환기 위치: `src/lib/ai-service/tool-converters.ts`에는 다양한 `sanitizeSchemaForCerebras` 및 변환 도구가 있어, 도구 `inputSchema`를 프로바이더별 요구사항에 맞춰 변형합니다. 이 과정에서 필수 필드(`type` 등)가 누락되거나 구조가 변형되어 백엔드 파서가 실패할 가능성이 있습니다.

근본 원인(가설):
- `MCPTool.inputSchema`의 소스(사용자 제공 또는 자동 생성) 중 일부가 `type` 필드를 누락한(또는 `type`이 배열/variant 형태로 제공된) 경우. 변환 함수가 `items`나 `properties`를 다루는 과정에서 `type`을 제거하거나 올바르게 채우지지 못함.
- `sanitizeSchemaForCerebras` 또는 유사 함수가 `items`가 배열인 스키마를 단순화하는 과정에서 `type`이 제거되거나 `sequence` 형태가 전달되어 후단 파서가 이를 인식하지 못함.

재현 방법(권장):
- 문제를 재현하려면, 로그에서 해당 WARN가 찍힌 `app_*.log` 블록에서 전송된 `tools` 블록(도구 선언 JSON)을 찾아 그 `inputSchema`를 복사한 뒤, Rust `mcp.rs`에서 사용하는 파서(또는 동일한 파서)를 사용해 파싱 시도를 해보면 원인을 확인할 수 있습니다.

권고(단기):
- `tool-converters.ts`의 `sanitizeSchemaForCerebras` 및 `convertMCPToolToProviderFormat` 함수들에 방어적 검증 추가:
  - 최종적으로 provider에 전달하기 전에 `inputSchema.type`이 누락되어 있거나 유효하지 않으면 `type: 'object'`(또는 적절한 기본값)를 설정.
  - `items`가 배열인 경우 첫 번째 항목을 사용하되, `type`이 확실치 않으면 기본 `type: 'string'` 등을 병기.
- Rust 백엔드(`src-tauri/src/mcp.rs`) 쪽에서 경고와 함께 문제가 된 원본 스키마(또는 그 요약)를 로그로 남겨 원인 추적을 쉽게 한다.

권고(중장기):
- 스키마 변환 파이프라인에 스키마 유효성 검사(스키마-스키마 검사)를 도입하여 전송 전에 문제를 검출하고 거부하거나 자동 보정하도록 한다.

---

## 사례 분석 3 — `logs/app_13.log`

- 파일: `logs/app_13.log` (lines: ~420)
- 주요 WARN: `Could not find function name for tool message with tool_call_id: <id>`

문제 개요:

- 로그에서 `tool` 메시지에 `tool_call_id`가 포함되어 도구 결과가 전송되었으나, `gemini.ts`의 변환 로직에서 해당 `tool_call_id`에 매칭되는 assistant-side `tool_calls`를 찾지 못해 경고를 찍었습니다.
- 동일 타임프레임에 Gemini provider의 `functionDeclarations`와 도구 목록이 포함된 최종 요청이 보입니다 — 이는 도구 선언 자체는 정상 전달되었음을 시사합니다.

원인(가설):

- 도구 결과 메시지가 도착한 시점에, in-memory 메시지 배열에서 대응하는 assistant `tool_calls` 항목이 아직 누적되지 않았거나, 클라이언트에서 생성한 optimistic id와 MCP/tauri에서 반환한 canonical id가 불일치합니다.

추가 증거와 위치:

- 매핑된 소스: `src/lib/ai-service/gemini.ts` (`convertToGeminiMessages`) — 도구 메시지 매칭 로직과 경고 포인트
- 관련 코드: `src/features/chat/ToolCaller.tsx` (tool_call_id 생성/전달), `src-tauri` MCP 브릿지(tauri 커맨드)에서 실제 tool call id가 어떻게 기록되는지 점검 필요

권고(단기):

1. 지금 적용한 진단 로깅을 통해, 경고가 다시 발생할 때 로그에 `recent_assistant_tool_call_ids`와 `tool_content_snippet`가 기록됩니다. 이를 바탕으로 ordering 문제인지 id-space 문제인지 빠르게 판별할 수 있습니다.
2. `ToolCaller.tsx`에서 optimistic id를 생성한 경우, MCP가 반환하는 canonical id를 받아온 뒤 전파/동기화하는 로직을 추가하세요. 또는 optimistic id를 사용하지 않고 서버-주도 id만 활용하도록 변경 검토.

권고(중장기):

1. 매칭 실패 시 보조 매칭 로직(fallback by function name or by scanning recent assistant tool_calls)을 도입하되, 이는 best-effort으로만 사용해야 합니다.
2. 메시지 흐름에 correlation id(예: 요청-응답 트레이스 id)를 추가해 end-to-end 추적을 용이하게 한다.

재현 팁:

1. 로컬에서 유사한 tool 호출 시퀀스를 재현하고, 도구 결과 메시지를 아주 짧은 지연(또는 바로)로 전송해 순서/동기화 실패를 시도해 보세요.
2. 적용된 진단 로그를 확인해 어떤 assistant-side tool call ids가 주변에 존재하는지 확인합니다.

상태: 분석 추가 및 gemini 진단 로그 개선 적용됨(코드 변경 커밋 포함).

---

## 사례 분석 3 — `logs/app_4.log`

- 파일: `logs/app_4.log` (lines: ~400; 여러 `Failed to parse JSON schema` WARN 포함)
- 상위 WARN 메시지:
  - `Failed to parse JSON schema: missing field `type`, using default`
  - `Failed to parse JSON schema: invalid type: sequence, expected variant identifier, using default`

문제 발생 샘플(로그에서 발췌):
```
[tauri_mcp_agent_lib::mcp][WARN] Failed to parse JSON schema: missing field `type`, using default
[tauri_mcp_agent_lib::mcp][WARN] Failed to parse JSON schema: invalid type: sequence, expected variant identifier, using default
```

매핑된 소스 파일 및 위치:
- `src-tauri/src/mcp.rs` — 경고 출력 지점(약 라인 360-390). 이 모듈은 외부 MCP 서버로부터 수신된 도구 선언의 `input_schema`를 Rust 내부 JSONSchema 타입으로 파싱하는 부분입니다.
- `src/lib/ai-service/tool-converters.ts` — 프런트엔드에서 도구 `input_schema`를 프로바이더 별 형식으로 변환/정제하는 책임. 이 파일의 `sanitizeSchemaForCerebras`, `convertMCPToolToProviderFormat` 함수들이 의심됩니다.

근본 원인 추정:
- 전송된 `input_schema` 객체에 `type` 필드가 없거나, `type` 필드가 배열/시퀀스 형식으로 제공되어 Rust 파서가 기대한 형태(문자열 식별자)를 받지 못함.
- 프런트엔드 변환기(`tool-converters.ts`)가 `items`나 `properties`를 처리하면서 `type`을 제거하거나, `type`을 `['string', 'number']`와 같은 배열로 남겨두어 Rust쪽 파서가 `sequence`로 해석하게 된 것으로 보입니다.

재현 단계(빠른 확인):
1. `logs/app_4.log`에서 문제 발생 직전의 `Raw tools response` 블록을 복사.
2. 해당 도구의 `input_schema` JSON을 `src-tauri`의 스키마 파서에 전달(테스트 유닛 또는 작은 Rust 스니펫)하여 동일한 파싱 경고가 발생하는지 확인.

권고 및 잠정 패치 제안:
- 프런트엔드 방어적 조치 (권장, 빠름):
  - `src/lib/ai-service/tool-converters.ts`에 다음 전처리 로직을 추가합니다:
    - 최종 전달 전 `input_schema.type`이 없으면 적절한 기본값(`object` 또는 `string`)을 설정하도록 변환 로직 수정.
    - `items`가 배열인 경우, 첫 번째 항목의 `type`을 확인해 유효하지 않으면 기본값을 설정.
    - `properties`가 있는 경우, 각 속성의 `type` 필드가 유효한지 검사하고, 누락된 경우 기본값 설정.
- Rust 백엔드 (장기적 개선):
  - `src-tauri/src/mcp.rs`에서 JSON 스키마 파서 개선:
    - `type` 필드가 누락되었을 때 기본값을 자동으로 설정하도록 수정.
    - 배열/시퀀스 형태의 `type`을 받을 경우, 첫 번째 항목의 타입을 기준으로 삼거나 경고 로그를 남기도록 수정.

