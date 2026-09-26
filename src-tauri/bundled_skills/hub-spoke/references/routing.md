# Spoke-to-Spoke Routing Protocol

Worker sessions (Spokes) must never communicate with each other directly. All messaging must be routed and filtered by the Coordinator (Hub).

## 📌 Routing Principles

1. **Indirect Link (Isolation)**
   - Spokes do not know about other Spokes. Each Spoke only responds to the Hub session.
   
2. **Context Filtering**
   - The Hub must not forward raw logs from Spoke A to Spoke B. 
   - Instead, the Hub extracts the core specs and artifact file paths, presenting a clean summary to Spoke B.

3. **Bridge Routing**
   - When Spoke A finishes, the Hub catches the event and triggers Spoke B with `agent__messageToSession`.
   - **SHARED** workspace (same Hub root / `workspaceOverride`): Hub may pass relative paths (e.g. "Spoke A finished — read `output_A.json` and proceed").
   - **ISOLATED** spokes: Hub cannot `workspace__readFile` a child Metadata path, and Spoke B cannot read Spoke A's private path either — Hub must bridge **Result text** (or ask A via `messageToSession` for the needed excerpt) into B's instructions.
   - Before starting a new spoke for later work, inspect the live child inventory and reuse an Idle spoke with the same `assistantId` when its workspace is compatible. Use `reset=true` for a genuinely fresh assignment; otherwise preserve the existing conversation.

---

## 📋 Routing Scenario

A translation workflow coordinated by the Hub (assumes SHARED workspace / `workspaceOverride` so spokes share the Hub root):

```
[Hub] "Decompose README.md translation and glossary document compilation."
  │
  ├─► [Spoke A (Translator)] Spawn: "Translate README.md to Korean."
  │     └─► Response: "Finished. Output at README.ko.md"  (Result text + relative path)
  │
  ├─► [Filter] Hub extracts key vocabulary from A's Result (or SHARED path).
  │
  └─► [Spoke B (QA Doc Writer)] Spawn: "Audit README.ko.md using vocabulary list."
        └─► Response: "Guide completed. Output at GUIDE.ko.md"
```

For **ISOLATED** spokes, replace path handoffs with Hub-bridged Result excerpts in `messageToSession` — do not tell B to read A's absolute Metadata workspace path.

---

## 🚫 Routing Warnings

- **Circular Dependencies (Deadlock):** Avoid scenarios where Spoke A waits for Spoke B's progress, while B is waiting for A. The Hub must compute a strict DAG (Directed Acyclic Graph) of task order before starting execution.
