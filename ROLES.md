# ROLES.md — LibrAgent 사업화 Task Force

## Coordinator (전략 기획자)

- **ID**: Master Mind (`8da332e5-4626-45b7-8551-1fb2830a7442`)
- **책임**: 전체 사업화 전략 통합, 우선순위 결정, 진행 상황 관리
- **허용 도구**: planning, knowledge, history, agent, skills, playbook, attachments, ui, scheduled_task, tool, media
- **필수 입력**: 모든 역할별 분석 결과
- **필수 출력**: `docs/BUSINESS_STRATEGY.md`, 진행 상황 업데이트
- **Handoff**: 각 역할별 분석 결과 통합 → 최종 전략 문서화

## Marketing Expert (마케팅 전문가)

- **ID**: Libr Assistant (`eef6cf3e-4b97-4640-9626-5a0f3fcea4ec`)
- **책임**: 홍보 채널 전략, 콘텐츠 마케팅, 브랜드 포지셔닝
- **허용 도구**: planning, scratchpad, workspace, agent, skills, playbook, attachments, ui, browser, scheduled_task, tool
- **필수 입력**: 제품 가치 제안, 타겟 고객 분석
- **필수 출력**: `docs/MARKETING_PLAN.md`, 홍보 자료 초안
- **Handoff**: 마케팅 전략 → 제품 전략가, 재무 분석가

## Product Strategist (제품 전략가)

- **ID**: Technical Expert (`c07ed980-9b67-465b-9a5a-cde8167b1cb9`)
- **책임**: 제품 포지셔닝, 기능 우선순위, 사용자 경험 개선
- **허용 도구**: planning, scratchpad, workspace, knowledge, history, agent, skills, playbook, attachments, ui, browser, scheduled_task, bootstrap, tool, media
- **필수 입력**: 경쟁사 분석, 사용자 피드백, 기술 트렌드
- **필수 출력**: `docs/PRODUCT_ROADMAP.md`, 기능 우선순위
- **Handoff**: 제품 로드맵 → 마케팅 전문가, 재무 분석가

## Financial Analyst (재무/투자 분석가)

- **ID**: Financial Analyst (`62d2c7eb-b15b-4469-a0d8-5fdcc975c58b`)
- **책임**: 수익 모델, 가격 정책, 투자 유치 전략, 재무 projections
- **허용 도구**: planning, scratchpad, workspace, knowledge, history, agent, skills, playbook, attachments, ui, browser, scheduled_task, bootstrap, tool, media
- **필수 입력**: 제품 로드맵, 마케팅 전략, 시장 규모
- **필수 출력**: `docs/REVENUE_MODEL.md`, `docs/INVESTOR_PITCH.md`, `docs/FINANCIAL_PROJECTION.md`
- **Handoff**: 재무 데이터 → 전략 기획자 통합

## Operating Rules

1. **Read First**: 모든 전문가는 `agents.md`, `MISSION.md`, `ROLES.md`를 먼저 읽음
2. **Check KANBAN**: 작업 시작 전 `coordination/KANBAN.md` 확인
3. **Claim Tasks**: 의미 있는 작업 전에 작업 claim 또는 업데이트
4. **Document Everything**: 발견사항 또는 상태 변경을 적절한 coordination 파일에 기록
5. **Handoff Explicitly**: 구체적인 handoff를 `coordination/HANDOFF.md`에 남김
6. **Blocked State**: Blocked 상태는 `KANBAN.md`에 기록
7. **Decisions**: 미결결정은 `DECISIONS.md`에 기록
8. **Risks**: 활성 리스크는 `RISKS.md`에 기록
