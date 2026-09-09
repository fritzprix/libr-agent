---
title: 세션
---

# 세션 관리하기

> 대화 단위가 **세션**입니다. 시작·북마크·삭제·검색은 사이드바 **Chat** / **History** / **Bookmarked**에서 합니다.

---

## 새 세션 시작

1. 사이드바 **Chat**
2. **Built-in Assistants** 또는 **My Assistants** 카드 클릭
3. 제목 **New Session** 초안에서 메시지 전송

「+ New Session」 단독 버튼으로 시작하지 않습니다.

---

## 북마크

중요한 세션을 빠르게 다시 열려면:

1. **History** 세션 카드의 북마크 아이콘을 켭니다.
2. 사이드바 **Bookmarked**, 또는 History의 bookmarked 필터로 모읍니다.

---

## 삭제

- 자식(하위 에이전트)이 **없으면**: 확인 후 해당 세션만 삭제.
- 자식이 **있으면**:
  - **Delete all** (+N subagents) — 부모와 하위 함께 삭제
  - **Delete only this** (Subagents kept) — 부모만 삭제, 자식은 유지

복구되지 않습니다. 하위 에이전트 개념: [서브 에이전트](sub-agents.md)

---

## 검색 · 재개

**History**에서 이름/ID 검색. 카드를 열어 이어서 대화합니다.  
최근 전환한 세션은 메모리에 웜(Warm) 상태로 유지되어 세션 간 이동 시 대화 뷰가 재로드 지연 없이 즉시 전환됩니다.  
세션 중 Provider/Model은 Chat 피커로 **현재 세션만** 변경 가능합니다.

---

## 내보내기

채팅 헤더 또는 History 카드의 **Export** 메뉴로 저장된 대화를 파일로 저장합니다.

- **Markdown (.md)** — 저장소의 전체 대화(thinking, tool call 포함). compaction/recovery 스캐폴딩은 제외합니다.
- **ATIF trajectory (.json)** — 분석용 Harbor 정렬 ATIF-v1.7. 스크린샷 등 바이너리 미디어는 원문이 아니라 placeholder만 넣습니다.
- **보이는 대화 복사** (활성 채팅만) — 메모리에 있는 창을 클립보드로 복사합니다. thinking은 빠지고 용량 제한이 있으며, Markdown 파일 내보내기와 같지 않습니다.

진행 중인 세션도 그 시점의 스냅샷으로 내보낼 수 있습니다. 아직 스트리밍 중인 행은 제외됩니다.

---

## 관련

- [첫 대화](../getting-started/first-agent.md)
- [Assistants](assistants.md)
- [서브 에이전트](sub-agents.md)
- [문제 해결](troubleshooting.md)
