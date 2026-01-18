# Bootstrap Module Refactoring Plan

**Date:** January 15, 2026  
**Module:** `src-tauri/src/mcp/builtin/bootstrap/`  
**Priority:** High  
**Estimated Effort:** 4-6 hours

---

## Executive Summary

The bootstrap module violates critical AI agent interaction best practices, resulting in poor tool usability. This plan addresses formatting, guidance, and error handling issues to align with established patterns from browser, planning, and workspace tools.

**Key Issues:**

- Raw JSON dumps instead of formatted text output
- Missing success hints and next-step guidance
- No visual markers for AI readability
- Empty service context (missed optimization)
- Minimal tool descriptions lacking workflow context

---

## Refactoring Phases

### Phase 1: Core Response Formatting (2 hours)

**Objective:** Replace raw JSON dumps with formatted, human-readable text using `SuccessHint` pattern.

#### Task 1.1: Refactor `detect_platform()` Method

**File:** `src-tauri/src/mcp/builtin/bootstrap/mod.rs`

**Current State:**

```rust
fn detect_platform(&self) -> MCPResult {
    let platform = platform::detect_current_platform();

    MCPResult {
        content: Some(vec![MCPContent::Text {
            text: serde_json::to_string_pretty(&platform).unwrap(),
        }]),
        structured_content: Some(json!(platform)),
        is_error: Some(false),
    }
}
```

**Target State:**

```rust
fn detect_platform(&self) -> MCPResult {
    let platform = platform::detect_current_platform();

    let text = format!(
        "✓ Platform detected:\n\n\
         OS: {}\n\
         Architecture: {}\n\
         Shell: {}\n\
         Home Directory: {}\n\
         Temp Directory: {}",
        platform.os,
        platform.arch,
        platform.shell,
        platform.home_dir.as_deref().unwrap_or("N/A"),
        platform.temp_dir
    );

    let hint = SuccessHint::new(
        text,
        vec![
            "Use getBootstrapGuide(tool) to get installation instructions".to_string(),
            "Available tools: node, python, uv, docker, git".to_string(),
        ]
    );

    hint.to_mcp_result_with_data(Some(json!(platform)))
}
```

**Changes:**

- Import `SuccessHint` from `crate::mcp::builtin::error_guidance`
- Format platform info as labeled, multi-line text with ✓ marker
- Add next-step suggestions (tool discovery)
- Preserve `structured_content` for UI components

**Validation:**

- AI can read OS, arch, shell without parsing JSON
- Suggestions guide to next logical action
- Test: `cargo test test_detect_platform`

---

#### Task 1.2: Create Installation Guide Formatter

**File:** `src-tauri/src/mcp/builtin/bootstrap/guides.rs`

**New Function:**

```rust
impl InstallationGuide {
    /// Format guide as human-readable text for AI agents
    pub fn format_as_text(&self) -> String {
        let steps_text = self.steps.iter()
            .enumerate()
            .map(|(i, step)| {
                let mut parts = vec![format!("{}. {}", i + 1, step.description)];

                if let Some(cmd) = &step.command {
                    parts.push(format!("   $ {}", cmd));
                }

                if let Some(url) = &step.url {
                    parts.push(format!("   🔗 {}", url));
                }

                parts.join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let notes_text = if !self.notes.is_empty() {
            format!("\n\n📝 Notes:\n• {}", self.notes.join("\n• "))
        } else {
            String::new()
        };

        format!(
            "✓ Installation guide for {} on {}:\n\n\
             {}\n\n\
             📋 Verification:\n\
             $ {}{}",
            self.tool,
            self.platform,
            steps_text,
            self.verification,
            notes_text
        )
    }
}
```

**Changes:**

- Add method to `InstallationGuide` struct
- Format steps with numbered list, command prefix ($), URL emoji (🔗)
- Add visual markers (✓, 📋, 📝)
- Preserve line breaks for readability

**Validation:**

- Unit test verifying formatted output
- No JSON escape sequences in output
- Steps are sequentially numbered

---

#### Task 1.3: Refactor `get_bootstrap_guide()` Method

