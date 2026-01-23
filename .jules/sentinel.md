# SENTINEL'S JOURNAL - SECURITY AUDIT LOG

Before starting, read `.jules/sentinel.md`. Only add entries when:
- A Critical/High severity vulnerability (CVSS > 7.0) is patched.
- A hardcoded secret was found and rotated.
- An unsafe dependency was identified and replaced.

Format: ## YYYY-MM-DD - [Module] **Threat:** [Vulnerability Type] **Mitigation:** [Action Taken]

## 2025-05-19 - [Session Isolation] **Threat:** Denial of Service (Panic on invalid path) **Mitigation:** Replaced `unwrap()` with error handling in macOS sandbox path resolution to prevent crashes with non-UTF8 paths.
