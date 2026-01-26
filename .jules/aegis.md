# Aegis Safety Violation Log

## 2026-01-25 - src-tauri/src/mcp/builtin/browser/interaction.rs **Safety Breach:** `unwrap()` on `serde_json::to_string` inside `click_element` and `input_text`. **Fix:** Replaced with `map_err` to propagate errors properly.

## 2026-01-25 - src-tauri/src/mcp/builtin/browser/content.rs **Safety Breach:** `unwrap()` on `Regex::new` inside `convert_to_markdown` causing repeated recompilation and potential panic. **Fix:** Extracted to `static Lazy<Regex>` constants for compile-time-like validation and performance.
