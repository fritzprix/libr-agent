# 🚀 LibrAgent Project Guidelines (Claude CLI Wrapper)

> [!IMPORTANT]
> 이 프로젝트의 통합 개발 컨벤션 및 가이드라인은 [agents.md](file:///c:/Users/SKTelecom/my_works/libr-agent/agents.md)를 기준으로 관리됩니다.
> Claude CLI 사용 시 아래 지시 사항과 함께 `agents.md`의 규칙을 최우선으로 따르십시오.

## 핵심 참고 사항

- 통합 아키텍처 및 코딩 컨벤션은 [agents.md](file:///c:/Users/SKTelecom/my_works/libr-agent/agents.md) 파일에 자세히 명시되어 있습니다.
- 시스템 리소스 보호를 위해 무거운 전체 파이프라인인 `pnpm refactor:validate`는 사용자가 명시적으로 지시할 때만 실행하십시오. 평상시 검증은 변경된 파일 대상의 가벼운 단위/통합 테스트나 린트만 선별하여 실행하십시오.
