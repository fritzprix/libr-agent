//! Narrow waiting-prompt detection for isolated shell success paths.
//! Avoids false positives that previously fired on `git diff` / status noise.

use tauri_mcp_agent_lib::mcp::builtin::workspace::code_execution::validation::looks_like_waiting_prompt;

#[test]
fn git_diff_and_status_noise_is_not_a_waiting_prompt() {
    let diff = r#"
diff --git a/src/lib/ai-service/factory.ts b/src/lib/ai-service/factory.ts
--- a/src/lib/ai-service/factory.ts
+++ b/src/lib/ai-service/factory.ts
@@ -1,3 +1,4 @@
+  const options?: Options;
   // confirm strategy selection
"#;
    assert!(!looks_like_waiting_prompt(diff, ""));
    assert!(!looks_like_waiting_prompt("? untracked.ts\n M modified.ts", ""));
    assert!(!looks_like_waiting_prompt(
        "Operation cancelled\nno changes made\nskipping",
        ""
    ));
}

#[test]
fn strong_tail_prompts_are_detected() {
    assert!(looks_like_waiting_prompt(
        "Installing...\nOverwrite package.json? (y/n)",
        ""
    ));
    assert!(looks_like_waiting_prompt("", "Enter password:"));
    assert!(looks_like_waiting_prompt("Continue [yes/no]", ""));
}
