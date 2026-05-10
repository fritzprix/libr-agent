use base64::{engine::general_purpose, Engine as _};

/// Redact sensitive input from output string
///
/// Note: This uses simple string replacement which may result in over-redaction
/// (e.g. "pass" will be redacted in "compass"). This is intentional for security
/// as over-redaction is safer than under-redaction in this context.
pub fn redact_sensitive_input(output: &str, sensitive: &str) -> String {
    if sensitive.is_empty() {
        return output.to_string();
    }
    output.replace(sensitive, "********")
}

/// Decode interactive input payloads.
///
/// Interactive UI resources send raw UTF-8 bytes encoded as base64. This is transport
/// encoding only, not a security boundary. The UI surface is a trusted local
/// Tauri/MCP resource; this encoding exists to safely move bytes through the
/// postMessage/tool-callback boundary, not to conceal secrets from an attacker
/// who can already inspect the local UI process.
pub fn decode_input_payload(input_base64: &str) -> Result<String, String> {
    let input_bytes = general_purpose::STANDARD
        .decode(input_base64)
        .map_err(|e| format!("Input must be base64-encoded UTF-8. Decode error: {e}"))?;

    String::from_utf8(input_bytes).map_err(|e| format!("Decoded input was not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_sensitive_input() {
        let input = "password123";
        let output = "Enter password: password123\nAccess granted";
        let redacted = redact_sensitive_input(output, input);
        assert_eq!(redacted, "Enter password: ********\nAccess granted");

        // Test multiple occurrences
        let output2 = "password123 is the password123";
        let redacted2 = redact_sensitive_input(output2, input);
        assert_eq!(redacted2, "******** is the ********");

        // Test empty input (should not change output)
        let output3 = "normal output";
        let redacted3 = redact_sensitive_input(output3, "");
        assert_eq!(redacted3, "normal output");
    }

    #[test]
    fn test_decode_input_payload() {
        let original = "password123";
        let encoded = general_purpose::STANDARD.encode(original.as_bytes());

        let decoded = decode_input_payload(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_decode_input_payload_requires_base64() {
        let result_invalid = decode_input_payload("not base64");
        assert!(result_invalid.is_err());
        assert!(result_invalid
            .unwrap_err()
            .contains("Input must be base64-encoded UTF-8"));
    }

    #[test]
    fn test_decode_input_payload_requires_utf8() {
        let invalid_utf8 = general_purpose::STANDARD.encode([0xff, 0xfe, 0xfd]);
        let result_invalid = decode_input_payload(&invalid_utf8);
        assert!(result_invalid.is_err());
        assert!(result_invalid
            .unwrap_err()
            .contains("Decoded input was not valid UTF-8"));
    }
}
