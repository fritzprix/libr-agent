---
marp: true
theme: default
style: |
  section {
    font-size: 18px;
    padding: 40px;
  }
  h1 {
    color: #2c3e50;
    border-bottom: 3px solid #3498db;
    padding-bottom: 10px;
  }
  h2 {
    color: #34495e;
    margin-top: 30px;
  }
  .highlight {
    background-color: #f39c12;
    color: white;
    padding: 2px 6px;
    border-radius: 3px;
  }
  .quote {
    font-style: italic;
    background-color: #ecf0f1;
    padding: 15px;
    border-left: 4px solid #3498db;
    margin: 10px 0;
  }
  .warning {
    background-color: #e74c3c;
    color: white;
    padding: 10px;
    border-radius: 5px;
  }
---

# Headless Internet과 MCP 기술 트렌드 분석

## Browser 중심 AI Agent 통합 vs AI Agent 중심 Headless Internet

### 글로벌 AI 서비스 회사의 전략적 방향성

---

# 1. 현재 상황: AI 에이전트의 부상

## 인터넷 인터페이스의 패러다임 전환

- **AI 에이전트가 새로운 인터페이스로 부상**
  - ChatGPT: 10억 사용자 (역사상 가장 빠른 성장)
  - Perplexity: Pro 사용자 대상 쇼핑 기능 제공
  - OpenAI: 2025년 Q3 ChatGPT 직접 결제 기능 통합 예정

- **기존 웹 브라우징의 변화**
  - 클릭 중심 → 대화 중심 인터페이스
  - 정적 웹사이트 → 동적 AI 상호작용
  - 사람 중심 UI → 에이전트 중심 API

<div class="highlight">핵심: 모든 것이 Headless로 전환되고 있음</div>

---

# 2. Headless Internet 등장 배경

## 정보 고립과 레거시 시스템의 한계

### 기존 문제점
- **N×M 통합 문제**: 각 데이터 소스마다 개별 커넥터 필요
- **정보 사일로**: AI 모델이 데이터로부터 격리됨
- **파편화된 통합**: 확장 가능한 연결 시스템 부재

### 해결책의 필요성
- **표준화된 프로토콜** 요구
- **양방향 보안 연결** 필요
- **범용 인터페이스** 개발

<div class="quote">
"AI 시스템이 진정으로 연결된 시스템이 되기 어려웠던 이유"
- Anthropic MCP 문서
</div>

---

# 3. MCP 기술의 폭발적 성장

## 2024년 11월 → 2025년 8월: 생태계 형성

### 채택 현황
- **2024년 11월**: Anthropic MCP 오픈소스 공개
- **2025년 초**: 1,000개+ MCP 서버 개발
- **2025년 3월**: OpenAI 공식 채택
- **2025년 4월**: Google DeepMind 지원 발표
- **2025년 5월**: Microsoft Windows 11 통합

### 주요 기업 참여
- **Early Adopters**: Block, Apollo, Replit, Codeium, Sourcegraph
- **개발 도구**: Zed, VS Code, GitHub
- **클라우드**: AWS, Azure, Cloudflare

<div class="highlight">"AI의 USB-C 포트" - 범용 표준으로 자리잡음</div>

---

# 4. 업계 리더들의 관점과 우려

## 찬성 vs 경계

### 지지 입장

<div class="quote">
<strong>Sam Altman (OpenAI CEO)</strong><br>
"People love MCP and we are excited to add support across our products. [It's a] step toward standardizing AI tool connectivity"
</div>

<div class="quote">
<strong>Demis Hassabis (Google DeepMind CEO)</strong><br>
"MCP is rapidly becoming an open standard for the AI agentic era"
</div>

### 우려 표명

<div class="quote">
<strong>Steve Lucas (Boomi CEO)</strong><br>
"There's nothing that would stop a model from reverse-engineering many functions of the application"
</div>

<div class="warning">거대 기술 기업의 지배력 강화 우려</div>

---

# 5. Browser 중심 vs AI Agent 중심 접근법

## 두 가지 발전 경로

### Browser 중심 AI Agent 통합
**현재 접근법**
- 전용 AI 브라우저 (Dia, Fellou, Sigma AI Browser)
- 헤드리스 브라우저 (Browserbase, Playwright)
- 기존 웹 인프라 + AI 기능 추가

