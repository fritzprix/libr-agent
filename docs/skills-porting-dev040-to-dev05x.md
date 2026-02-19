# Skills Implementation: dev/0.4.0 → dev/0.5.x Porting Guide

> **Status**: dev/0.4.0 is fully working after the fixes described here.  
> **Purpose**: Port the entire skills subsystem to dev/0.5.x.

---

## 1. Architecture Overview

### Two injection paths exist in the codebase

| Path | File | Mechanism | Status (dev/0.4.0) | Status (dev/0.5.x) |
|------|------|-----------|--------------------|--------------------|
| **SkillsServer** | `src-tauri/src/mcp/builtin/skills/mod.rs` | MCP BuiltinServer → `get_service_context()` → system prompt step 4 | ✅ ACTIVE (after fix) | ❌ DEAD (not wired) |
| **SkillsContextProvider** | `src-tauri/src/agent/context/skills.rs` | ContextRegistry provider → system prompt step 3 | ❌ DEAD (commented out) | ✅ ACTIVE (accidentally restored by revert) |

### Why SkillsServer is preferred over SkillsContextProvider

1. Receives `ServiceContextOptions { session_id, assistant_id }` → can do assistant-specific overrides
2. `disabledSkills` filter from assistant config (assistant config JSON field)
3. Standard BuiltinMCPServer pattern, consistent with other tools (Planning, Browser, etc.)
4. Already uses correct XML format (`<available_skills>` / `<location>`)

### Recommendation for dev/0.5.x

- **Activate SkillsServer** (add `"skills"` to `extract_builtin_tool_ids()`)
- **Disable SkillsContextProvider** (comment out or remove registration from ContextRegistry)
- This matches what dev/0.4.0 does after the fix

---

## 2. File-by-File Changes Needed

### 2.1 `src-tauri/src/agent/tools.rs` — **CRITICAL FIX**

`extract_builtin_tool_ids()` controls which BuiltinMCPServer instances are created per session.
`"skills"` is missing from both branches — this is why SkillsServer never activates.

**Find the function and add `"skills"` to BOTH branches:**

```rust
// Branch 1: allowed_aliases match arm (when specific tool IDs are provided)
// Look for where "planning", "knowledge", "browser", etc. are listed
// Add "skills" alongside them

// Branch 2: None case / default (all services enabled)
// Same list — add "skills" here too
```

The exact pattern to search for (grep):
```
grep -n "planning\|knowledge\|browser" src-tauri/src/agent/tools.rs
```

After the fix, both branches should include `"skills"`.

---

### 2.2 `src-tauri/src/lifecycle/app_setup.rs` — **MISSING ENTIRELY IN dev/0.5.x**

dev/0.5.x deleted `copy_bundled_skills_to_app_data()` and `copy_dir_recursive()` completely.
These must be restored.

#### 2.2.1 Add constant at top of file

```rust
/// Marker file written into every bundled skill directory in AppData.
/// Used to distinguish bundled skills from user-created ones so that skills
/// removed from the bundle can be cleaned up automatically on the next launch.
const BUNDLED_SKILL_MARKER: &str = ".bundled_skill";
```

#### 2.2.2 Add `copy_bundled_skills_to_app_data()` function

```rust
/// Copy bundled skills from app resources to AppData/skills directory.
///
/// Rules:
/// - `.force_update` present  → always overwrite existing skill
/// - `.force_update` absent   → copy only if destination doesn't exist (preserves user edits)
/// - Skill has `.bundled_skill` marker but is no longer in bundle → remove (cleanup stale skills)
async fn copy_bundled_skills_to_app_data(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    let resource_dir = app.path().resource_dir()?;
    let bundled_skills_dir = resource_dir.join("bundled_skills");

    let app_data_dir = app.path().app_data_dir()?;
    let target_skills_dir = app_data_dir.join("skills");

    if !bundled_skills_dir.exists() {
        log::debug!("No bundled_skills directory found in resources");
        return Ok(());
    }

    fs::create_dir_all(&target_skills_dir)?;

    // Build set of current bundled skill names
    let bundled_names: std::collections::HashSet<std::ffi::OsString> =
        fs::read_dir(&bundled_skills_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();

    // Remove stale bundled skills: present in AppData but no longer in bundle
    for entry in fs::read_dir(&target_skills_dir)? {
        let entry = entry?;
        let skill_name = entry.file_name();
        let target_skill_dir = entry.path();

        if target_skill_dir.is_dir()
            && !bundled_names.contains(&skill_name)
            && target_skill_dir.join(BUNDLED_SKILL_MARKER).exists()
        {
            log::info!("🗑️  Removing stale bundled skill: {:?}", skill_name);
            fs::remove_dir_all(&target_skill_dir)?;
        }
    }

    // Copy / update each bundled skill
    for entry in fs::read_dir(&bundled_skills_dir)? {
        let entry = entry?;
        let skill_name = entry.file_name();
        let source_skill_dir = entry.path();
        let target_skill_dir = target_skills_dir.join(&skill_name);

        let force_update_marker = source_skill_dir.join(".force_update");
        let should_force_update = force_update_marker.exists();

        if should_force_update {
            if target_skill_dir.exists() {
                log::info!("🔄 Force updating skill: {:?}", skill_name);
                fs::remove_dir_all(&target_skill_dir)?;
            } else {
                log::info!("📦 Installing new skill: {:?}", skill_name);
            }
            copy_dir_recursive(&source_skill_dir, &target_skill_dir)?;
        } else if !target_skill_dir.exists() {
            log::info!("📦 Copying bundled skill: {:?}", skill_name);
            copy_dir_recursive(&source_skill_dir, &target_skill_dir)?;
        } else {
            log::debug!("⏭️  Skill already exists, skipping: {:?}", skill_name);
        }

        // Write bundled marker so future runs can identify this as a bundled skill
        let marker_path = target_skill_dir.join(BUNDLED_SKILL_MARKER);
        if !marker_path.exists() {
            fs::write(&marker_path, "")?;
        }
    }

    Ok(())
}

/// Recursively copy directory contents from src to dst.
fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    use std::fs;

    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
```

