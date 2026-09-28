# 🚀 LibrAgent Project Guidelines (Gemini Agent Wrapper)

> [!IMPORTANT]
> 이 프로젝트의 통합 개발 컨벤션 및 가이드라인은 [agents.md](agents.md)를 기준으로 관리됩니다.
> Gemini 에이전트 구동 시 아래 지시 사항과 함께 `agents.md`의 규칙을 최우선으로 따르십시오.

## 핵심 참고 사항

- 통합 아키텍처 및 코딩 컨벤션은 [agents.md](agents.md) 파일에 자세히 명시되어 있습니다.
- 시스템 리소스 보호를 위해 무거운 전체 파이프라인인 `pnpm refactor:validate`는 사용자가 명시적으로 지시할 때만 실행하십시오.
- **🚫 Rust raw `cargo` 직접 실행 절대 금지**: `cargo test` / `cargo test --tests` 는 여러 ~600MB Tauri 테스트 바이너리를 한 번에 링크해 32GB 머신도 OOM 납니다. 항상 `pnpm rust:…` 래퍼를 쓰십시오:
  - 포맷: `pnpm rust:fmt:check`
  - 타입/컴파일: `pnpm rust:check`
  - 린트: `pnpm rust:clippy`
  - 전체 스위트(안전): `pnpm rust:test` — 타깃당 1개씩 순차 링크 (`run-rust-tests-sequential.cjs`)
  - 반복 작업: `pnpm rust:test --test <특정테스트타깃>`
- **🚫 코드 리뷰(`review local changes` 등) 시 무단 테스트 실행 절대 금지**: 리뷰 요청의 본질은 diff 기반 정적 분석(로직 버그, 설계 결함, 사이드 이펙트, 보안 검토)입니다. 사용자가 명시적으로 "테스트 돌려봐"라고 지시하기 전에 임의로 무거운 테스트나 빌드를 실행하지 마십시오. 검증이 필요하더라도 가벼운 정적 검사(`pnpm rust:fmt:check`, `pnpm rust:check`)까지만 선별 수행하십시오.

## 🚫 오버엔지니어링 & 불필요한 추상화 절대 금지 (KISS / YAGNI)

- **로컬 데스크톱 환경 망각 금지**: 이 프로젝트는 100만 동접 분산 웹서비스가 아닌 개인 로컬(Tauri + SQLite) 앱입니다. 수십~수백 개 수준의 데이터에 분산 시스템급의 과도한 계층 분리, 불필요한 이중 추상화를 절대 도입하지 마십시오.
- **Dual State / Dual ID 설계 절대 금지**: 토큰 몇 바이트 아끼겠다고 ID를 잘라내거나(Display vs Storage 분리), 불필요한 상태 이원화를 만들어 복잡한 역방향 매핑(reverse lookup) 및 불일치 버그를 유발하지 마십시오. 단일 고유 ID(SSOT) 원칙을 철저히 유지하십시오.
- **방어 코드 남발보다 단순한 구조 우선**: 복잡도를 키우는 수백 줄의 껍데기 래퍼, 뉴타입, 매핑 알고리즘보다 처음부터 문제가 생기지 않는 단순하고 직관적인 설계를 우선하십시오.
