---
name: critique-builtin-tool
description: Audit and critique builtin MCP tool implementations in LibrAgent. Use when auditing existing builtin MCP server implementations for compliance, reviewing pull requests that add or modify builtin tools, validating tool implementations against the Tool Design Manifesto v2.1, or identifying potential issues before they reach production.
---

# Critique Builtin MCP Tool Skill

## When to Use This Skill

Use this skill when:

- Auditing existing builtin MCP server implementations for compliance
- Reviewing pull requests that add or modify builtin tools
- Validating tool implementations against the Tool Design Manifesto v2.1
- Providing constructive feedback on tool design and implementation
- Identifying potential issues before they reach production

**Prerequisites:** Familiarity with Tool Design Manifesto v2.1, Rust, and MCP protocol basics.

---

## Audit Methodology

### Step 1: Understand the Tool Design Manifesto Rules

Before auditing, internalize these 6 critical rules:

#### **Rule 1: The Immutable ID Rule (Schema Design)**

- **Never** expose system-critical IDs as input for CREATE operations
- CREATE tools: System generates ID, agent receives it
- UPDATE/DELETE tools: ID is required input (validated before use)

#### **Rule 2: The Hallucination Firewall (Execution Logic)**

- **Never** trust agent-provided IDs without validation
- Check existence BEFORE any database/state mutation
- Return logic errors (not DB errors) with recovery hints

#### **Rule 3: The Dual-Channel Response Rule (Output)**

- **Text content** (what AI sees): Complete narrative with IDs, status, next steps
- **Structured content** (what UI sees): JSON for rendering tables/graphs
- Critical IDs MUST be in BOTH channels

#### **Rule 4: AI-Native Descriptions (Input)**

- Use data operation terms: "extract", "use", "target"
- Avoid human UI actions: "click", "type", "copy", "paste"
- Document prerequisites explicitly
- Show workflow patterns, not button clicks

#### **Rule 5: The "Success Hint" Pattern (Error Handling)**

- Every error includes path to success
- Suggest recovery tools from same tool group
- Format: "❌ Problem. 💡 Use toolName() to fix"
- Never raw "Not Found" without context

#### **Rule 6: Cache-Safe Stable Contexts (Prompt Prefix Stability)**

- `ContextVolatility::Stable` content becomes part of the cacheable prompt prefix
- Equivalent state must render **byte-for-byte identical** `context_prompt` text
- Canonicalize unordered inputs before formatting text: `HashMap`, `HashSet`, directory scans, filesystem/API listings, DB queries without `ORDER BY`
- Live or frequently changing state must **not** be marked `Stable`
- If you limit a list (`take(5)`), sort first and truncate second

---

## Step 2: Gather Code Artifacts

Collect these files for analysis:

```bash
src-tauri/src/mcp/builtin/your_server/
├── mod.rs              # Server struct + trait impl
├── tools/*.rs          # Tool schema definitions
├── handlers/*.rs       # Tool execution handlers
└── types.rs            # Domain types (if any)
```

**Critical Files to Examine:**

1. **Tool Schemas** - Check for ID parameters in CREATE operations
2. **Tool Handlers** - Check for validation before mutations
3. **Response Building** - Check dual-channel compliance
4. **Error Messages** - Check for recovery hints
5. **Service Context Builders** - Check `get_service_context()` text and volatility
6. **Backing Repositories / Iterators** - Check ordering guarantees feeding service context text

---

## Step 3: Rule-by-Rule Audit Process

### ✅ **Auditing Rule 1: Immutable ID Rule**

**What to Look For:**

```rust
// ❌ VIOLATION: ID parameter in CREATE tool
pub fn create_resource_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "id".to_string(),  // ❌ Agent can hallucinate this!
        string_prop(Some(1), Some(50), Some("Resource ID (optional)")),
    );
    // ...
}
```

```rust
// ✅ COMPLIANT: No ID parameter
pub fn create_resource_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "name".to_string(),  // ✅ Only business data
        string_prop(Some(1), Some(100), Some("Resource name")),
    );
    // No "id" field at all
}
```

