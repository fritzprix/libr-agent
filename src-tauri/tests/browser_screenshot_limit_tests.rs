use tauri_mcp_agent_lib::browser_sidecar::{
    validate_full_page_dimensions, validate_screenshot_png_bytes, MAX_FULL_PAGE_PIXELS,
    MAX_SCREENSHOT_BYTES,
};

#[test]
fn full_page_dimensions_reject_non_finite_values() {
    assert!(validate_full_page_dimensions(f64::NAN, 100.0).is_err());
    assert!(validate_full_page_dimensions(100.0, f64::INFINITY).is_err());
}

#[test]
fn full_page_dimensions_reject_oversized_content() {
    // Slightly over the 64-million-pixel budget.
    let width = 8_000.0;
    let height = (MAX_FULL_PAGE_PIXELS / width).floor() + 1.0;
    let error = validate_full_page_dimensions(width, height).expect_err("should reject");
    assert!(
        error.contains("too large"),
        "error should mention size limit, got: {error}"
    );
}

#[test]
fn full_page_dimensions_accept_boundary_and_typical_pages() {
    assert!(validate_full_page_dimensions(1_920.0, 1_080.0).is_ok());
    assert!(validate_full_page_dimensions(8_000.0, 8_000.0).is_ok()); // 64M exactly
}

#[test]
fn screenshot_png_bytes_reject_over_limit() {
    let oversized = vec![0_u8; MAX_SCREENSHOT_BYTES + 1];
    let error = validate_screenshot_png_bytes(&oversized).expect_err("should reject");
    assert!(
        error.contains("too large"),
        "error should mention byte limit, got: {error}"
    );
}

#[test]
fn screenshot_png_bytes_accept_at_limit() {
    let at_limit = vec![0_u8; MAX_SCREENSHOT_BYTES];
    assert!(validate_screenshot_png_bytes(&at_limit).is_ok());
    assert!(validate_screenshot_png_bytes(&[]).is_ok());
}