**장점**: 호환성, 사용자 친숙도, 점진적 전환
**한계**: 레거시 의존성, "DMV는 MCP 서버를 갖지 않을 것"

### AI Agent 중심 Headless Internet
**새로운 패러다임**
- 웹사이트 우회, 직접 API 거래
- MCP 통한 직접 데이터 접근
- 브랜드의 챗봇 인터페이스 압축

**혁신 가능성**: 효율성, 자동화, 맞춤화
**위험 요소**: 브랜딩 상실, 새로운 보안 취약점

---

# 6. 판매자들의 딜레마와 선택지

## 아마존 딜레마의 재현

### 판매자가 직면한 선택
1. **ChatGPT 플랫폼 의존**
   - 10억 사용자 접근 vs 통제권 상실
   - 브랜딩, 교차판매, 고객 충성도 위험

2. **헤드리스 인프라 개방**
   - 제3자 접근 허용 vs 통제권 유지
   - 단일 플랫폼 의존성 감소

### 필요한 기술적 요구사항
- **/catalog**: 상품 검색, 메타데이터
- **/cart**: 상태 저장 세션 관리
- **/checkout**: 토큰 기반 결제 (핵심 누락 부분)
- **/post-order**: 주문 추적, 배송 상태

<div class="highlight">핵심: 리다이렉트 없는 토큰 기반 체크아웃 API</div>

---

# 7. 글로벌 AI 서비스 회사의 전략적 과제

## 기회와 위험의 양면성

### 기회 요소
- **새로운 유통 채널**: AI 에이전트 네트워크
- **효율성 증대**: 자동화된 고객 상호작용
- **맞춤형 서비스**: 컨텍스트 기반 개인화

### 위험 요소
- **플랫폼 종속성**: 새로운 중간자 의존
- **브랜드 정체성 희석**: 챗봇 인터페이스 압축
- **보안 취약점**: Cross-Prompt Injection, Tool Poisoning

### 전략적 고려사항
- 기존 비즈니스 모델 보호 vs 혁신 수용
- 통제권 유지 vs 시장 접근성
- 단기 수익 vs 장기 경쟁력

---

# 8. 단계별 실행 방안

## 단기 전략 (2025-2026): 하이브리드 접근

### 기술적 준비
- **MCP 서버 인프라 구축**
  - 핵심 비즈니스 도메인 우선
  - 기존 API 래핑 및 표준화
- **보안 프레임워크 강화**
  - Cross-Prompt Injection 방어
  - Authentication Gap 해결

### 비즈니스 준비
- **AI 거버넌스 구축**: IBM 권고사항 적용
- **파트너십 확대**: 주요 AI 플랫폼과 협력
- **고객 관계 보호**: 기존 채널 유지 전략

---

# 9. 중장기 전략 (2027-2030)

## Headless-First 아키텍처로의 전환

### 기술적 전환
- **API-First 설계**: GUI는 여러 인터페이스 중 하나
- **토큰 기반 결제**: 자율 결제 시스템 준비
- **AI 에이전트 파트너십**: B2B 거래 최적화

### 비즈니스 모델 혁신
- **새로운 수익 구조**: 웹 트래픽 의존성 탈피
- **가치 제안 재정의**: 브랜드 → 기능 중심
- **생태계 참여**: MCP 표준 기여 및 영향력 확보

### 위험 관리
- **데이터 주권 확보**: 자체 MCP 서버 운영
- **다중 플랫폼 전략**: 단일 의존성 방지
- **브랜드 보호**: MCP-UI 표준 활용

---

# 10. 결론 및 향후 전망

## 변화의 물결에 대한 대응

### 현실 인식
- **MCP 혁명은 이미 시작됨**: 1,000개+ 서버, 주요 기업 채택
- **판매자의 딜레마는 현실**: ChatGPT vs 헤드리스 개방
- **2025년은 실험의 해**: 대규모 상용화는 2026-2027년

### 핵심 전략 방향
1. **점진적 전환**: 기존 모델 보호하며 새 패러다임 준비
2. **표준 참여**: MCP 생태계 적극 참여 및 영향력 확보
3. **차별화 추진**: 보안, 거버넌스, UX에서 경쟁 우위

### 성공 요소
<div class="highlight">
과도한 엔지니어링 없이 현실적이고 단계적인 접근
</div>

**"조기 채택자들이 미래 AI 애플리케이션 형성에 상당한 이점을 가질 것"**