**Audit Checklist:**

- [ ] All CREATE tools have NO `id` parameter in schema
- [ ] UPDATE/DELETE tools have REQUIRED `id` parameter
- [ ] IDs are generated server-side (UUID/CUID/domain-specific)
- [ ] Generated IDs are returned in responses

**Common False Positives:**

- File paths are NOT system IDs (user-controlled, not DB PKs) ✅ OK
- Session IDs passed as options (not creation parameters) ✅ OK

---

### ⚠️ **Auditing Rule 2: Hallucination Firewall**

**What to Look For:**

```rust
// ❌ VIOLATION: Direct database access without validation
pub async fn handle_update_resource(args: Value) -> Result<MCPResult, String> {
    let args: UpdateArgs = serde_json::from_value(args)?;

    // ❌ No existence check - agent can hallucinate ID
    db.resources.update(&args.id, data).await?;

    Ok(success_result())
}
```

```rust
// ✅ COMPLIANT: Validation before mutation
pub async fn handle_update_resource(args: Value) -> Result<MCPResult, String> {
    let args: UpdateArgs = serde_json::from_value(args)?;

    // ✅ Hallucination firewall
    if !db.resources.exists(&args.id).await? {
        return Ok(operation_failed_error(
            "Update Resource",
            &format!("Resource '{}' not found", args.id),
            vec![
                "Use listResources() to find the correct ID".to_string(),
                "IDs are case-sensitive".to_string(),
            ],
            ToolGroup::YourServer
        ));
    }

    // Safe to proceed
    db.resources.update(&args.id, data).await?;
    Ok(success_result())
}
```

**Audit Checklist:**

- [ ] All ID-based operations validate existence FIRST
- [ ] Validation happens BEFORE database writes
- [ ] Invalid IDs return logic errors (not DB constraint errors)
- [ ] Error messages suggest how to find valid IDs

**Common Pitfalls:**

- Registry lookups that assume ID exists ❌
- Direct `.get(id).unwrap()` without checking ❌
- Generic `not_found_error` without context ⚠️ (functional but not ideal)

---

### 🔍 **Auditing Rule 3: Dual-Channel Response (CRITICAL)**

**This is the most commonly violated rule. Check carefully!**

**What to Look For:**

```rust
// ❌ VIOLATION: ID only in structured_content (AI can't see it!)
let result_text = "Process started successfully";
let data = json!({
    "process_id": process_id  // ❌ INVISIBLE to AI
});

MCPResult {
    content: vec![text(result_text)],  // ❌ No ID in text!
    structured_content: Some(data),
}
```

```rust
// ✅ COMPLIANT: ID in BOTH channels
let result_text = format!(
    "Process started successfully (ID: {}).\n\n\
     Use pollProcess(\"{}\") to check status",
    process_id,  // ✅ Visible to AI
    process_id
);
let data = json!({
    "process_id": process_id  // ✅ Also in JSON for UI
});

MCPResult {
    content: vec![text(result_text)],
    structured_content: Some(data),
}
```

**Audit Process:**

1. **Find all response building code** - Search for `MCPResult`, `SuccessHint`, `to_mcp_result`
2. **Extract the text content** - What does the AI actually see?
3. **Check for critical IDs** - Are process IDs, resource IDs, session IDs visible?
4. **Verify in structured_content** - Is the same data in JSON for UI?

**Audit Checklist:**

- [ ] All generated IDs appear in text content (AI-visible)
- [ ] Text content is self-sufficient (no dependency on JSON)
- [ ] Critical values repeated in structured_content for UI
- [ ] No orphaned IDs (only in JSON, not in text)

**Testing Trick:**

```
Read only the text field. Can an agent:
1. Know what happened?
2. Extract the ID for next operation?
3. Understand the current state?

If NO to any → Violation of Rule 3
```

---

### 📖 **Auditing Rule 4: AI-Native Descriptions**

**What to Look For:**

