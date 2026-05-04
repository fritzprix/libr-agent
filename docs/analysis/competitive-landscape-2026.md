# Competitive Landscape 2026

이 문서는 기존 루트 문서 `competetion_landscape.md.md`의 핵심 내용을 정리해  
`docs/analysis/` 아래로 옮긴 **경쟁 구도 요약 문서**다.

초점은 2026년 기준 AI 시장이 왜 **모델 경쟁**에서 **하니스 경쟁**으로 이동했는지,  
그리고 그 안에서 LibrAgent가 어떤 위치를 갖는지다.

---

## 1. 시장 변화: 이제 핵심은 모델이 아니라 하니스다

2026년의 핵심 변화는 명확하다.

- 기반 모델의 품질 차이는 여전히 중요하지만
- 실제 작업 성공률, 장기 실행, 안정성, 도구 사용 능력, 세션 지속성은
- **모델 자체보다 하니스(실행 환경, 컨텍스트 관리, 도구 계층, orchestration)**에 훨씬 크게 좌우된다

요약하면:

> **모델은 엔진이고, 하니스는 실제 주행 시스템이다.**

에이전트가 여러 턴에 걸쳐 상태를 유지하고, 도구를 사용하고, 검증하고, 복구하는 능력은 전부 하니스에서 결정된다.

---

## 2. 경쟁군별 특징

### OpenClaw 류

강점:

- 자유도 높음
- 오픈소스 생태계 빠르게 성장
- 다양한 채널과 스킬 기반 확장성

한계:

- 보안/거버넌스 리스크가 큼
- 신뢰할 수 없는 스킬 실행 위험
- 엔터프라이즈 운영 관점에서 부담이 큼

### Claude Cowork / Claude Code 류

강점:

- 세련된 사용자 경험
- 강한 코드/지식 작업 생산성
- agent team, local VM 등 선도적 시도 존재

한계:

- Cowork는 폐쇄 생태계
- Claude Code는 개발자 중심
- 병렬 팀 orchestration은 강하지만 비용/토큰 폭증 문제가 존재

### Operator / Mariner 류

강점:

- GUI 자동화 및 클라우드 병렬 실행
- 매우 강한 자동화 상상력

한계:

- 클라우드/브라우저 중심
- 사용자의 로컬 실행 환경과는 거리감이 있음
- 데이터 통제권과 실행 기저 환경 통제에서 제약이 있음

### LangGraph / Pydantic AI / CrewAI 류

강점:

- 프레임워크 유연성
- 상태 제어, 타입 안전성, 다중 에이전트 프로토타이핑에 장점

한계:

- 제품이 아니라 프레임워크
- 사용자가 직접 조립해야 함
- 실전용 UI/운영/배포 경험은 별도로 해결해야 함

---

## 3. 2026년 시장 요구

경쟁 문서 전반에서 드러난 요구는 아래와 같다.

1. **세션 지속성**
2. **도구 사용과 검증 루프**
3. **multi-agent orchestration**
4. **권한/보안/거버넌스**
5. **표준화된 외부 연결 계층(MCP)**
6. **긴 세션 가시성과 디버깅**

즉, 단순한 LLM 래퍼나 예쁜 챗 UI는 더 이상 충분하지 않다.

---

## 4. LibrAgent의 포지션

LibrAgent는 이 시장에서 다음 위치를 가진다.

### 1. local-first desktop harness

- 로컬 실행 환경
- Rust 백엔드
- Tauri 기반 데스크톱
- workspace/browser/shell/knowledge가 실제 substrate로 결합

### 2. MCP-native product

- MCP를 단순 연동 기능이 아니라 핵심 인프라로 사용
- builtin + external MCP를 같은 구조 안에서 다룸
- registry / preset / metadata / transport 흐름을 제품 차원에서 운영

### 3. single agent에서 swarm/org까지 이어지는 제품

- `delegate`
- `teamwork`
- `org`
- `schedule`

이 흐름이 한 제품 안에서 이어진다.

### 4. 안정성 지향형 하니스

- context compaction
- loop prevention
- circuit breaker
- stale response 대응
- session isolation
- runtime sequencing 개선

즉, 기능 추가보다 **장시간 운용**을 더 신경 쓴 구조다.

---

## 5. LibrAgent의 경쟁 우위 요약

### 1. 보안과 자유도를 동시에 잡으려는 포지션

OpenClaw류의 자유도는 매력적이지만 보안/운영 리스크가 크다.  
LibrAgent는 local-first + validation + session isolation으로 이 문제를 더 진지하게 다룬다.

### 2. 프레임워크가 아니라 바로 쓸 수 있는 제품

LangGraph/CrewAI는 강력하지만 직접 조립해야 한다.  
LibrAgent는 프레임워크의 유연성 일부를 제품화된 UX와 함께 제공한다.

### 3. agent orchestration의 성장 경로가 자연스럽다

단일 agent → specialist 생성 → delegate → teamwork → org → schedule 흐름이 있다.  
이건 시장에서 꽤 드문 조합이다.

### 4. 로컬 환경과 실제 작업에 더 가깝다

클라우드 VM이나 브라우저 클릭 자동화만으로는 부족한 경우가 많다.  
LibrAgent는 실제 파일, 셸, 브라우저, 지식 저장, 세션 상태를 묶어 **실제 작업 환경에 밀착**한다.

---

## 6. 결론

2026년의 승부처는 더 큰 모델이 아니라 **더 나은 하니스**다.

LibrAgent의 강점은 아래 세 문장으로 정리할 수 있다.

1. **로컬 중심의 실제 실행 환경을 제공한다**
2. **MCP 기반 확장성을 제품 차원에서 운영한다**
3. **단일 agent에서 swarm/org까지 자연스럽게 확장된다**

즉, LibrAgent는 단순한 AI 앱이 아니라  
**하니스 시대에 맞는 Agent Operating System 포지션**을 노릴 수 있는 제품이다.
