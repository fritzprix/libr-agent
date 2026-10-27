//! The `session-relay` binaries embedded at build time (see `build_support/session_relay.rs`).
//!
//! A build without `LIBRAGENT_RELAY_DIR` embeds nothing: [`relay_binary`] is `None`, and a managed
//! Docker session then has no relay and no `$LIBRAGENT_WAKE_URL`.

use sha2::{Digest, Sha256};

/// A relay binary with the hash it was pinned to at build time.
pub struct EmbeddedRelay {
    pub bytes: &'static [u8],
    pub sha256: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/session_relay_embed.rs"));

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The bytes of `embedded`, only when they still hash to the value pinned at build time.
fn verified(embedded: Option<&EmbeddedRelay>) -> Option<&'static [u8]> {
    let embedded = embedded?;
    if sha256_hex(embedded.bytes) == embedded.sha256 {
        Some(embedded.bytes)
    } else {
        log::error!("The embedded session-relay does not match its pinned sha256; not using it");
        None
    }
}

/// The relay for a container architecture as `uname -m` reports it (`x86_64`, `aarch64`).
pub fn relay_binary(machine: &str) -> Option<&'static [u8]> {
    match machine {
        "x86_64" | "amd64" => verified(X86_64.as_ref()),
        "aarch64" | "arm64" => verified(AARCH64.as_ref()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BYTES: &[u8] = b"relay";

    fn pinned(bytes: &'static [u8]) -> EmbeddedRelay {
        EmbeddedRelay {
            bytes,
            sha256: Box::leak(sha256_hex(bytes).into_boxed_str()),
        }
    }

    #[test]
    fn bytes_that_match_their_pinned_hash_are_used() {
        let embedded = pinned(BYTES);
        assert_eq!(verified(Some(&embedded)), Some(BYTES));
    }

    #[test]
    fn bytes_that_do_not_match_are_refused() {
        let embedded = EmbeddedRelay {
            bytes: BYTES,
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        };
        assert_eq!(verified(Some(&embedded)), None);
    }

    #[test]
    fn a_build_without_a_relay_has_none() {
        assert_eq!(verified(None), None);
        assert_eq!(relay_binary("riscv64"), None);
    }

    #[test]
    fn whatever_the_build_embedded_matches_its_pinned_hash() {
        for embedded in [X86_64.as_ref(), AARCH64.as_ref()].into_iter().flatten() {
            assert_eq!(verified(Some(embedded)), Some(embedded.bytes));
            assert!(!embedded.bytes.is_empty());
        }
    }

    #[test]
    fn the_hash_is_a_lowercase_sha256_hex_digest() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