```rust
// ❌ VIOLATION: Human UI-centric language
description: "Click the resource to select it, then copy the ID and paste it into the update tool"
```

```rust
// ✅ COMPLIANT: AI-native workflow description
description: "Extract resource ID from listResources() output.
Use the ID as input to updateResource() for modifications.

WORKFLOW:
1. Call listResources() to view available resources
2. Identify target resource and extract its ID
3. Pass ID to updateResource(id, newData)

PREREQUISITE: Resource must exist (created via createResource)"
```

**Audit Checklist:**

- [ ] No human UI verbs: click, type, copy, paste, drag, select
- [ ] Uses data operation verbs: extract, use, pass, call, retrieve
- [ ] Prerequisites explicitly documented
- [ ] Workflow shows tool call sequences
- [ ] Examples demonstrate actual usage patterns

**Red Flags:**

- References to "UI", "button", "dialog", "form", "screen"
- Phrases like "enter your input", "click to confirm"
- Missing prerequisite tools in workflow

---

### ✂️ **Auditing Rule 4b: Description / Parameter Separation**

**Principle:** Tool description and parameter description have distinct responsibilities. Repeating information in both wastes the agent's token budget and increases hallucination risk (conflicting or stale duplicates).

| Layer                     | Responsibility                                                  |
| ------------------------- | --------------------------------------------------------------- |
| **Tool description**      | _What_ the tool does — purpose, behavior, constraints, output   |
| **Parameter description** | _What to put in this field_ — format, accepted values, examples |

**What to Look For:**

```rust
// ❌ VIOLATION: Source examples duplicated in both layers
MCPTool {
    description: r#"Fetch an image from a URL or workspace-relative path.

**Supported sources:**
- Web URLs: `https://example.com/photo.jpg`        // ← repeated below
- Workspace-relative paths: `screenshots/img.png`  // ← repeated below
"#,
    // param repeats exactly the same examples
    input_schema: string_prop_required(
        "URL or workspace-relative path (e.g. https://example.com/photo.jpg or screenshots/img.png).",
    ),
}
```

```rust
// ✅ COMPLIANT: Each layer owns its content
MCPTool {
    description: r#"Fetch an image and include it in the conversation so you can visually analyse it.

**Supported formats:** JPEG, PNG, GIF, WebP, BMP, SVG   // constraint belongs here

**Notes:**
- Maximum file size: 20 MB.                             // constraint belongs here
- Local paths must be inside the session workspace."#,

    // param owns: format spec + examples
    input_schema: string_prop_required(
        "URL or workspace-relative path of the image to fetch \
         (e.g. https://example.com/photo.jpg or screenshots/capture.png).",
    ),
}
```

**Audit Checklist:**

- [ ] Tool description does NOT list param examples (URL examples, path examples)
- [ ] Tool description does NOT describe input format (that belongs in the param)
- [ ] Tool description does NOT re-state param description semantics (mode meanings, enum semantics)
- [ ] Parameter description does NOT re-state the tool's purpose or constraints
- [ ] Parameter description does NOT repeat schema-encoded constraints (max, min, default, enum values)
- [ ] Supported format lists belong in tool description (apply to the whole tool, not one param)
- [ ] File size / access restrictions belong in tool description (behavioral constraints)
- [ ] Concrete input examples (e.g. `https://...`) belong in parameter description only
- [ ] **Numeric/length constraints encoded in JSON Schema, NOT in description text**
- [ ] **Default values NOT repeated in param descriptions when already in schema `default` field**
- [ ] **Enum semantics (what each value does) are acceptable; bare value lists are not**

**Rule 4b Extension — Schema Contract First**

Constraints that can be expressed in JSON Schema MUST be encoded there, not in description text. Description text is a fallback for constraints the schema cannot express.

This rule applies to **both** tool descriptions AND parameter descriptions — neither layer should repeat what the schema already encodes.

