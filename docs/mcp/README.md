# MCP Integration (Developer Documentation)

This directory contains **developer-facing** documentation for MCP (Model Context Protocol) integration in LibrAgent.

## Architecture

- **MCPServiceProxy** — session-isolated proxy that routes tool calls to builtin or external servers
- **HttpSessionManager** — per-session HTTP manager for external MCP servers
- **BuiltinMCPServer** trait — interface for native Rust MCP server implementations
- **SessionMCPManager** — manages builtin server instances per session

## Files

| File                                             | Description                                    |
| ------------------------------------------------ | ---------------------------------------------- |
| `MCP_ACTIVATION_API_GUIDE.md`                    | Rust API for user-activated MCP servers        |
| `API_RESPONSE_SCHEMA_FOR_USER_ACTIVATED_MCPs.md` | Response schema for user-activated MCP servers |
| `MCP_CONFIG_COMPARISON_ANALYSIS.md`              | Config format comparison (old vs new)          |
| `RUST_MCP_CONFIG_MIGRATION_STRATEGY.md`          | Migration strategy from old to new config      |
| `builtin_tools_migration_report.md`              | Builtin tool migration status                  |
| `claude-channels-dev-team-announcement.md`       | Team announcement for Claude Channels project  |
| `claude-channels-implementation-status.md`       | Implementation status tracker                  |
| `claude-channels-dev-task-assignment.md`         | Task assignment details                        |
| `claude-channels-mcp-server-reference.md`        | MCP server reference (implementation details)  |
| `migration_status_update_v4.md`                  | Migration status v4                            |

## User-Facing Documentation

For end-user MCP documentation, see:

- [MCP 서버 설정](../user/guides/mcp-servers.md) — 연결 및 사용 가이드
- [Extensions 관리](../user/guides/extensions.md) — 확장 프로그램 관리
- [커스텀 MCP 설치](../user/guides/custom-mcp.md) — 커스텀 서버 설치
