//! Decode arbitrary file/process bytes into UTF-8 text for agent-facing tools.
//! Prefer real encodings when detectable; fall back to lossy UTF-8 instead of failing.

#[cfg(windows)]
use windows_sys::Win32::Globalization::{GetACP, MultiByteToWideChar};

/// Result of decoding file bytes for text tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedText {
    /// Decoded text. `note` is set when a non-UTF-8 path or lossy conversion was used.
    Text {
        text: String,
        note: Option<&'static str>,
    },
    /// Looks like a binary file (embedded NULs and not UTF-16 text).
    Binary,
}

/// Decode bytes into UTF-8 text for `readFile` and similar tools.
///
/// Order:
/// 1. UTF-8 / UTF-8 BOM
/// 2. UTF-16 LE/BE (BOM or heuristic)
/// 3. Windows ANSI code page (e.g. CP949) when not valid UTF-8
/// 4. Lossy UTF-8 (never fails solely for encoding)
///
/// Returns [`DecodedText::Binary`] only when the payload looks binary (NULs that are
/// not explained by UTF-16).
pub fn decode_text_bytes(bytes: &[u8]) -> DecodedText {
    if bytes.is_empty() {
        return DecodedText::Text {
            text: String::new(),
            note: None,
        };
    }

    if looks_like_binary(bytes) {
        return DecodedText::Binary;
    }

    // UTF-8 BOM
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return DecodedText::Text {
            text: String::from_utf8_lossy(&bytes[3..]).into_owned(),
            note: Some("decoded with UTF-8 BOM stripped"),
        };
    }

    // UTF-16 BOM LE
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return DecodedText::Text {
            text: decode_utf16_le(&bytes[2..]),
            note: Some("decoded as UTF-16LE"),
        };
    }

    // UTF-16 BOM BE
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return DecodedText::Text {
            text: decode_utf16_be(&bytes[2..]),
            note: Some("decoded as UTF-16BE"),
        };
    }

    if looks_like_utf16le(bytes) {
        return DecodedText::Text {
            text: decode_utf16_le(bytes),
            note: Some("decoded as UTF-16LE (heuristic)"),
        };
    }

    if let Ok(text) = std::str::from_utf8(bytes) {
        return DecodedText::Text {
            text: text.to_string(),
            note: None,
        };
    }

    #[cfg(windows)]
    {
        if let Some(text) = decode_windows_acp(bytes) {
            return DecodedText::Text {
                text,
                note: Some("decoded using Windows ANSI code page (file was not UTF-8)"),
            };
        }
    }

    DecodedText::Text {
        text: String::from_utf8_lossy(bytes).into_owned(),
        note: Some("decoded with lossy UTF-8 (invalid sequences replaced)"),
    }
}

fn looks_like_binary(bytes: &[u8]) -> bool {
    if looks_like_utf16le(bytes)
        || bytes.starts_with(&[0xFF, 0xFE])
        || bytes.starts_with(&[0xFE, 0xFF])
    {
        return false;
    }
    // NULs in the sniff window usually mean binary for single-byte encodings.
    let sniff = &bytes[..bytes.len().min(8 * 1024)];
    sniff.contains(&0)
}

fn looks_like_utf16le(bytes: &[u8]) -> bool {
    if bytes.len() < 4 || !bytes.len().is_multiple_of(2) {
        return false;
    }

    let sample_len = bytes.len().min(200);
    let mut nul_count = 0;
    let mut checked = 0;

    for i in (1..sample_len).step_by(2) {
        checked += 1;
        if bytes[i] == 0 {
            nul_count += 1;
        }
    }

    checked > 0 && (nul_count * 4 >= checked * 3)
}

fn decode_utf16_le(bytes: &[u8]) -> String {
    let u16_len = bytes.len() / 2;
    let mut wide = Vec::with_capacity(u16_len);
    for chunk in bytes.chunks_exact(2) {
        wide.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    String::from_utf16_lossy(&wide)
}

fn decode_utf16_be(bytes: &[u8]) -> String {
    let u16_len = bytes.len() / 2;
    let mut wide = Vec::with_capacity(u16_len);
    for chunk in bytes.chunks_exact(2) {
        wide.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    String::from_utf16_lossy(&wide)
}

#[cfg(windows)]
fn decode_windows_acp(bytes: &[u8]) -> Option<String> {
    let code_page = unsafe { GetACP() };
    let required = unsafe {
        MultiByteToWideChar(
            code_page,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        )
    };
    if required <= 0 {
        return None;
    }

    let mut wide: Vec<u16> = vec![0; required as usize];
    let converted = unsafe {
        MultiByteToWideChar(
            code_page,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide.as_mut_ptr(),
            required,
        )
    };
    if converted <= 0 {
        return None;
    }
    wide.truncate(converted as usize);
    Some(String::from_utf16_lossy(&wide))
}
