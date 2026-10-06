# ASD-STE100 (Simplified Technical English) Guide for LibrAgent

This reference outlines the core principles of ASD-STE100 applied to desktop and AI agent user interfaces.

## Core Rules

1. **Keep Sentences Short & Direct**:
   - Limit sentences to 20 words or fewer.
   - Use active voice. Never use passive voice where active is possible.
   - Example:
     - ❌ *Settings are saved automatically after they are changed.*
     - ✅ *LibrAgent saves changes automatically.*

2. **One Word, One Meaning**:
   - Standardize terminology across all screens.
   - Do not use interchangeable synonyms (e.g. do not mix `cancel`, `abort`, `halt`, `stop`—use `cancel` for active actions and `stop` for terminating loops).

3. **No Internal Developer Identifiers in UI**:
   - Never expose internal function names, backend flags, or code parameter syntax to end users.
   - ❌ `browser="userChrome"`, `browser="sidecar"`, `createSession`, `awaitAgent`, `pollProcess`
   - ✅ `Everyday Chrome`, `separate built-in browser`, `new conversation`, `long-running tool`

4. **Eliminate Developer & LLM Architecture Jargon**:
   - ❌ `Prefill Performance` → ✅ `Initial Response Speed`
   - ❌ `Time to First Token` → ✅ `Time to first response`
   - ❌ `old turns` → ✅ `older messages`
   - ❌ `clean resample` → ✅ `automatic retry`
   - ❌ `circuit breaker` → ✅ `stop task` or `stop workflow`
   - ❌ `serving-engine` → ✅ `model provider`
   - ❌ `diffs` → ✅ `file changes` or `code comparisons`
   - ❌ `params` → ✅ `parameters`

5. **Clarity & Actionability**:
   - Instructions must tell the user what to do or what happens in plain terms.