**File:** `src-tauri/src/mcp/builtin/bootstrap/mod.rs`

**Target State:**

```rust
fn get_bootstrap_guide(&self, args: Value) -> MCPResult {
    let tool = match args.get("tool").and_then(|v| v.as_str()) {
        Some(t) => {
            if t.trim().is_empty() {
                return invalid_input_error(
                    "Tool name cannot be empty",
                    ToolGroup::Bootstrap
                );
            }
            t
        }
        None => return missing_param_error("tool", ToolGroup::Bootstrap),
    };

    // Validate tool name
    let valid_tools = ["node", "python", "uv", "docker", "git"];
    if !valid_tools.contains(&tool) {
        return invalid_input_error(
            &format!(
                "Invalid tool '{}'. Must be one of: {}",
                tool,
                valid_tools.join(", ")
            ),
            ToolGroup::Bootstrap,
        );
    }

    let platform = args.get("platform").and_then(|v| v.as_str());

    // Validate platform if provided
    if let Some(p) = platform {
        let valid_platforms = ["windows", "linux", "darwin", "auto"];
        if !valid_platforms.contains(&p) {
            return invalid_input_error(
                &format!(
                    "Invalid platform '{}'. Must be one of: {}",
                    p,
                    valid_platforms.join(", ")
                ),
                ToolGroup::Bootstrap,
            );
        }
    }

    let guide = guides::get_installation_guide(tool, platform);
    let formatted_text = guide.format_as_text();

    let hint = SuccessHint::new(
        formatted_text,
        vec![
            format!("Run: {} to verify installation", guide.verification),
            "Use detectPlatform to check your current environment".to_string(),
        ]
    );

    hint.to_mcp_result_with_data(Some(json!(guide)))
}
```

**Changes:**

- Improve validation with explicit empty check
- Use new `format_as_text()` method
- Add verification command to suggestions
- Remove JSON serialization in text content

**Validation:**

- Test all valid tools (node, python, uv, docker, git)
- Test invalid tool rejection
- Test empty tool name rejection
- Verify formatted output readability

---

### Phase 2: Tool Descriptions Enhancement (1 hour)

**Objective:** Add workflow context and usage guidance to tool descriptions.

#### Task 2.1: Enhance `detectPlatform` Tool Description

**File:** `src-tauri/src/mcp/builtin/bootstrap/mod.rs`

**Current:**

```rust
fn create_detect_platform_tool() -> MCPTool {
    MCPTool {
        name: "detectPlatform".to_string(),
        title: Some("Detect Platform".to_string()),
        description: "Detect current operating system, architecture, and shell environment"
            .to_string(),
        input_schema: object_schema(HashMap::new(), vec![]),
        output_schema: None,
        annotations: None,
    }
}
```

**Target:**

```rust
fn create_detect_platform_tool() -> MCPTool {
    MCPTool {
        name: "detectPlatform".to_string(),
        title: Some("Detect Platform".to_string()),
        description: "Detect current operating system, architecture, and shell environment

Use this tool to:
• Identify platform-specific requirements before installation
• Verify system compatibility with development tools
• Get accurate environment information for troubleshooting

Returns: OS type (windows/darwin/linux), CPU architecture (x64/arm64), default shell, home directory path, and temp directory path

💡 Next Steps:
• Use getBootstrapGuide(tool) to get installation instructions for your detected platform
• Available tools: node, python, uv, docker, git".to_string(),
        input_schema: object_schema(HashMap::new(), vec![]),
        output_schema: None,
        annotations: None,
    }
}
```

---

#### Task 2.2: Enhance `getBootstrapGuide` Tool Description

**File:** `src-tauri/src/mcp/builtin/bootstrap/mod.rs`

**Target:**

