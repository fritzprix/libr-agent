//! Windows-safe encoding decode tests (not behind cfg(not(windows))).

use tauri_mcp_agent_lib::mcp::builtin::workspace::text_encoding::{decode_text_bytes, DecodedText};

#[test]
fn decodes_valid_utf8() {
    match decode_text_bytes(b"hello\nworld") {
        DecodedText::Text { text, note } => {
            assert_eq!(text, "hello\nworld");
            assert!(note.is_none());
        }
        DecodedText::Binary => panic!("expected text"),
    }
}

#[test]
fn strips_utf8_bom() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice("한글".as_bytes());
    match decode_text_bytes(&bytes) {
        DecodedText::Text { text, note } => {
            assert_eq!(text, "한글");
            assert!(note.is_some());
        }
        DecodedText::Binary => panic!("expected text"),
    }
}

#[test]
fn decodes_utf16le_bom() {
    // "Hi" in UTF-16LE with BOM
    let bytes = [0xFF, 0xFE, b'H', 0x00, b'i', 0x00];
    match decode_text_bytes(&bytes) {
        DecodedText::Text { text, note } => {
            assert_eq!(text, "Hi");
            assert_eq!(note, Some("decoded as UTF-16LE"));
        }
        DecodedText::Binary => panic!("expected text"),
    }
}

#[test]
fn rejects_binary_with_nuls() {
    let bytes = b"abc\0def\0ghi";
    assert_eq!(decode_text_bytes(bytes), DecodedText::Binary);
}

#[test]
fn lossy_fallback_for_invalid_utf8_without_nuls() {
    // Invalid UTF-8 continuation without NULs
    let bytes = [0xC3, 0x28]; // invalid seq
    match decode_text_bytes(&bytes) {
        DecodedText::Text { text, note } => {
            assert!(!text.is_empty());
            // On Windows ACP may succeed; otherwise lossy note.
            assert!(note.is_some());
        }
        DecodedText::Binary => panic!("expected text via ACP or lossy"),
    }
}
