## 2025-05-24 - [Workspace/Shell] **Threat:** Privilege Escalation / Sandbox Escape **Mitigation:** Enforcing Session Isolation on Persistent Shells

The `runInPersistentShell` tool (backed by `PersistentShell`) was found to bypass the `SessionIsolationManager`, executing shell processes with the full environment and permissions of the host application. This could allow an attacker (or malfunctioning agent) to access sensitive environment variables, escape the workspace directory, or execute commands outside the intended sandbox.

**Mitigation Plan:**
1. Refactor `SessionIsolationManager` to support spawning interactive shells with isolation wrappers (namespaces, sandbox profiles, env clearing).
2. Update `PersistentShell` to use `SessionIsolationManager` for process creation, ensuring it inherits the same security posture as one-shot commands (`runShell`).
3. Enforce "High" or "Medium" isolation levels on persistent shells where supported.