```rust
fn create_get_bootstrap_guide_tool() -> MCPTool {
    let mut props = HashMap::new();
    props.insert(
        "tool".to_string(),
        enum_prop_required(
            vec!["node", "python", "uv", "docker", "git"],
            "Development tool to install (node, python, uv, docker, git)",
        ),
    );
    props.insert(
        "platform".to_string(),
        enum_prop(
            vec!["windows", "linux", "darwin", "auto"],
            "auto",
            Some("Target platform (auto = detect automatically, windows = Windows, darwin = macOS, linux = Linux)"),
        ),
    );

    MCPTool {
        name: "getBootstrapGuide".to_string(),
        title: Some("Get Bootstrap Guide".to_string(),
        description: "Get step-by-step installation guide for common development tools

Supported Tools:
• node - Node.js runtime and npm package manager
• python - Python interpreter and pip
• uv - Ultra-fast Python package installer
• docker - Docker container platform
• git - Version control system

The guide includes:
• Platform-specific installation commands
• Download URLs for installers
• Verification commands to test installation
• Post-installation notes and configuration tips

💡 Workflow:
1. (Optional) Call detectPlatform to identify your system
2. Call getBootstrapGuide(tool, platform) to get instructions
3. Follow the numbered steps in the response
4. Run verification command to confirm installation".to_string(),
        input_schema: object_schema(props, vec!["tool".to_string()]),
        output_schema: None,
        annotations: None,
    }
}
```

---

### Phase 3: Service Context Implementation (30 minutes)

**Objective:** Provide platform info automatically to AI agents via service context.

#### Task 3.1: Implement Service Context with Caching

**File:** `src-tauri/src/mcp/builtin/bootstrap/mod.rs`

**Add to struct:**

```rust
use std::sync::{Arc, RwLock};
use std::time::Instant;

#[derive(Debug)]
pub struct BootstrapServer {
    platform_cache: Arc<RwLock<Option<(platform::PlatformInfo, Instant)>>>,
}

impl BootstrapServer {
    pub fn new() -> Self {
        Self {
            platform_cache: Arc::new(RwLock::new(None)),
        }
    }

    fn invalidate_cache(&self) {
        if let Ok(mut cache) = self.platform_cache.write() {
            *cache = None;
        }
    }
}
```

**Update service context:**

```rust
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    const CACHE_TTL_SECS: u64 = 30; // Platform rarely changes

    // Check cache first
    if let Ok(cache_guard) = self.platform_cache.read() {
        if let Some((platform, last_update)) = cache_guard.as_ref() {
            if last_update.elapsed().as_secs() < CACHE_TTL_SECS {
                return ServiceContext {
                    context_prompt: format!(
                        "## Bootstrap\n\nCurrent platform: {} ({}) using {}",
                        platform.os, platform.arch, platform.shell
                    ),
                    structured_state: Some(json!(platform)),
                };
            }
        }
    }

    // Cache miss - detect platform
    let platform = platform::detect_current_platform();

    // Update cache
    if let Ok(mut cache_guard) = self.platform_cache.write() {
        *cache_guard = Some((platform.clone(), Instant::now()));
    }

    ServiceContext {
        context_prompt: format!(
            "## Bootstrap\n\nCurrent platform: {} ({}) using {}",
            platform.os, platform.arch, platform.shell
        ),
        structured_state: Some(json!(platform)),
    }
}
```

**Benefits:**

- AI agents see platform info without calling `detectPlatform`
- 30-second cache reduces redundant system calls
- Enables platform-aware decision making

---

### Phase 4: Error Handling Improvements (30 minutes)

**Objective:** Add more specific error guidance and proactive validation.

#### Task 4.1: Enhanced Parameter Validation

**File:** `src-tauri/src/mcp/builtin/bootstrap/mod.rs`

**Already covered in Task 1.3** - explicit empty string check added.

#### Task 4.2: Add Tool-Specific Error Guidance

**File:** `src-tauri/src/mcp/builtin/bootstrap/mod.rs`

**Add error helper function:**

```rust
use crate::mcp::builtin::error_guidance::{
    ErrorCategory, ErrorGuidance, ToolGroup
};

fn bootstrap_error(
    operation: &str,
    details: &str,
    suggestions: Vec<String>,
) -> MCPResult {
    ErrorGuidance::new(
        ErrorCategory::InvalidInput,
        operation,
        details,
        suggestions,
        ToolGroup::Bootstrap,
    ).to_mcp_result()
}
```

