# Docker Workspace 코드 리뷰 v2 (Updated)

**작성일:** 2026-07-05  
**상태:** P0/P1 해결 + 기술 부채 정리 완료

---

## Summary

Docker workspace isolation 변경사항은 **merge-ready** 상태입니다. P0 이슈, 컨텍스트 프롬프트, runtime 캐싱, 코드 중복 제거까지 반영되었습니다.

---

## ✅ 해결됨

| #   | 이슈                                              | 조치                                                          |
| --- | ------------------------------------------------- | ------------------------------------------------------------- |
| 1   | Windows `PathMappingLayer` 실패                   | `strip_prefix("/workspace/")` + 테스트                        |
| 2   | `runShell` / `spawnProcess` CWD 미포함            | `effective_command_cwd()` + JSON/텍스트 응답                  |
| 3   | 컨텍스트 프롬프트 호스트/컨테이너 혼란            | `build_workspace_live_state()` — Docker 시 `/workspace`       |
| 4   | `ensure_runtime()` 매 명령 healthcheck            | `prepare_docker_runtime()` + 30s health/container/shell cache |
| 5   | `ensure_supported_shell` 중복 호출                | shell type 캐시 + `create_docker_exec_command` 단일 resolve   |
| 6   | Docker 감지/CWD 로직 중복                         | `utils.rs` helpers + `context.rs` live state                  |
| 7   | Isolation fail-open (`Ok(None)`, DB Err fallback) | `session_isolation` fail-closed                               |
| 8   | Draft UI 중복                                     | `WorkspaceIsolationSettings` 공통 컴포넌트                    |

---

## 아키텍처 (정리 후)

```
Agent context/tools
    └── build_workspace_live_state() / effective_command_cwd()
            └── is_session_docker_isolated()

Shell execution (Docker)
    └── create_docker_exec_command()
            └── prepare_docker_runtime(cache_health=true, cache_container=true)
                    ├── DOCKER_HEALTH_CACHE (30s)
                    ├── DOCKER_CONTAINER_READY_CACHE (30s)
                    └── DOCKER_SHELL_CACHE
```

---

## 💡 향후 optional 개선

| #   | 제안                                                                     | Priority |
| --- | ------------------------------------------------------------------------ | -------- |
| 1   | Docker 모드에서 host/container 파일시스템 분리 — agent 문서화            | P2       |
| 2   | Windows 5-tool surface — tool description에 bash vs PowerShell 가이드    | P3       |
| 3   | `is_session_docker_isolated` → `Result` 반환 (context vs execution 정합) | P3       |

---

_리뷰 v2 업데이트: P1 캐싱 포함 전체 기술 부채 정리 반영._
