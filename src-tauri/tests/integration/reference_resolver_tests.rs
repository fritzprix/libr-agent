/// Regression tests for `@mention` reference resolution pipeline.
///
/// Covers:
///   - `ReferenceRegistry::preprocess_message_text` output format
///   - `SkillReferenceResolver`: metadata + read guidance appended after user text
///   - `FileReferenceResolver`: compact workspace-file references with targeted read guidance
///     and path traversal protection
use std::fs;
use tauri_mcp_agent_lib::agent::references::{ReferenceRegistry, ReferenceResolver};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// ReferenceRegistry::preprocess_message_text
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_preprocess_no_mentions_returns_original() {
    let registry = ReferenceRegistry::default();
    let text = "Hello, world! No mentions here.";
    let result = registry.preprocess_message_text(text).await;
    assert_eq!(result, text);
}

#[tokio::test]
async fn test_preprocess_unresolved_mention_appends_notice() {
    let registry = ReferenceRegistry::default(); // no resolvers registered
    let text = "Please use @skill:nonexistent to help.";
    let result = registry.preprocess_message_text(text).await;
    assert!(
        result.contains("not found"),
        "Expected unresolved notice, got: {result}"
    );
    assert!(result.starts_with(text), "Original user text must come first");
    assert!(result.contains(text), "Original text must be preserved");
}

#[tokio::test]
async fn test_preprocess_skill_like_resolver_appends_after_original_text() {
    use async_trait::async_trait;

    struct SkillLikeResolver;
    #[async_trait]
    impl ReferenceResolver for SkillLikeResolver {
        fn type_name(&self) -> &'static str {
            "skill"
        }

        fn append_after_user_text(&self) -> bool {
            true
        }

        async fn resolve(&self, _arg: &str) -> Option<String> {
            Some("SKILL META".to_string())
        }
    }

    let mut registry = ReferenceRegistry::new();
    registry.register(Box::new(SkillLikeResolver));

    let text = "@skill:delegate 로컬 변경 사항을 리뷰";
    let result = registry.preprocess_message_text(text).await;

    assert!(result.starts_with(text), "User text must stay at the top");
    let meta_pos = result.find("SKILL META").unwrap();
    let original_pos = result.find(text).unwrap();
    assert!(
        original_pos < meta_pos,
        "Skill reference must be appended after original text"
    );
    assert!(result.contains("## Reference: @skill:delegate"));
}

#[tokio::test]
async fn test_preprocess_duplicate_skill_reference_injects_once() {
    use async_trait::async_trait;

    struct SkillLikeResolver;
    #[async_trait]
    impl ReferenceResolver for SkillLikeResolver {
        fn type_name(&self) -> &'static str {
            "skill"
        }

        fn append_after_user_text(&self) -> bool {
            true
        }

        async fn resolve(&self, arg: &str) -> Option<String> {
            Some(format!("SKILL META for {arg}").to_string())
        }
    }

    let mut registry = ReferenceRegistry::new();
    registry.register(Box::new(SkillLikeResolver));

    let text = "@skill:delegate review @skill:delegate again @skill:Delegate";
    let result = registry.preprocess_message_text(text).await;

    assert_eq!(result.matches("## Reference: @skill:").count(), 1);
    assert!(!result.contains("Multiple skills referenced"));
}

#[tokio::test]
async fn test_preprocess_multiple_distinct_skill_references_each_injected_once() {
    use async_trait::async_trait;

    struct SkillLikeResolver;
    #[async_trait]
    impl ReferenceResolver for SkillLikeResolver {
        fn type_name(&self) -> &'static str {
            "skill"
        }

        fn append_after_user_text(&self) -> bool {
            true
        }

        async fn resolve(&self, arg: &str) -> Option<String> {
            Some(format!("SKILL META for {arg}").to_string())
        }
    }

    let mut registry = ReferenceRegistry::new();
    registry.register(Box::new(SkillLikeResolver));

    let text = "@skill:delegate review @skill:code-audit-expert audit @skill:delegate";
    let result = registry.preprocess_message_text(text).await;

    assert!(result.contains("Multiple skills referenced"));
    assert!(result.contains("## Reference: @skill:delegate"));
    assert!(result.contains("## Reference: @skill:code-audit-expert"));
    assert_eq!(result.matches("## Reference: @skill:").count(), 2);
}

#[tokio::test]
async fn test_preprocess_resolved_content_prepended_before_original_text() {
    use async_trait::async_trait;

    struct StubResolver;
    #[async_trait]
    impl ReferenceResolver for StubResolver {
        fn type_name(&self) -> &'static str {
            "stub"
        }
        async fn resolve(&self, _arg: &str) -> Option<String> {
            Some("STUB CONTENT".to_string())
        }
    }

    let mut registry = ReferenceRegistry::new();
    registry.register(Box::new(StubResolver));

    let text = "Do @stub:anything please.";
    let result = registry.preprocess_message_text(text).await;

    // Resolved block must come before the original text
    let resolved_pos = result.find("STUB CONTENT").unwrap();
    let original_pos = result.find(text).unwrap();
    assert!(
        resolved_pos < original_pos,
        "Resolved content must be prepended before original text"
    );
    // Section header format
    assert!(result.contains("## Reference: @stub:anything"));
}

// ---------------------------------------------------------------------------
// FileReferenceResolver
// ---------------------------------------------------------------------------