**Usage example (already using standard errors, but could add custom guidance):**

```rust
// For invalid tool
return bootstrap_error(
    "Get installation guide",
    &format!("Tool '{}' is not supported", tool),
    vec![
        "Available tools: node, python, uv, docker, git".to_string(),
        "Use detectPlatform to check your environment first".to_string(),
        "Example: getBootstrapGuide(tool=\"node\", platform=\"auto\")".to_string(),
    ],
);
```

---

### Phase 5: Testing & Validation (1 hour)

**Objective:** Ensure all changes work correctly and maintain backward compatibility.

#### Task 5.1: Update Unit Tests

**File:** `src-tauri/src/mcp/builtin/bootstrap/mod.rs`

**Add tests for formatted output:**

```rust
#[tokio::test]
async fn test_detect_platform_formatted_output() {
    let server = BootstrapServer::new();
    let result = server.call_tool("detectPlatform", json!({}), None).await.unwrap();

    assert_eq!(result.is_error, Some(false));

    let content = result.content.unwrap();
    let text = match &content[0] {
        MCPContent::Text { text } => text,
        _ => panic!("Expected text content"),
    };

    // Verify visual markers
    assert!(text.contains("✓ Platform detected"));
    // Verify labeled fields
    assert!(text.contains("OS:"));
    assert!(text.contains("Architecture:"));
    assert!(text.contains("Shell:"));
    // Verify guidance marker
    assert!(text.contains("💡 Next"));
}

#[tokio::test]
async fn test_get_bootstrap_guide_formatted_output() {
    let server = BootstrapServer::new();
    let result = server.call_tool(
        "getBootstrapGuide",
        json!({"tool": "node", "platform": "windows"}),
        None,
    ).await.unwrap();

    assert_eq!(result.is_error, Some(false));

    let content = result.content.unwrap();
    let text = match &content[0] {
        MCPContent::Text { text } => text,
        _ => panic!("Expected text content"),
    };

    // Verify visual markers
    assert!(text.contains("✓ Installation guide"));
    // Verify numbered steps
    assert!(text.contains("1."));
    // Verify command prefix
    assert!(text.contains("$"));
    // Verify verification section
    assert!(text.contains("📋 Verification"));
    // Verify notes section
    assert!(text.contains("📝 Notes"));
}

#[tokio::test]
async fn test_empty_tool_name_validation() {
    let server = BootstrapServer::new();
    let result = server.call_tool(
        "getBootstrapGuide",
        json!({"tool": "   "}),
        None,
    ).await.unwrap();

    assert_eq!(result.is_error, Some(true));

    let content = result.content.unwrap();
    let text = match &content[0] {
        MCPContent::Text { text } => text,
        _ => panic!("Expected text content"),
    };

    assert!(text.contains("Tool name cannot be empty"));
}

#[tokio::test]
async fn test_service_context_provides_platform() {
    let server = BootstrapServer::new();
    let context = server.get_service_context(None).await;

    assert!(!context.context_prompt.is_empty());
    assert!(context.context_prompt.contains("## Bootstrap"));
    assert!(context.context_prompt.contains("Current platform:"));
    assert!(context.structured_state.is_some());
}

#[tokio::test]
async fn test_service_context_caching() {
    let server = BootstrapServer::new();

    // First call
    let context1 = server.get_service_context(None).await;
    let text1 = context1.context_prompt.clone();

    // Second call (should use cache)
    let context2 = server.get_service_context(None).await;
    let text2 = context2.context_prompt;

    assert_eq!(text1, text2);
}
```

#### Task 5.2: Add Integration Tests

**File:** `src-tauri/src/mcp/builtin/bootstrap/guides.rs`

**Test formatter:**

