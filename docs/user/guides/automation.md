---
title: 자동화 (Scheduled Tasks)
description: 'Scheduled Tasks, loop·schedule 스킬, Org 자동화의 차이와 언제 무엇을 쓸지 정리합니다.'
---

# 자동화 — Scheduled Tasks

> 사이드바 **Scheduled Tasks**에서 관리합니다.  
> 정해진 시간에 **어시스턴트 세션을 자동 실행**합니다. 일회성 서브 에이전트 위임·Org 팀과는 별개입니다.

---

## 새 작업 만들기

1. **Scheduled Tasks** → **New task** (또는 **Create your first task**)
2. 처음 시작할 때는 화면에 제공되는 **Starter Templates**(데일리 스탠드업 요약, 코드 리뷰 다이제스트, **오래 쓸 지식 일일 증류** 등)를 클릭하여 기본 구성값과 스케줄을 즉시 채울 수 있습니다. Chat Hub 팁의 **일일 증류 작업 만들기**로도 동일 작업을 한 번에 만들 수 있습니다.
3. **New Scheduled Task**에서 필요한 항목을 조정합니다:

| 필드          | 설명                                                                     |
| ------------- | ------------------------------------------------------------------------ |
| **Task name** | 목록에 보일 이름                                                         |
| **Assistant** | 실행에 쓸 어시스턴트                                                     |
| **Schedule**  | 반복/시각 (UI 스케줄 컨트롤)                                             |
| **Workspace** | 선택. 특정 폴더에서 돌리려면 Browse 또는 폴더 드롭                       |
| **Message**   | 깨어날 때 보낼 프롬프트. `@playbook:`, `@skill:`, `@file:` 자동완성 가능 |

4. 저장합니다. 목록에 **Next run**이 표시됩니다.

비활성(disabled)으로 두면 스케줄이 돌지 않습니다.

---

## 편집 · 삭제

카드/목록에서 **Edit Task** / **Delete Task**.  
Settings의 **Scheduled Task Minimum Interval**이 켜져 있으면 너무 짧은 주기는 거부될 수 있습니다 (**Settings** 쪽 System/관련 가드 설정).

---

## 다른 자동화와 구분

| 수단                                                | 언제                                          |
| --------------------------------------------------- | --------------------------------------------- |
| **Scheduled Tasks** (이 페이지)                     | 앱 전역·반복 백그라운드 실행                  |
| **`@skill:loop`** ([안내](loop.md))                 | **지금 세션 안** 시계 기반 루프/리마인더/지연 |
| **`@skill:call-me-back`** ([안내](call-me-back.md)) | 프로세스·칸반·웹훅 **완료 신호**로 세션 재개  |
| **`@skill:schedule`**                               | 스케줄 운영 절차(에이전트용 스킬)             |
| **Org / teamwork**                                  | 명시적 팀 계보 — 사이드바 **Org**             |
| **delegate / divide-conquer**                       | 당장 자식 세션 위임                           |

자세한 멀티 에이전트: [서브 에이전트 · 오케스트레이션](sub-agents.md)

---

## 팁

- 메시지에 목표·산출물 형식을 구체적으로 적으세요.
- MCP가 필요하면 먼저 [Extensions](extensions.md)에 설치하고, 고른 **Assistant**의 Tools에서 허용하세요.
- 실패 시 **History**에서 해당 실행 세션을 열어 로그를 확인합니다.
- 작업에 **Workspace**를 지정하면 그 폴더에서 실행됩니다. 예약 작업의 Workspace를 비워 두어도, 이미 채팅에서 지정해 둔 작업 폴더는 그대로 유지됩니다.

---

## 관련

- [Assistants](assistants.md)
- [Playbooks](playbooks.md)
- [문제 해결](troubleshooting.md)
- [서브 에이전트](sub-agents.md)