| Constraint type                       | Encode in schema                                        | Also in description/param desc?                      |
| ------------------------------------- | ------------------------------------------------------- | ---------------------------------------------------- |
| String length limit                   | `string_prop(None, Some(500), ...)`                     | ❌ No — schema is authoritative                      |
| Integer range                         | `integer_prop(Some(1), Some(100), ...)`                 | ❌ No                                                |
| Default value                         | second arg in `integer_prop_with_default(..., 30, ...)` | ❌ No — `(default 30)` is redundant                  |
| Enum allowed values (bare list)       | `enum_prop(vec!["a","b","c"], ...)`                     | ❌ No — listing values in text is triple duplication |
| Enum semantics (what each value does) | Cannot be expressed in schema                           | ✅ Yes — explains behaviour, not just names          |
| Required vs optional                  | `required: vec!["field"]` in `object_prop`              | ❌ No                                                |
| Pattern / format                      | `JSONSchemaType::String { pattern: Some(...) }`         | Only if pattern is opaque                            |
| Cross-field rules                     | Cannot be expressed in JSON Schema                      | ✅ Yes — must go in description                      |

```rust
// ❌ VIOLATION: Constraint in description, not schema
string_prop_required("The task description. Maximum 500 characters.")
//                                          ^^^^^^^^^^^^^^^^^^^^^^^^^^
//                                          This belongs in maxLength, not here

// ✅ CORRECT: Constraint encoded in schema
string_prop(None, Some(500), Some("The task description."))
//               ^^^^^^^^
//               maxLength: 500 — LLMs reading the schema see this automatically

// ❌ VIOLATION: Enum values re-listed in description (bare list adds nothing)
enum_prop(vec!["done", "pending", "cancel"], "done",
    Some("Action: 'done', 'pending', or 'cancel'."))

// ❌ VIOLATION: Default value re-stated in param description
integer_prop_with_default(Some(0), Some(3600), 30,
    Some("Timeout in seconds (default 30)."))
//                                 ^^^^^^^^^^ already in schema default

// ✅ CORRECT: Enum semantics (not bare list) — explains what each value does
enum_prop(vec!["tail", "head"], "tail",
    Some("'tail' reads last N lines, 'head' reads first N lines"))
//       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//       Values appear in schema; this adds the semantic meaning → acceptable

// ✅ CORRECT: No redundant default or values
integer_prop_with_default(Some(0), Some(3600), 30,
    Some("Timeout in seconds. Use 0 to return current status immediately."))
//        ^^^^^^^^ constraint-free prose; the behaviour of 0 is not in schema
```

**Tool description ↔ Param description cross-direction rule:**

The duplication problem runs both ways:

- Tool description must NOT copy content from param descriptions
- Param descriptions must NOT copy content from tool description

```
// ❌ VIOLATION: Tool description re-states all param semantics
writeFile description:
  - mode='create': fails if file already exists
  - mode='overwrite': replaces entire content, returns a diff   ← copied from mode param
  - mode='append': adds content to the end                      ← copied from mode param
mode param: "Write mode. 'create' fails if exists, 'overwrite' replaces, 'append' adds."

// ✅ CORRECT: Tool description adds ONLY what param descriptions don't cover
writeFile description: "Create, overwrite, or append content to a file. mode='overwrite' returns a diff."
//                                                                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//                                                                        Unique fact not in the mode param desc
mode param: "Write mode. If omitted, defaults to 'create'. 'create' fails if the file already exists,
             'overwrite' replaces the entire file, and 'append' adds content to the end."
```

**Test:** "Could this constraint be removed from the description without losing information, because the schema already encodes it?" → If yes, remove it from the description.

**Common Duplication Patterns:**