#### 2.2.3 Call from `setup_app()`

Inside `setup_app()`, find where other async startup tasks are run (look for `tauri::async_runtime::block_on`).
Add the bundled skills copy call there:

```rust
tauri::async_runtime::block_on(async {
    // ... other init calls ...
    if let Err(e) = copy_bundled_skills_to_app_data(app).await {
        log::warn!("Failed to copy bundled skills: {}", e);
    }
});
```

---

### 2.3 `src-tauri/src/agent/context/skills.rs` — **Disable in dev/0.5.x**

dev/0.5.x has `SkillsContextProvider` active (via ContextRegistry).
Once SkillsServer is activated, this creates duplicate skills injection.

**Two-step disable:**

1. In `src-tauri/src/agent/context/mod.rs`, comment out:
   ```rust
   // pub mod skills;
   ```

2. In `src-tauri/src/agent/session_manager.rs` (or wherever ContextRegistry is built),
   remove the `SkillsContextProvider` registration:
   ```rust
   // registry.register(Box::new(SkillsContextProvider::new()));  // replaced by SkillsServer
   ```

**Alternative (keep both but check for duplicates):** Not recommended. SkillsServer is more capable.

---

### 2.4 `src-tauri/src/mcp/builtin/skills/mod.rs` — No changes needed

This file is already correct in both branches. The `get_service_context()` implementation:

- Reads `skillsDirectory` from `systemSettings` (falls back to `AppData/skills`)
- Builds `assistant_skills_dir` from `AppData/assistants/{assistant_id}/skills` when assistant_id provided
- Calls `resolve_skills(global_dir, assistant_skills_dir)` for override logic
- Filters `disabledSkills` from assistant config JSON
- Returns empty `context_prompt` (not an error) when no skills found
- XML format is correct: `<available_skills>` root, `<skill source="...">`, `<name>`, `<description>`, `<location>`

---

### 2.5 `src-tauri/src/commands/skill_commands.rs` — Verify `get_configured_skills_directory()`

In dev/0.4.0 this function is **hardcoded** to AppData/skills.  
In dev/0.5.x it reads from settings with fallback — which is better.

Check dev/0.5.x already has the settings-aware version. If so, no change needed.  
The relevant code in dev/0.5.x's SkillsServer (`get_skills_directory()`) also reads from settings, so they're consistent.

---

## 3. Core Logic Reference

### 3.1 `resolve_skills()` — Override-only, no merge

```
if assistant_dir is Some AND contains skills → return ONLY assistant skills (source="assistant")
else                                         → return global skills (source="global")
NEVER merges both lists
```

This means an assistant with even one skill completely replaces the global skill list.

### 3.2 Skill discovery: `scan_skills_internal()`

- Uses `WalkDir` starting at the given directory
- Finds all files named exactly `SKILL.md`
- Calls `parse_skill_metadata()` on each
- Sets `path` = absolute path to the `SKILL.md` file

### 3.3 `parse_skill_metadata()`

- Reads file content
- Looks for YAML frontmatter delimited by `---` ... `---`
- Extracts `name:` and `description:` fields
- Returns `None` if either field is missing or frontmatter is absent

### 3.4 `SkillMetadata` struct