```rust
#[test]
fn test_installation_guide_formatter() {
    let guide = InstallationGuide {
        tool: "test-tool".to_string(),
        platform: "test-platform".to_string(),
        steps: vec![
            InstallationStep {
                description: "Step one".to_string(),
                command: Some("command1".to_string()),
                url: None,
            },
            InstallationStep {
                description: "Step two".to_string(),
                command: None,
                url: Some("https://example.com".to_string()),
            },
        ],
        verification: "verify command".to_string(),
        notes: vec!["Note 1".to_string(), "Note 2".to_string()],
    };

    let formatted = guide.format_as_text();

    // Check structure
    assert!(formatted.contains("✓ Installation guide"));
    assert!(formatted.contains("1. Step one"));
    assert!(formatted.contains("$ command1"));
    assert!(formatted.contains("2. Step two"));
    assert!(formatted.contains("🔗 https://example.com"));
    assert!(formatted.contains("📋 Verification"));
    assert!(formatted.contains("$ verify command"));
    assert!(formatted.contains("📝 Notes"));
    assert!(formatted.contains("• Note 1"));
    assert!(formatted.contains("• Note 2"));
}
```

#### Task 5.3: Manual Testing Checklist

**Test Cases:**

- [ ] Run `detectPlatform` - verify formatted output with ✓ marker
- [ ] Run `getBootstrapGuide(tool="node")` - verify numbered steps
- [ ] Run `getBootstrapGuide(tool="python", platform="windows")` - verify platform-specific guide
- [ ] Run `getBootstrapGuide(tool="invalid")` - verify error with ✗ marker
- [ ] Run `getBootstrapGuide(tool="")` - verify empty validation error
- [ ] Check service context in system prompt - verify platform info appears
- [ ] Test all 5 tools (node, python, uv, docker, git) on all 3 platforms
- [ ] Verify structured_content still contains JSON (for UI components)

---

### Phase 6: Documentation Updates (30 minutes)

**Objective:** Update documentation to reflect new patterns.

#### Task 6.1: Update Module README

**File:** `src-tauri/src/mcp/builtin/bootstrap/README.md` (create if not exists)

````markdown
# Bootstrap Server

Platform detection and development tool installation guides.

## Features

- **Platform Detection**: Identify OS, architecture, and shell environment
- **Installation Guides**: Step-by-step instructions for common dev tools
- **Auto-formatted Output**: Human-readable text with visual markers
- **Service Context**: Platform info automatically available to agents

## Tools

### detectPlatform

Detects current system platform information.

**No parameters required**

**Returns:**

- OS type (windows/darwin/linux)
- CPU architecture (x64/arm64)
- Default shell (powershell/bash/zsh/etc)
- Home directory path
- Temp directory path

**Example Usage:**

```json
{
  "tool": "detectPlatform",
  "args": {}
}
```
````

### getBootstrapGuide

Get installation guide for development tools.

**Parameters:**

- `tool` (required): Tool name (node, python, uv, docker, git)
- `platform` (optional): Target platform (auto/windows/darwin/linux, default: auto)

**Returns:**

- Numbered installation steps
- Platform-specific commands
- Download URLs
- Verification commands
- Post-installation notes

**Example Usage:**

```json
{
  "tool": "getBootstrapGuide",
  "args": {
    "tool": "node",
    "platform": "auto"
  }
}
```

## Service Context

The bootstrap server provides platform information in service context:

```
## Bootstrap

Current platform: windows (x64) using powershell
```

This allows agents to make platform-aware decisions without calling detectPlatform.

## Best Practices

1. **Call detectPlatform first** when environment is unknown
2. **Use platform="auto"** to auto-detect target platform
3. **Run verification commands** after installation to confirm success
4. **Check service context** before calling detectPlatform (platform info is cached)

## Implementation Notes

- Platform info is cached for 30 seconds to reduce system calls
- All responses use SuccessHint pattern for consistency
- Visual markers (✓, 📋, 📝, 🔗) improve AI readability
- Formatted text output instead of JSON dumps

````

#### Task 6.2: Add to Built-in Tools Documentation
**File:** `docs/builtin-tools.md` (update Bootstrap section)

Update the Bootstrap section with:
- New formatted output examples
- Service context description
- Usage workflow
- Visual markers explanation

---

## Implementation Order