```
❌ description: "Fetch image from a URL or path. Supported sources: URLs, paths."
   param:       "URL or path of the image to fetch."
   → "URL or path" stated twice

❌ description: "...  examples: https://x.com/a.jpg or screenshots/img.png"
   param:       "(e.g. https://x.com/a.jpg or screenshots/img.png)"
   → Same examples copy-pasted

❌ description: lists all mode='X': behaviour semantics for every enum value
   param:       already has the full semantics for the mode parameter
   → tool description is copying param description content

❌ param:       "Number of lines to read (max 100)"
   schema:      integer_prop_with_default(Some(1), Some(100), 20, ...)
   → (max 100) is redundant; schema encodes it

✅ description: "Fetch an image and include it in the conversation."
   param:       "URL or workspace-relative path (e.g. https://x.com/a.jpg or screenshots/img.png)."
   → Clean split: purpose in description, format+examples in param
```

---

### 💡 **Auditing Rule 5: Success Hint Pattern**

**What to Look For:**

```rust
// ❌ VIOLATION: Raw error, no recovery path
.ok_or_else(|| format!("Process '{}' not found", process_id))?
```

```rust
// ⚠️ PARTIAL: Generic hints (not context-specific)
return Ok(not_found_error("Process", process_id, ToolGroup::Workspace));
// Uses default hints like "Use listDirectory" ❌ Wrong for processes
```

```rust
// ✅ COMPLIANT: Context-specific recovery hints
return Ok(operation_failed_error(
    "Poll Process",
    &format!("Process '{}' not found", process_id),
    vec![
        "Use listProcesses() to see all active processes".to_string(),
        "Process IDs are case-sensitive and must match exactly".to_string(),
        "Process may have finished - check with readProcessOutput()".to_string(),
    ],
    ToolGroup::Workspace
));
```

**Audit Checklist:**

- [ ] All errors include 2-3 actionable recovery steps
- [ ] Suggested tools are from same tool group
- [ ] Error format includes ✗ marker and 💡 hints
- [ ] No raw Err() returns without context
- [ ] Generic helpers used appropriately (or replaced with specific hints)

**Common Issues:**

- Using `not_found_error` helper for different resource types
- Missing tool suggestions for recovery
- Hints reference wrong tool group (browser hints for workspace errors)

---

### 🧭 **Auditing Rule 6: Cache-Safe Stable Contexts**

**Why this matters:**

`ContextVolatility::Stable` is not just a UI hint. Stable service-context text is appended into the reusable prompt prefix, so nondeterministic rendering directly causes prompt cache misses even when message history is unchanged.

**What to Look For:**

```rust
// ❌ VIOLATION: Stable context built from HashMap iteration
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let installed: Vec<String> = platform
        .installed_tools     // HashMap<String, ToolInfo>
        .iter()              // ❌ Unordered
        .filter(|(_, info)| info.installed)
        .map(|(name, _)| name.clone())
        .collect();

    ServiceContext::new(format!(
        "## Bootstrap\n\nInstalled Tools: {}",
        installed.join(", ")
    ))
    .with_volatility(ContextVolatility::Stable)
}
```

```rust
// ✅ COMPLIANT: Canonicalize before rendering Stable text
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let mut installed: Vec<String> = platform
        .installed_tools
        .iter()
        .filter(|(_, info)| info.installed)
        .map(|(name, _)| name.clone())
        .collect();
    installed.sort_unstable();

    ServiceContext::new(format!(
        "## Bootstrap\n\nInstalled Tools: {}",
        installed.join(", ")
    ))
    .with_volatility(ContextVolatility::Stable)
}
```

```rust
// ❌ VIOLATION: Truncating before sorting makes the visible subset unstable
let processes = registry
    .entries
    .values()
    .filter(|entry| entry.running)
    .take(5)   // ❌ Picks arbitrary five first
    .map(|entry| (entry.id.clone(), entry.command.clone()))
    .collect::<Vec<_>>();
```

```rust
// ✅ COMPLIANT: Sort first, then truncate
let mut processes = registry
    .entries
    .values()
    .filter(|entry| entry.running)
    .map(|entry| (entry.id.clone(), entry.command.clone()))
    .collect::<Vec<_>>();
processes.sort_by(|left, right| left.0.cmp(&right.0));
let visible = processes.into_iter().take(5).collect::<Vec<_>>();
```