```rust
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub path: String,
    pub source: Option<String>,  // "global" | "assistant"
}
```

---

## 4. Bundled Skills File Structure

```
bundled_skills/           (in tauri resources, ships with app)
  skill-name/
    SKILL.md              (required: YAML frontmatter + content)
    .force_update         (optional: if present, always overwrites user's copy)
    other-files...

AppData/skills/           (deployed at runtime)
  skill-name/
    SKILL.md
    .bundled_skill        (marker written by app — identifies as bundled, not user-created)
    other-files...
  user-custom-skill/      (NO .bundled_skill marker → never auto-deleted)
    SKILL.md
```

### Stale cleanup logic

On every app launch, `copy_bundled_skills_to_app_data()`:
1. Builds `bundled_names` = set of directory names in `bundled_skills/` resource
2. Scans `AppData/skills/`; for each directory:
   - If it has `.bundled_skill` marker AND its name is NOT in `bundled_names` → `rm -rf` it
3. Copies/updates from bundle using `.force_update` rules

User-created skills (no `.bundled_skill` marker) are **never touched**.

---

## 5. `tauri.conf.json` Resources Entry

Bundled skills must be declared in `tauri.conf.json` under `bundle.resources`:

```json
{
  "bundle": {
    "resources": [
      "bundled_skills/**/*"
    ]
  }
}
```

Verify this exists in dev/0.5.x. If it was removed, add it back or skills won't be packaged.

---

## 6. XML Format (agentskills.io standard)

```xml
## Available Skills

You have access to the following skills. The <location> tag specifies the main documentation file for each skill.
To use a skill, you MUST first read its <location> file using the `readFile` tool. This file contains all necessary instructions and commands.

<available_skills>
  <skill source="global">
    <name>skill-name</name>
    <description>What this skill does</description>
    <location>/absolute/path/to/SKILL.md</location>
  </skill>
  <skill source="assistant">
    <name>assistant-skill</name>
    <description>Assistant-specific skill</description>
    <location>/absolute/path/to/SKILL.md</location>
  </skill>
</available_skills>
```

**NOT** `<skills>` / `<file>` (old non-standard format used by SkillsContextProvider before the fix).

---

## 7. Porting Checklist

- [ ] **`agent/tools.rs`**: Add `"skills"` to `extract_builtin_tool_ids()` — both the `allowed_aliases` branch AND the `None`/default branch
- [ ] **`lifecycle/app_setup.rs`**: Add `BUNDLED_SKILL_MARKER` constant
- [ ] **`lifecycle/app_setup.rs`**: Add `copy_bundled_skills_to_app_data()` function
- [ ] **`lifecycle/app_setup.rs`**: Add `copy_dir_recursive()` helper
- [ ] **`lifecycle/app_setup.rs`**: Call `copy_bundled_skills_to_app_data(app)` in `setup_app()`
- [ ] **`agent/context/mod.rs`**: Comment out `pub mod skills;`
- [ ] **`session_manager.rs` (or ContextRegistry builder)**: Remove `SkillsContextProvider` registration
- [ ] **`tauri.conf.json`**: Verify `"bundled_skills/**/*"` is in `bundle.resources`
- [ ] **Verify**: `bundled_skills/` directory exists in repo root (should have ~10 skills with `.force_update` markers)
- [ ] **Compile check**: `cargo build` with no errors
- [ ] **Runtime check**: Launch app, verify `AppData/skills/` populated, skills appear in agent system prompt

---

## 8. Compilation Errors to Watch For

When adding `"skills"` to `extract_builtin_tool_ids()`, the compiler may catch mismatches.

Also, three files had unused `CommandExt` import errors in dev/0.4.0 — these may or may not exist in dev/0.5.x:

- `src-tauri/src/mcp/builtin/workspace/mod.rs`
- `src-tauri/src/mcp/builtin/workspace/handlers/terminal.rs`  
- `src-tauri/src/mcp/builtin/workspace/persistent_shell.rs`

If these errors appear: remove the `use crate::mcp::utils::command_helper::CommandExt;` line.

---

## 9. Quick Pre-Port Diff Commands

```bash
# Check current state of extract_builtin_tool_ids on dev/0.5.x
grep -n "skills\|planning\|knowledge\|browser" src-tauri/src/agent/tools.rs

# Check if copy_bundled_skills exists on dev/0.5.x
grep -n "copy_bundled_skills\|bundled_skill" src-tauri/src/lifecycle/app_setup.rs

# Check SkillsContextProvider registration status
grep -rn "SkillsContextProvider\|pub mod skills" src-tauri/src/agent/

# Check tauri.conf.json resources
grep -A5 '"resources"' src-tauri/tauri.conf.json
```
