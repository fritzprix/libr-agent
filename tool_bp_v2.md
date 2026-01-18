# Built-in MCP Tool Design Manifesto

**Version 2.1**
**Core Philosophy:** The AI Agent is a high-reasoning engine with **zero system trust**. It must never possess the authority to define system identifiers, and its inputs must always be treated as potential hallucinations.

## 1. The Immutable ID Rule (Schema Design)

**Rule:** Never expose system-critical identifiers (IDs, Primary Keys) as input parameters for **Creation** tools.

- **The Vulnerability:** If you include an optional `id` field in a `create` tool schema, the Agent **will** attempt to invent one (hallucinate), often creating collisions or malformed keys.
- **The Fix:**
- **Creation Schemas:** `id` field is **strictly forbidden**. The System generates it; the Agent receives it.
- **Update/Delete Schemas:** `id` is **mandatory** but treated as "unverified text" until validated.

| Operation         | Schema `id` Field | Origin                              |
| ----------------- | ----------------- | ----------------------------------- |
| `create_resource` | ❌ **Forbidden**  | System Generated (UUID/CUID)        |
| `update_resource` | ✅ **Required**   | Extracted from previous tool output |

## 2. The Hallucination Firewall (Execution Logic)

**Rule:** Never trust an ID provided by an Agent. Always validate existence **before** any database operation.

- **The Risk:** Agents often hallucinate IDs (e.g., guessing `user_1` instead of `user_8723`). If passed directly to a DB, this causes Foreign Key constraints to break or corrupts data relationships.
- **The Protocol:**

1. **Check Existence First:** Before `UPDATE` or `INSERT` with a Foreign Key, perform a lightweight `SELECT count(*)` or `exists()` check.
2. **Fail Gracefully:** If the ID does not exist, return a **Logic Error**, not a Database Error.
3. **Suggest Recovery:** Tell the agent the ID is invalid and prompt them to `list_items` to find the real one.

```rust
// ❌ Dangerous (Direct DB Access)
// Agent sends "proj_999" (Hallucinated) -> DB crashes on FK violation
db.tasks.insert(task_data).await?;

// ✅ Secure (The Firewall)
if !db.projects.exists(&args.project_id).await? {
    // Stop immediately. Do not touch the DB write layer.
    return Ok(operation_failed_error(
        "Create Task",
        &format!("Project ID '{}' does not exist", args.project_id),
        vec!["Use 'list_projects' to find the correct valid ID".to_string()],
        ToolGroup::Planning
    ));
}
// Proceed only after validation passes
db.tasks.insert(task_data).await?;

```

## 3. The Dual-Channel Response Rule (Output)

**Rule:** Every tool execution must satisfy both the Agent's reasoning and the User's UI.

- **Channel 1: `content` (Text)**
- **Audience:** The AI Agent.
- **Requirement:** Must be a **complete narrative**. explicitly stating _what_ happened, the _IDs_ created, and the _status_.

- **Channel 2: `structured_content` (JSON)**
- **Audience:** The Frontend UI.
- **Requirement:** Raw data for rendering tables/graphs. **The Agent never sees this.**

## 4. AI-Native Descriptions (Input)

**Rule:** The tool description is your API documentation for the AI. Use precise, non-human vocabulary.

- ❌ **Avoid:** "Copy", "Paste", "Click" (Human UI actions).
- ✅ **Use:** "Extract", "Use", "Target" (Data operations).
- **Prerequisites:** Explicitly document dependencies (e.g., "MANDATORY: Call `get_user` FIRST to obtain the valid `user_id`").

## 5. The "Success Hint" Pattern (Error Handling)

**Rule:** An error must always provide a path to success using _other_ tools.

- **Never** return a raw "Not Found".
- **Always** return an error + a suggestion from the **same tool group**.
- _Example:_ "❌ ID `doc_X` not found. 💡 **Next Steps:** Use `list_documents` to verify the ID."

---

### Security & Integrity Checklist

| Feature                   | Implementation Check                                                                     |
| ------------------------- | ---------------------------------------------------------------------------------------- |
| **Schema Security**       | Is the `id` parameter completely removed from `create` schemas?                          |
| **Integrity Check**       | Does the code validate the existence of every FK/ID _before_ attempting a write?         |
| **Hallucination Defense** | Does the error message guide the agent to find the _real_ ID (e.g., via `list`)?         |
| **Response Clarity**      | Does the text response clearly display the System-Generated ID so the Agent can read it? |