```rust
// ❌ VIOLATION: Repository query feeding service context without ORDER BY
ScheduledTaskEntity::find()
    .filter(scheduled_task::Column::Enabled.eq(true))
    .all(&self.db)   // ❌ Row order is not guaranteed
    .await
```

```rust
// ✅ COMPLIANT: Deterministic DB ordering with tie-breaker
ScheduledTaskEntity::find()
    .filter(scheduled_task::Column::Enabled.eq(true))
    .order_by(scheduled_task::Column::NextRunAt, Order::Asc)
    .order_by(scheduled_task::Column::CreatedAt, Order::Asc)
    .order_by(scheduled_task::Column::Id, Order::Asc)
    .all(&self.db)
    .await
```

**Audit Checklist:**

- [ ] Every `ContextVolatility::Stable` context uses deterministic text rendering
- [ ] Unordered collections are sorted before joining into prompt text
- [ ] Filesystem scans / directory walks are sorted before rendering
- [ ] DB queries feeding stable context use explicit `ORDER BY` with tie-breakers when needed
- [ ] `take()/limit` happens **after** sort, not before
- [ ] Frequently changing state is marked `Medium` or `Volatile`, not `Stable`
- [ ] If a service context claims "Stable", equivalent state really produces identical text

**Lower-Priority Extension:**

Even `Medium` / `Volatile` contexts benefit from deterministic ordering for prompt quality and compaction stability. These are not prompt-cache blockers, but they are still worth flagging as cleanup if ordering is obviously arbitrary.

**Common Pitfalls:**

- `HashMap::iter()` / `.values()` inside `Stable` service context text ❌
- `WalkDir` / `read_dir()` output rendered without sorting ❌
- Relying on implicit DB row order ❌
- Sorting after `.take(5)` ❌
- Marking live process lists, browser session state, or recent uploads as `Stable` ❌

**Priority Guidance:**

- **P1 (High):** Nondeterministic text inside `ContextVolatility::Stable`
- **P2 (Medium):** Ordering instability in `Medium` / `Volatile` contexts
- **P2 (Medium):** Repository helpers lacking explicit ordering that could later feed prompt text

---

## Step 4: Document Findings

### Compliance Matrix Template

```markdown
## Compliance Audit: [ServerName] Builtin Tools

| Rule                          | Status   | Grade | Evidence  |
| ----------------------------- | -------- | ----- | --------- |
| 1. Immutable ID Rule          | ✅/⚠️/🔴 | A-F   | [Details] |
| 2. Hallucination Firewall     | ✅/⚠️/🔴 | A-F   | [Details] |
| 3. Dual-Channel Response      | ✅/⚠️/🔴 | A-F   | [Details] |
| 4. AI-Native Descriptions     | ✅/⚠️/🔴 | A-F   | [Details] |
| 4b. Description/Param Split   | ✅/⚠️/🔴 | A-F   | [Details] |
| 5. Success Hint Pattern       | ✅/⚠️/🔴 | A-F   | [Details] |
| 6. Cache-Safe Stable Contexts | ✅/⚠️/🔴 | A-F   | [Details] |

**Overall Grade:** [A-F] - [Summary]
```

### Grading Rubric

**A (Excellent):** Fully compliant, exemplary implementation
**B (Good):** Compliant with minor improvements possible
**C (Acceptable):** Functional but has non-critical issues
**D (Needs Improvement):** Has violations that impact UX
**F (Critical Issues):** Blocking issues, not production-ready

---

## Step 5: Provide Constructive Feedback

### Feedback Template

````markdown
### [Priority Level] [Rule Name] - [Title]

