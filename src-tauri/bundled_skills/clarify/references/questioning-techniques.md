# Questioning Techniques

## Question Priority Order

Work from critical to cosmetic. Ask about a lower tier only when the tier above is stable:

1. **Scope & Purpose**: What problem? What is in, what is out? What does done look like?
2. **People**: Who uses, attends, reads, or approves the result? What are the top 3 scenarios?
3. **Core Activities**: What are the main functions, phases, or steps? What happens when everything goes well?
4. **Deliverables & Materials**: What is produced, and what is needed to produce it? For software only: inputs, outputs, stored data, formats, integrations.
5. **Constraints**: Budget, schedule, people, tools, venue, platform, compliance.
6. **Edge Cases & Failure Handling**: What happens when things go wrong?
7. **Non-Goals Confirmation**: Read back the non-goals and confirm nothing important is missing.

## Techniques

### Funnel questioning
Start with one broad open question ("이 일이 성공했다고 판단하는 순간은 언제인가요?"), then narrow with multiple choice until each item is a concrete, testable statement.

### Contrast pairs
When the user is vague, offer two contrasting interpretations: "A안: X에 초점 / B안: Y에 초점 — 어느 쪽인가요?" This forces a decision instead of another vague answer.

### Assumption surfacing
State your assumption and ask for a correction: "~라고 이해했는데 맞나요? 틀렸다면 어떻게 다른가요?" This is cheaper than an open question and shows a misunderstanding fast.

### Non-goal flip
For an in-scope item, sometimes ask the reverse: "반대로 이건 안 하는 게 맞나요?" Ambiguous requirements hide in what was never excluded.

### Example walk-through
Ask the user to narrate one concrete case from start to end: "참석자가 도착해서 떠날 때까지 어떤 순서로 진행되나요?" A concrete story shows missing requirements faster than abstract questions.

## Round Structure (per turn)

1. One-line summary of what the previous round decided.
2. 2–4 questions, each answerable in one sentence; multiple choice where possible.
3. After the answers: update the doc, report the change ("v0.3 — R3, R4 확정"), then continue.

## Example Sequences

### Software feature
- Round 1 (Scope): "이 기능의 핵심 목표는?" / "데스크톱만? 모바일도?" / "1차 출시에서 빼도 되는 것은?"
- Round 2 (People): "주 사용자는 누구인가요?" / "가장 자주 쓰는 시나리오를 한 문장으로?"
- Round 3 (Core activities): "A 상황에서 사용자가 보아야 하는 결과는?" / "B와 C 중 어떤 동작이 맞나요?"
- Round 4 (Deliverables & data): "어떤 데이터를 저장해야 하나요?" / "기한이나 기술 스택 제약은?"
- Round 5 (Edge cases): "입력이 비어 있거나 잘못됐을 때는?" / "여러 사용자가 동시에 쓰면?"

### Event
- Round 1 (Scope): "행사의 목적은? (네트워킹 / 교육 / 축하)" / "규모는 몇 명인가요?"
- Round 2 (People): "누가 참석하고, 누가 진행하나요?" / "외부 연사나 손님이 있나요?"
- Round 3 (Constraints): "날짜와 예산 상한은?" / "장소는 정해졌나요?"
- Round 4 (Edge cases): "우천이나 불참이 많을 때 대안은?"

### Report or document
- Round 1 (Scope): "누가 읽고, 읽은 뒤 무엇을 결정하나요?" / "분량과 형식은? (1쪽 요약 / 상세 보고서)"
- Round 2 (Deliverables & materials): "꼭 들어가야 할 항목은?" / "사용할 자료나 데이터 출처는?"
- Round 3 (Constraints): "마감일과 검토자는?"

### Work process
- Round 1 (Scope): "이 프로세스가 해결할 문제는?" / "시작과 끝은 어디인가요?"
- Round 2 (People): "단계마다 담당자는 누구인가요?" / "승인이 필요한 단계는?"
- Round 3 (Edge cases): "단계가 지연되거나 반려되면 어떻게 하나요?"

## Anti-patterns

- More than 4 questions in one turn.
- Asking about cosmetics (wording, colors) before the scope is stable.
- Asking again about anything already recorded in the doc.
- Recording the user's answer word for word as a requirement. Restate it as a testable statement and confirm it.
