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

## 관련

- [첫 대화](../getting-started/first-agent.md)
- [Assistants](assistants.md)
- [서브 에이전트](sub-agents.md)
- [문제 해결](troubleshooting.md)