**Problem:** [Clear description of what's wrong]

**Location:** `file.rs` lines X-Y

**Current Code:**

```rust
// Show the problematic code
```
````

**Issue:** [Why this violates the manifesto]

**Recommended Fix:**

```rust
// Show the corrected code
```

**Impact:** [How this affects AI agents]

**Priority:** P0 (Blocker) / P1 (High) / P2 (Medium) / P3 (Low)

````

### Priority Guidelines

**P0 (Blocker):** Must fix before production
- Rule 3 violations (IDs invisible to AI)
- Rule 1 violations (ID input on create)
- Missing validation causing crashes

**P1 (High):** Should fix soon
- Rule 2 partial violations (poor error messages)
- Rule 5 violations (no recovery hints)
- Rule 6 violations in `ContextVolatility::Stable`

**P2 (Medium):** Nice to have
- Rule 4 improvements (better descriptions)
- Inconsistent error formatting
- Ordering instability in non-stable service contexts

**P3 (Low):** Optional polish
- Documentation improvements
- Code organization

---

## Real-World Example: Workspace Tools Audit

### Initial Assessment (Incorrect)

```markdown
## 📊 Compliance Score

| Rule | Grade | Issue |
|------|-------|-------|
| 3. Dual-Channel Response | D 🔴 | Process IDs invisible to AI |

**Finding:** Process IDs only in structured_content, agents can't see them.

**Evidence:** Assumed text content didn't include IDs based on quick scan.
````

### Corrected Assessment (After Deep Analysis)

````markdown
## 📊 Compliance Score (CORRECTED)

| Rule                     | Grade | Evidence                        |
| ------------------------ | ----- | ------------------------------- |
| 3. Dual-Channel Response | A ✅  | Process IDs ARE in text content |

**Finding:** IDs are properly visible in BOTH channels.

**Evidence from code (lines 1075-1090):**

```rust
let hint = SuccessHint::new(
    format!(
        "Background process started successfully

• Process ID: {}  // ✅ VISIBLE
• Command: {}

💡 Next Steps:
Use pollProcess(\"{}\") to check status",
        process_id, command, process_id
    ),
    // ...
);
```
````

**Lesson:** Always verify by reading actual response text, not just scanning for patterns.

```

---

## Common Audit Mistakes to Avoid

### ❌ Mistake 1: Scanning Instead of Reading

**Wrong Approach:**
```

Search for "process_id" in structured_content → Found!
Assume it's not in text → Mark as violation ❌

```

**Right Approach:**
```

1. Find response building code
2. Extract literal text content
3. Read what AI actually sees
4. Then check structured_content
5. Verify both channels have critical data ✅

```

### ❌ Mistake 2: Assuming Generic Helpers Are Wrong

**Wrong Assumption:**
```

Code uses not_found_error() → Must be bad ❌

```

**Right Analysis:**
```

1. Check what not_found_error() returns
2. Read the default hints for this tool group
3. Verify hints match the resource type
4. If hints are generic → Suggest improvement ⚠️
5. If hints are wrong → Mark as issue 🔴

```

### ❌ Mistake 3: Missing Context

**Wrong Critique:**
```

"File operations lack validation" ❌
(Actually, they use SecureFileManager with validation)

```

**Right Critique:**
```

1. Trace validation through call stack
2. Check if validation helper exists
3. Verify validation catches edge cases
4. Only flag if genuinely missing ✅

````

---

## Validation Checklist

Before submitting audit findings:

- [ ] Verified code by reading actual implementations (not assumptions)
- [ ] Checked if violations are real or helper-abstracted
- [ ] Provided specific line numbers for issues
- [ ] Included code examples for both violation and fix
- [ ] Graded each rule independently
- [ ] Assigned appropriate priorities
- [ ] Tested recommended fixes compile (if providing code)
- [ ] Acknowledged what's already good (not just problems)
- [ ] Checked `get_service_context()` volatility + ordering, not just tool handlers

---

## Audit Report Template

```markdown
# [ServerName] Builtin Tools Audit Report

**Auditor:** [Name/System]
**Date:** [YYYY-MM-DD]
**Version:** [Code version/commit]

## Executive Summary

[Overall grade and key findings in 2-3 sentences]

## Detailed Analysis

### Rule 1: Immutable ID Rule - [Grade]

**Status:** ✅ Compliant / ⚠️ Partial / 🔴 Violation

**Findings:**
- [Finding 1]
- [Finding 2]

**Evidence:**
```rust
// Code examples
````

### [Repeat for Rules 2-5]

## Priority Fixes

### P0 (Blocker)

1. [Issue title] - [File location]
   - Impact: [Description]
   - Fix: [Code example]

### P1 (High)

[Similar format]

### P2 (Medium)

[Similar format]

## What's Already Good

- ✅ [Strength 1]
- ✅ [Strength 2]
- ✅ [Strength 3]

## Recommendations

1. [Recommendation 1]
2. [Recommendation 2]

## Conclusion

[Final assessment and next steps]

````

---

## Testing Your Audit

### Self-Validation Questions

1. **Did I read actual code or make assumptions?**
   - ✅ Traced through implementation
   - ❌ Assumed based on patterns

2. **Are my examples accurate?**
   - ✅ Copy-pasted from source
   - ❌ Paraphrased or invented

3. **Did I verify fixes would work?**
   - ✅ Tested or checked against working examples
   - ❌ Suggested theoretical fixes

4. **Am I being fair?**
   - ✅ Acknowledged what's good
   - ❌ Only listed problems

5. **Is my feedback actionable?**
   - ✅ Specific files, lines, and code examples
   - ❌ Vague "improve error handling"

---

## Advanced: Automated Audit Patterns

### Grep Patterns for Quick Scan

```bash
# Rule 1: Find CREATE tools with ID parameters
rg -A 10 'fn create_.*_tool' | rg 'props.insert.*"id"'

# Rule 2: Find direct database access without validation
rg 'db\.\w+\.(update|delete|insert)' | rg -v 'if.*exists'

# Rule 3: Find response building
rg 'MCPResult|SuccessHint::new|to_mcp_result'

# Rule 5: Find raw Err returns
rg 'Err\(format!\(".*not found'

# Rule 6: Find Stable service contexts
rg -n 'ContextVolatility::Stable|with_volatility\(ContextVolatility::Stable\)' src-tauri/src/mcp

# Rule 6: Find likely unordered inputs near service-context builders
rg -n 'get_service_context|HashMap|HashSet|WalkDir|read_dir|\.values\(\)|\.keys\(\)|join\(' src-tauri/src/mcp src-tauri/src/services src-tauri/src/repositories

# Rule 6: Find DB queries without explicit ordering in prompt-adjacent code
rg -n 'find\(\)|all\(&self\.db\)|all\(&db\)' src-tauri/src/repositories src-tauri/src/mcp | rg -v 'order_by'
````

### Code Review Checklist

When reviewing PR:

- [ ] New CREATE tools have no ID parameter
- [ ] UPDATE/DELETE operations validate before mutation
- [ ] Response text includes all critical IDs
- [ ] Error messages suggest recovery tools
- [ ] Tool descriptions are AI-native
- [ ] No human UI verbs in descriptions
- [ ] Success/error hints are context-specific
- [ ] Structured content mirrors text content
- [ ] Stable service-context text is canonicalized before rendering
- [ ] Live state is not mislabeled as `ContextVolatility::Stable`

---

## Summary

**Key Principles for Effective Audits:**

1. **Read Code, Don't Assume** - Verify by tracing actual execution
2. **Context Matters** - Generic helpers may be appropriate
3. **Be Specific** - Provide exact locations and examples
4. **Be Balanced** - Acknowledge strengths and weaknesses
5. **Be Actionable** - Give clear fixes, not vague suggestions
6. **Validate Yourself** - Double-check before publishing findings

**Common Audit Flow:**

```
1. Gather code artifacts
2. Apply rule-by-rule checklist
3. Document findings with evidence
4. Prioritize issues (P0-P3)
5. Provide specific fixes
6. Validate audit accuracy
7. Submit constructive feedback
```

**When in Doubt:**

- Read the actual code path
- Check if helpers abstract the pattern correctly
- Look for working examples in the codebase
- Verify your recommended fix would compile
- Ask for second review if unsure

The goal is **constructive improvement**, not finding fault. Good audits help teams build better AI agent tools!
