------

name: extract-log-debugname: extract-log-debug

description: Extract and analyze LibrAgent debug logs with pattern matching and context. Use when debugging LibrAgent issues, analyzing agent workflows, investigating tool execution problems, or extracting specific log patterns like errors, warnings, planning operations, MCP tool calls, or browser automation traces. Supports extracting last N lines or searching for patterns with surrounding context.description: [TODO: Complete and informative explanation of what the skill does and when to use it. Include WHEN to use this skill - specific scenarios, file types, or tasks that trigger it.]

---

---

# Extract Log Debug# Extract Log Debug

Extract and analyze LibrAgent debug logs for troubleshooting and analysis.## Overview

## Quick Start[TODO: 1-2 sentences explaining what this skill enables]

Extract logs using the Python script:## Structuring This Skill

````bash[TODO: Choose the structure that best fits this skill's purpose. Common patterns:

# Extract last 100 lines

python scripts/extract_logs.py -n 100**1. Workflow-Based** (best for sequential processes)

- Works well when there are clear step-by-step procedures

# Extract all errors with 5 lines of context- Example: DOCX skill with "Workflow Decision Tree" �� "Reading" �� "Creating" �� "Editing"

python scripts/extract_logs.py --pattern "[ERROR]" --context 5- Structure: ## Overview �� ## Workflow Decision Tree �� ## Step 1 �� ## Step 2...



# Extract planning logs**2. Task-Based** (best for tool collections)

python scripts/extract_logs.py --pattern "PLANNING" -n 5000- Works well when the skill offers different operations/capabilities

- Example: PDF skill with "Quick Start" �� "Merge PDFs" �� "Split PDFs" �� "Extract Text"

# Save to custom file- Structure: ## Overview �� ## Quick Start �� ## Task Category 1 �� ## Task Category 2...

python scripts/extract_logs.py --pattern "[WARN]" -o warnings.txt

```**3. Reference/Guidelines** (best for standards or specifications)

- Works well for brand guidelines, coding standards, or requirements

## Log File Location- Example: Brand styling with "Brand Guidelines" �� "Colors" �� "Typography" �� "Features"

- Structure: ## Overview �� ## Guidelines �� ## Specifications �� ## Usage...

LibrAgent stores logs in platform-specific directories:

**4. Capabilities-Based** (best for integrated systems)

- **Windows**: `%LOCALAPPDATA%\com.fritzprix.libragent\logs\libragent.log`- Works well when the skill provides multiple interrelated features

- **macOS**: `~/Library/Logs/com.fritzprix.libragent/libragent.log`- Example: Product Management with "Core Capabilities" �� numbered capability list

- **Linux**: `~/.local/share/com.fritzprix.libragent/logs/libragent.log`- Structure: ## Overview �� ## Core Capabilities �� ### 1. Feature �� ### 2. Feature...



The script automatically detects and uses the correct path.Patterns can be mixed and matched as needed. Most skills combine patterns (e.g., start with task-based, add workflow for complex operations).



## Common PatternsDelete this entire "Structuring This Skill" section when done - it's just guidance.]



### Error Extraction## [TODO: Replace with the first main section based on chosen structure]

```bash

python scripts/extract_logs.py --pattern "[ERROR]" --context 10[TODO: Add content here. See examples in existing skills:

```- Code samples for technical skills

- Decision trees for complex workflows

### Component-Specific Logs- Concrete examples with realistic user requests

```bash- References to scripts/templates/references as needed]

# Agent workflow logs

python scripts/extract_logs.py --pattern "agent_" -n 5000## Resources



# MCP tool executionThis skill includes example resource directories that demonstrate how to organize different types of bundled resources:

python scripts/extract_logs.py --pattern "MCPServiceProxy" --context 10

### scripts/

# Planning operationsExecutable code (Python/Bash/etc.) that can be run directly to perform specific operations.

python scripts/extract_logs.py --pattern "PLANNING" --context 5

**Examples from other skills:**

# Browser automation- PDF skill: `fill_fillable_fields.py`, `extract_form_field_info.py` - utilities for PDF manipulation

python scripts/extract_logs.py --pattern "BrowserServer" --context 10- DOCX skill: `document.py`, `utilities.py` - Python modules for document processing

````

**Appropriate for:** Python scripts, shell scripts, or any executable code that performs automation, data processing, or specific operations.

### Recent Activity

````bash**Note:** Scripts may be executed without loading into context, but can still be read by Claude for patching or environment adjustments.

# Last 500 lines for quick check

python scripts/extract_logs.py -n 500### references/

Documentation and reference material intended to be loaded into context to inform Claude's process and thinking.

# Last 5000 lines for detailed analysis

python scripts/extract_logs.py -n 5000**Examples from other skills:**

```- Product management: `communication.md`, `context_building.md` - detailed workflow guides

- BigQuery: API reference documentation and query examples

## Pattern Reference- Finance: Schema documentation, company policies



For comprehensive list of log patterns and search strategies, see [log_patterns.md](references/log_patterns.md).**Appropriate for:** In-depth documentation, API references, database schemas, comprehensive guides, or any detailed information that Claude should reference while working.



Key pattern categories:### assets/

- Error patterns (`[ERROR]`, `Failed to`, `panic`)Files not intended to be loaded into context, but rather used within the output Claude produces.

- Component logs (`agent_`, `MCPServiceProxy`, `BrowserServer`)

- Workflow phases (`Think phase`, `Act phase`, `Observe phase`)**Examples from other skills:**

- Tool operations (`call_tool`, `list_tools`)- Brand styling: PowerPoint template files (.pptx), logo files

- Performance indicators (`Duration:`, `elapsed`)- Frontend builder: HTML/React boilerplate project directories

- Typography: Font files (.ttf, .woff2)

## Debugging Workflows

**Appropriate for:** Templates, boilerplate code, document templates, images, icons, fonts, or any files meant to be copied or used in the final output.

### 1. Investigate Error

---

```bash

# Extract errors**Any unneeded directories can be deleted.** Not every skill requires all three types of resources.

python scripts/extract_logs.py --pattern "[ERROR]" --context 10 -o errors.txt

# Review errors.txt for:
# - Error message and stack trace
# - Preceding operations (context lines above)
# - Affected component (agent, MCP, builtin server)
# - Session ID for tracking
````

### 2. Analyze Agent Workflow

```bash
# Extract workflow logs
python scripts/extract_logs.py --pattern "agent_" -n 5000 -o workflow.txt

# Review workflow.txt for:
# - Session lifecycle (create, start, stop)
# - Workflow phases (Think, Act, Observe)
# - Tool execution sequences
# - Loop iterations and completion
```

### 3. Debug Tool Execution

```bash
# Extract tool logs
python scripts/extract_logs.py --pattern "call_tool" --context 10 -o tools.txt

# Review tools.txt for:
# - Tool name and arguments
# - Routing (builtin vs external)
# - Execution results or errors
# - Response structure
```

### 4. Track Session Activity

```bash
# Extract specific session logs (replace <session-id> with actual ID)
python scripts/extract_logs.py --pattern "<session-id>" --context 5 -o session.txt

# Review session.txt for:
# - Session initialization
# - Tool calls made during session
# - Workflow status changes
# - Errors specific to session
```

## Output Format

### Pattern Match Output

When using `--pattern`, output includes:

- Match count and line ranges
- Line numbers with markers (`>` for matching lines)
- Context lines before and after matches
- Merged ranges for adjacent matches

Example:

```
=== Match 1: Lines 1234-1244 ===
  1234   [DEBUG] Starting operation
  1235 > [ERROR] Failed to execute tool
  1236   Stack trace: ...
  1237   at module.rs:123
```

### Full Line Output

Without `--pattern`, outputs raw log lines preserving original format.

## Tips

- Use wider context (10-20 lines) for workflow analysis
- Use narrower context (3-5 lines) for quick error checks
- Combine `-n` with `--pattern` to search recent logs only
- Check [log_patterns.md](references/log_patterns.md) for component-specific patterns
- For performance issues, search for "Duration:" or "elapsed" patterns