### Day 1 (3 hours)
1. **Morning:** Phase 1 (Tasks 1.1, 1.2, 1.3) - Core formatting
2. **Afternoon:** Phase 2 (Tasks 2.1, 2.2) - Tool descriptions

### Day 2 (2 hours)
3. **Morning:** Phase 3 (Task 3.1) - Service context
4. **Afternoon:** Phase 5 (Tasks 5.1, 5.2, 5.3) - Testing

### Day 3 (1 hour)
5. **Morning:** Phase 6 (Tasks 6.1, 6.2) - Documentation

---

## Validation Criteria

### Must Pass:
- ✅ All existing unit tests pass
- ✅ New tests for formatted output pass
- ✅ No raw JSON in text content fields
- ✅ All responses include visual markers (✓ or ✗)
- ✅ All success responses include 💡 Next steps
- ✅ Service context returns non-empty platform info
- ✅ Tool descriptions include workflow guidance
- ✅ `cargo clippy` passes with no warnings
- ✅ `cargo test` passes all tests
- ✅ Manual testing checklist completed

### Nice to Have:
- 🎯 Response formatting matches browser/planning tools
- 🎯 Cache hit rate > 80% in service context
- 🎯 AI agent successfully uses tools without human intervention

---

## Rollback Plan

If issues arise after deployment:

1. **Revert commits** in reverse order (Phase 6 → Phase 1)
2. **Critical fix priority:**
   - Phase 1 (core formatting) - highest priority
   - Phase 3 (service context) - medium priority
   - Phase 2 (descriptions) - low priority

3. **Emergency rollback:**
   ```bash
   git revert <commit-hash-range>
   cargo test
   cargo build --release
````

---

## Success Metrics

### Pre-Refactoring:

- AI agents receive raw JSON dumps
- No guidance on next steps
- Empty service context
- Manual tool usage required

### Post-Refactoring:

- AI agents receive formatted, readable text
- 2+ actionable suggestions per response
- Platform info available in service context
- AI agents can chain tools autonomously

### Measurement:

- Manual review of 10 agent sessions using bootstrap tools
- Verify AI correctly interprets platform info
- Verify AI follows suggested next steps
- No JSON parsing errors in agent conversations

---

## Dependencies

### Required:

- `SuccessHint` from `crate::mcp::builtin::error_guidance`
- `ErrorGuidance` from `crate::mcp::builtin::error_guidance`
- Standard library: `Arc`, `RwLock`, `Instant`

### Optional:

- None (all dependencies already in project)

---

## Risk Assessment

| Risk                           | Impact | Probability | Mitigation                           |
| ------------------------------ | ------ | ----------- | ------------------------------------ |
| Breaking existing integrations | High   | Low         | Preserve `structured_content` format |
| Performance regression         | Medium | Low         | Cache platform info, 30s TTL         |
| Test failures                  | Medium | Medium      | Comprehensive test coverage          |
| Formatting inconsistencies     | Low    | Medium      | Follow established patterns          |

---

## Appendix: Code Snippets

### A. Import Statements to Add

```rust
// In mod.rs
use crate::mcp::builtin::error_guidance::SuccessHint;
use std::sync::{Arc, RwLock};
use std::time::Instant;
```

### B. Visual Marker Reference

```rust
// Success marker
"✓ Operation successful"

// Error marker
"✗ Operation failed"

// Guidance marker
"💡 Next: Use toolName"

// Section markers
"📋 Verification"
"📝 Notes"
"🔗 https://example.com"
```

### C. Testing Helper

```rust
fn extract_text_content(result: &MCPResult) -> String {
    match result.content.as_ref().unwrap()[0] {
        MCPContent::Text { text } => text.clone(),
        _ => panic!("Expected text content"),
    }
}
```

---

## Notes

- This refactoring maintains backward compatibility (structured_content unchanged)
- All changes follow patterns from browser/planning/workspace tools
- Service context caching reduces system call overhead
- Visual markers improve both AI and human readability
- Formatted text enables better AI decision-making

---

**Plan Status:** Draft  
**Review Required:** Yes  
**Approval Needed From:** Core team  
**Target Completion:** 3 working days after approval
