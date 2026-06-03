#![cfg(not(windows))]

// This consolidated integration binary links the full Tauri/WebView path and
// crashes before the test harness starts on Windows (STATUS_ENTRYPOINT_NOT_FOUND).
// Keep Linux/macOS coverage here until the test layout is split into Windows-safe targets.

// LibrAgent Consolidated Integration Tests
mod common;
mod integration;