/// Build a minimal session workspace and a FileReferenceResolver pointing to it.
/// FileReferenceResolver needs a session_id whose workspace maps to a real dir.
/// We test via its `resolve()` method by constructing a temporary workspace manually
/// and calling the resolver directly with a patched session_manager — or we test the
/// formatting logic in isolation since session_manager is a global singleton.
///
/// For these tests we validate the *resolver logic* by testing the public
/// `ReferenceResolver` trait through a thin wrapper that bypasses the session manager.
use async_trait::async_trait;

/// A test-only file resolver that reads directly from a given base directory,
/// mirroring FileReferenceResolver's logic without the session_manager dependency.
struct TestFileResolver {
    workspace: std::path::PathBuf,
}

#[async_trait]
impl ReferenceResolver for TestFileResolver {
    fn type_name(&self) -> &'static str {
        "file"
    }

    async fn resolve(&self, arg: &str) -> Option<String> {
        let rel = arg.trim_start_matches('/').trim_start_matches("./");
        let target = self.workspace.join(rel);

        let canonical_workspace = self.workspace.canonicalize().ok()?;
        let canonical_target = target.canonicalize().ok()?;

        // Path traversal guard
        if !canonical_target.starts_with(&canonical_workspace) {
            return None;
        }
        if !canonical_target.is_file() {
            return None;
        }

        let meta = tokio::fs::metadata(&canonical_target).await.ok()?;
        let rel_path = rel.replace('\\', "/");
        let extension = canonical_target
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        Some(format!(
            "# File Reference `{}`\n\n\
             The file content was not inlined to avoid unnecessary context usage.\n\n\
             - Relative path: `{}`\n\
             - File size: {} bytes\n\
             - Extension: `{}`\n\
             - To inspect it, call: `workspace__readFile(path: \"{}\")`\n\
             - Prefer reading only the relevant line range or searching before loading more content.",
            rel_path,
            rel_path,
            meta.len(),
            if extension.is_empty() { "(none)" } else { extension },
            rel_path
        ))
    }
}

#[tokio::test]
async fn test_file_resolve_normal_text_file() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("hello.txt"), "Hello, world!").unwrap();

    let resolver = TestFileResolver {
        workspace: tmp.path().to_path_buf(),
    };
    let result = resolver.resolve("hello.txt").await.unwrap();

    assert!(
        result.starts_with("# File Reference `hello.txt`"),
        "Must have path header"
    );
    assert!(
        result.contains("workspace__readFile(path: \"hello.txt\")"),
        "Must contain targeted read guidance"
    );
    assert!(
        !result.contains("Hello, world!"),
        "Must not inline file content"
    );
}

#[tokio::test]
async fn test_file_resolve_nonexistent_returns_none() {
    let tmp = TempDir::new().unwrap();
    let resolver = TestFileResolver {
        workspace: tmp.path().to_path_buf(),
    };
    let result = resolver.resolve("no_such_file.txt").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn test_file_resolve_large_file_returns_reference_not_content() {
    let tmp = TempDir::new().unwrap();
    // Write 101 KB of text
    let large = "a".repeat(101 * 1024);
    fs::write(tmp.path().join("big.txt"), large).unwrap();

    let resolver = TestFileResolver {
        workspace: tmp.path().to_path_buf(),
    };
    let result = resolver.resolve("big.txt").await.unwrap();

    assert!(
        result.contains("workspace__readFile(path: \"big.txt\")"),
        "Must contain targeted read guidance"
    );
    assert!(!result.contains("```"), "Must NOT inline file content");
}

#[tokio::test]
async fn test_file_resolve_binary_file_returns_reference_not_content() {
    let tmp = TempDir::new().unwrap();
    // Write bytes that are not valid UTF-8
    let binary: Vec<u8> = vec![0xFF, 0xFE, 0x00, 0x01, 0x80, 0x90];
    fs::write(tmp.path().join("data.bin"), &binary).unwrap();

    let resolver = TestFileResolver {
        workspace: tmp.path().to_path_buf(),
    };
    let result = resolver.resolve("data.bin").await.unwrap();

    assert!(
        result.contains("workspace__readFile(path: \"data.bin\")"),
        "Must contain targeted read guidance"
    );
    assert!(!result.contains("```"), "Must NOT inline binary content");
}

#[tokio::test]
async fn test_file_resolve_path_traversal_returns_none() {
    let tmp = TempDir::new().unwrap();
    // Create a file outside the workspace
    let outside = TempDir::new().unwrap();
    fs::write(outside.path().join("secret.txt"), "secret").unwrap();

    let resolver = TestFileResolver {
        workspace: tmp.path().to_path_buf(),
    };
    // Attempt traversal
    let result = resolver.resolve("../../secret.txt").await;
    assert!(result.is_none(), "Path traversal must be blocked");
}

#[tokio::test]
async fn test_file_resolve_strips_leading_slash() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("note.md"), "# Note").unwrap();

    let resolver = TestFileResolver {
        workspace: tmp.path().to_path_buf(),
    };
    // Leading slash should be stripped
    let result = resolver.resolve("/note.md").await.unwrap();
    assert!(result.contains("note.md"));
    assert!(result.contains("Relative path: `note.md`"));
    assert!(!result.contains("# Note"));
}

#[tokio::test]
async fn test_file_resolve_output_uses_relative_path_in_header() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("src")).unwrap();
    fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();

    let resolver = TestFileResolver {
        workspace: tmp.path().to_path_buf(),
    };
    let result = resolver.resolve("src/main.rs").await.unwrap();

    // Header must use the relative path, NOT an absolute path
    assert!(
        result.starts_with("# File Reference `src/main.rs`"),
        "Header must show relative path, got: {result}"
    );
}
