# 🚀 LibrAgent 사업화 추진 Task Force

## Mission

LibrAgent를 수익성 있는 사업으로 성장시키기 위한 전략 수립 및 실행 계획 수립

## Work Shape

Integration-heavy — 전략, 마케팅, 제품, 재무가 유기적으로 연결된 종합 사업화 계획

## Artifacts

- `docs/BUSINESS_STRATEGY.md` — 종합 사업화 전략
- `docs/COMPETITIVE_ANALYSIS.md` — 경쟁사 분석 및 차별화 포인트
- `docs/PRODUCT_ROADMAP.md` — 제품 로드맵 (단기/중기/장기)
- `docs/REVENUE_MODEL.md` — 수익 모델 및 가격 정책
- `docs/MARKETING_PLAN.md` — 마케팅 채널 및 콘텐츠 전략
- `docs/INVESTOR_PITCH.md` — 투자 유치용 피치덱 초안
- `docs/FINANCIAL_PROJECTION.md` — 재무 projections

## Roles

### Coordinator (전략 기획자)

- **책임**: 전체 사업화 전략 통합, 우선순위 결정, 진행 상황 관리
- **입력**: 모든 스펙트럼의 연구 및 분석 결과
- **출력**: `docs/BUSINESS_STRATEGY.md`, 진행 상황 업데이트
- **handoff**: 각 역할별 분석 결과 통합 → 최종 전략 문서화

### Marketing Expert (마케팅 전문가)

- **책임**: 홍보 채널 전략, 콘텐츠 마케팅, 브랜드 포지셔닝
- **입력**: 제품 가치 제안, 타겟 고객 분석
- **출력**: `docs/MARKETING_PLAN.md`, 홍보 자료 초안
- **handoff**: 마케팅 전략 → 제품 전략가, 재무 분석가

### Product Strategist (제품 전략가)

- **책임**: 제품 포지셔닝, 기능 우선순위, 사용자 경험 개선
- **입력**: 경쟁사 분석, 사용자 피드백, 기술 트렌드
- **출력**: `docs/PRODUCT_ROADMAP.md`, 기능 우선순위
- **handoff**: 제품 로드맵 → 마케팅 전문가, 재무 분석가

### Financial Analyst (재무/투자 분석가)

- **책임**: 수익 모델, 가격 정책, 투자 유치 전략, 재무 projections
- **입력**: 제품 로드맵, 마케팅 전략, 시장 규모
- **출력**: `docs/REVENUE_MODEL.md`, `docs/INVESTOR_PITCH.md`, `docs/FINANCIAL_PROJECTION.md`
- **handoff**: 재무 데이터 → 전략 기획자 통합

## Coordination Model

**Hub-and-Spoke** — 전략 기획자(Coordinator)가 허브 역할, 다른 3명이 스포크

## Execution Substrate

**Explicit Org Lineage** — `agent__createOrg()`로 org 생성 후 org-visible child sessions 사용

## Refresh Semantics

- `agents.md` 변경사항은 다음 실행 단계부터 적용됨
- 새 workspace skill은 같은 턴에서 즉시 적용되지 않음
- 업데이트된 규칙은 명시적으로 고지된 후 다음 실행부터 적용

## Operating Rules

1. 모든 전문가는 `agents.md`, `MISSION.md`, `ROLES.md`를 먼저 읽음
2. 작업 시작 전 `coordination/KANBAN.md` 확인
3. 의미 있는 작업 전에 작업 claim 또는 업데이트
4. 발견사항 또는 상태 변경을 적절한 coordination 파일에 기록
5. 구체적인 handoff를 `coordination/HANDOFF.md`에 남김
6. Blocked 상태는 `KANBAN.md`에 기록
7. 미결결정은 `DECISIONS.md`에 기록
8. 활성 리스크는 `RISKS.md`에 기록
