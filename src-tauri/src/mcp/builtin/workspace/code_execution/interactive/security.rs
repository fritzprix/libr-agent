use base64::{engine::general_purpose, Engine as _};
use tracing::warn;

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

/// De-obfuscate input using XOR and Base64
pub fn deobfuscate_input(input_base64: &str, nonce: &str) -> Result<String, String> {
    // If input doesn't look like base64 (e.g. plain text fallback), return as is
    // But for security, we should expect base64 if nonce was provided.
    // For backward compatibility or direct tool calls, we might need to handle plain text.
    // However, since this is a security feature, we assume the UI sends obfuscated data.

    let input_bytes = match general_purpose::STANDARD.decode(input_base64) {
        Ok(b) => b,
        Err(e) => {
            if !nonce.is_empty() {
                // Security: fail if nonce is present and decoding fails
                return Err(format!(
                    "Input must be base64-obfuscated when nonce is provided. Decode error: {e}"
                ));
            } else {
                // For legacy/plain text, allow fallback but log a warning
                warn!("Base64 decode failed, falling back to plain text input: {e}");
                return Ok(input_base64.to_string());
            }
        }
    };

    let nonce_bytes = nonce.as_bytes();
    if nonce_bytes.is_empty() {
        return Ok(input_base64.to_string());
    }

    let xored: Vec<u8> = input_bytes
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ nonce_bytes[i % nonce_bytes.len()])
        .collect();

    String::from_utf8(xored).map_err(|e| format!("UTF-8 decode failed: {e}"))
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
    fn test_deobfuscate_input() {
        // "password123" XOR "nonce" -> base64
        // nonce = "nonce" (5 bytes)
        // p (112) ^ n (110) = 2
        // a (97) ^ o (111) = 14
        // s (115) ^ n (110) = 29
        // s (115) ^ c (99) = 16
        // w (119) ^ e (101) = 22
        // o (111) ^ n (110) = 1
        // r (114) ^ o (111) = 29
        // d (100) ^ n (110) = 10
        // 1 (49) ^ c (99) = 82
        // 2 (50) ^ e (101) = 87
        // 3 (51) ^ n (110) = 93

        let nonce = "nonce";
        let original = "password123";

        // Manual XOR for verification
        let original_bytes = original.as_bytes();
        let nonce_bytes = nonce.as_bytes();
        let xored: Vec<u8> = original_bytes
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ nonce_bytes[i % nonce_bytes.len()])
            .collect();
        let encoded = general_purpose::STANDARD.encode(&xored);

        // Test deobfuscation
        let decoded = deobfuscate_input(&encoded, nonce).unwrap();
        assert_eq!(decoded, original);

        // Test with empty nonce (should return input as is)
        let decoded_empty = deobfuscate_input(original, "").unwrap();
        assert_eq!(decoded_empty, original);

        // Test with invalid base64 (should return error when nonce is provided)
        let result_invalid = deobfuscate_input("not base64", nonce);
        assert!(result_invalid.is_err());
        assert!(result_invalid
            .unwrap_err()
            .contains("Input must be base64-obfuscated"));
    }
}
