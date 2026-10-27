//! Embeds the `session-relay` binaries built by `.github/workflows/session-relay.yml`.
//!
//! `LIBRAGENT_RELAY_DIR` points at a directory holding `session-relay-<arch>` and its
//! `session-relay-<arch>.sha256` for each architecture. A binary whose hash does not match its
//! `.sha256` fails the build; one that is not there is simply not embedded, and the app then has
//! no relay (Docker sessions get no wake URL). The binaries are never committed.

use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

const ARCHES: [(&str, &str); 2] = [("X86_64", "x86_64"), ("AARCH64", "aarch64")];

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// The hash in a `sha256sum` line (`<hash>  <file name>`).
fn expected_hash(sha_file: &str) -> Option<String> {
    let hash = sha_file.split_whitespace().next()?.to_ascii_lowercase();
    (hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(hash)
}

/// Writes `session_relay_embed.rs` into `out_dir`, defining `X86_64` and `AARCH64` as
/// `Option<EmbeddedRelay>`.
pub fn write_embed(out_dir: &Path, relay_dir: Option<&Path>) -> io::Result<()> {
    let mut source = String::new();
    for (constant, arch) in ARCHES {
        let embedded = match relay_dir {
            Some(dir) => embed_arch(out_dir, dir, arch)?,
            None => None,
        };
        match embedded {
            Some((copied, hash)) => {
                let _ = writeln!(
                    source,
                    "pub static {constant}: Option<EmbeddedRelay> = Some(EmbeddedRelay {{ bytes: include_bytes!({:?}), sha256: {hash:?} }});",
                    copied.to_string_lossy()
                );
            }
            None => {
                let _ = writeln!(
                    source,
                    "pub static {constant}: Option<EmbeddedRelay> = None;"
                );
            }
        }
    }
    fs::write(out_dir.join("session_relay_embed.rs"), source)
}

fn embed_arch(
    out_dir: &Path,
    relay_dir: &Path,
    arch: &str,
) -> io::Result<Option<(std::path::PathBuf, String)>> {
    let binary_path = relay_dir.join(format!("session-relay-{arch}"));
    let sha_path = relay_dir.join(format!("session-relay-{arch}.sha256"));
    println!("cargo:rerun-if-changed={}", binary_path.display());
    println!("cargo:rerun-if-changed={}", sha_path.display());

    let (Ok(bytes), Ok(sha_file)) = (fs::read(&binary_path), fs::read_to_string(&sha_path)) else {
        println!(
            "cargo:warning=session-relay-{arch} (or its .sha256) not found in LIBRAGENT_RELAY_DIR; building without it"
        );
        return Ok(None);
    };
    let Some(expected) = expected_hash(&sha_file) else {
        return Err(io::Error::other(format!(
            "{} is not a sha256sum line",
            sha_path.display()
        )));
    };
    let actual = sha256_hex(&bytes);
    if actual != expected {
        return Err(io::Error::other(format!(
            "{} does not match its .sha256 (expected {expected}, got {actual})",
            binary_path.display()
        )));
    }
    let copied = out_dir.join(format!("session-relay-{arch}"));
    fs::write(&copied, &bytes)?;
    Ok(Some((copied, actual)))
}
