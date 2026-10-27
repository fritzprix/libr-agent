//! Session API key (issue #2348), first slice: key format, hashing, the `Bearer` check and
//! capturing a key injected through the environment.
//!
//! Issuing, listing, revoking and approving keys (and any key scope) are not here yet; they
//! wait on owner decisions in #2348. Until a key is configured the TCP API behaves as before.

use sha2::{Digest, Sha256};
use std::sync::OnceLock;
use warp::http::{header, HeaderMap};
use warp::reject::Reject;
use warp::{Filter, Rejection};

/// Environment variable a launcher (for example the bench) uses to inject the key.
pub const ENV_VAR: &str = "LIBRAGENT_SESSION_API_KEY";
/// Prefix so secret scanners can recognise a leaked key.
pub const KEY_PREFIX: &str = "libr_";

const KEY_BYTES: usize = 32;
/// Base62 characters needed for 256 bits (62^43 > 2^256).
const KEY_CHARS: usize = 43;
const BASE62: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

type KeyHash = [u8; 32];

/// SHA-256 of the key captured from the environment at startup. Only the hash is kept.
static ENV_KEY_HASH: OnceLock<KeyHash> = OnceLock::new();

#[derive(Debug)]
pub struct Unauthorized;

impl Reject for Unauthorized {}

fn base62_encode_fixed(bytes: &[u8]) -> String {
    let mut digits = Vec::with_capacity(KEY_CHARS);
    let mut number = bytes.to_vec();
    while number.iter().any(|byte| *byte != 0) {
        let mut remainder = 0u32;
        for byte in number.iter_mut() {
            let current = (remainder << 8) | u32::from(*byte);
            *byte = (current / 62) as u8;
            remainder = current % 62;
        }
        digits.push(BASE62[remainder as usize]);
    }
    while digits.len() < KEY_CHARS {
        digits.push(BASE62[0]);
    }
    digits.reverse();
    String::from_utf8(digits).expect("base62 digits are ASCII")
}

/// `libr_` followed by 32 random bytes in base62 (43 characters).
pub fn generate_key() -> Result<String, getrandom::Error> {
    let mut bytes = [0u8; KEY_BYTES];
    getrandom::getrandom(&mut bytes)?;
    Ok(format!("{KEY_PREFIX}{}", base62_encode_fixed(&bytes)))
}

/// Keys are stored and compared by hash only.
pub fn hash_key(key: &str) -> KeyHash {
    Sha256::digest(key.as_bytes()).into()
}

fn hashes_equal(a: &KeyHash, b: &KeyHash) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Value of an RFC 6750 `Authorization: Bearer <token>` header.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// `Ok` when no key is configured, or when the request carries the configured key.
fn check_bearer(headers: &HeaderMap, expected: Option<&KeyHash>) -> Result<(), Unauthorized> {
    let Some(expected) = expected else {
        return Ok(());
    };
    match bearer_token(headers) {
        Some(token) if hashes_equal(&hash_key(token), expected) => Ok(()),
        _ => Err(Unauthorized),
    }
}

/// Reads the injected key, keeps only its hash and removes the variable from the process
/// environment so no child process (shells, MCP servers, `docker`) can inherit it.
///
/// Call it first in `main`, while the process is still single-threaded.
pub fn capture_env_key() {
    let value = std::env::var_os(ENV_VAR);
    std::env::remove_var(ENV_VAR);
    if let Some(hash) = value.and_then(|value| hash_from_env_value(&value.to_string_lossy())) {
        let _ = ENV_KEY_HASH.set(hash);
    }
}

fn hash_from_env_value(value: &str) -> Option<KeyHash> {
    let value = value.trim();
    (!value.is_empty()).then(|| hash_key(value))
}

fn guard(expected: Option<KeyHash>) -> impl Filter<Extract = (), Error = Rejection> + Clone {
    warp::header::headers_cloned()
        .and_then(move |headers: HeaderMap| async move {
            check_bearer(&headers, expected.as_ref()).map_err(warp::reject::custom)
        })
        .untuple_one()
}

/// Requires the configured key on every request (a no-op while no key is configured).
pub fn api_key_guard() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    guard(ENV_KEY_HASH.get().copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::access_guard::handle_rejection;
    use warp::http::HeaderValue;

    fn headers(authorization: Option<&str>) -> HeaderMap {
        let mut map = HeaderMap::new();
        if let Some(value) = authorization {
            map.insert(header::AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn generated_keys_have_the_documented_format() {
        let key = generate_key().unwrap();
        let body = key.strip_prefix(KEY_PREFIX).expect("prefix");
        assert_eq!(body.len(), KEY_CHARS);
        assert!(body.bytes().all(|byte| BASE62.contains(&byte)));
        assert_ne!(key, generate_key().unwrap());
    }

    #[test]
    fn base62_is_fixed_width_and_big_endian() {
        assert_eq!(base62_encode_fixed(&[0u8; 32]), "0".repeat(KEY_CHARS));
        let mut one = [0u8; 32];
        one[31] = 1;
        assert_eq!(
            base62_encode_fixed(&one),
            format!("{}1", "0".repeat(KEY_CHARS - 1))
        );
        let mut sixty_two = [0u8; 32];
        sixty_two[31] = 62;
        assert_eq!(
            base62_encode_fixed(&sixty_two),
            format!("{}10", "0".repeat(KEY_CHARS - 2))
        );
        let max = base62_encode_fixed(&[0xFF; 32]);
        assert_eq!(max.len(), KEY_CHARS);
        assert!(max.bytes().all(|byte| BASE62.contains(&byte)));
    }

    #[test]
    fn no_configured_key_allows_every_request() {
        assert!(check_bearer(&headers(None), None).is_ok());
        assert!(check_bearer(&headers(Some("Bearer anything")), None).is_ok());
    }

    #[test]
    fn configured_key_needs_the_matching_bearer_token() {
        let expected = hash_key("libr_secret");
        for ok in [
            "Bearer libr_secret",
            "bearer libr_secret",
            "BEARER libr_secret",
        ] {
            assert!(
                check_bearer(&headers(Some(ok)), Some(&expected)).is_ok(),
                "{ok}"
            );
        }
        for bad in [
            None,
            Some("Bearer wrong"),
            Some("Bearer"),
            Some("Bearer "),
            Some("Basic libr_secret"),
            Some("libr_secret"),
        ] {
            assert!(
                check_bearer(&headers(bad), Some(&expected)).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn env_value_is_hashed_and_blank_values_are_ignored() {
        assert_eq!(hash_from_env_value(" libr_a \n"), Some(hash_key("libr_a")));
        assert_eq!(hash_from_env_value("   "), None);
        assert_eq!(hash_from_env_value(""), None);
    }

    #[tokio::test]
    async fn guard_answers_401_with_www_authenticate() {
        let routes = guard(Some(hash_key("libr_secret")))
            .map(|| "ok")
            .recover(handle_rejection);

        let denied = warp::test::request().reply(&routes).await;
        assert_eq!(denied.status(), 401);
        assert_eq!(denied.headers().get("www-authenticate").unwrap(), "Bearer");

        let wrong = warp::test::request()
            .header("authorization", "Bearer nope")
            .reply(&routes)
            .await;
        assert_eq!(wrong.status(), 401);

        let allowed = warp::test::request()
            .header("authorization", "Bearer libr_secret")
            .reply(&routes)
            .await;
        assert_eq!(allowed.status(), 200);
    }

    #[tokio::test]
    async fn guard_is_a_no_op_without_a_configured_key() {
        let routes = guard(None).map(|| "ok").recover(handle_rejection);
        assert_eq!(warp::test::request().reply(&routes).await.status(), 200);
    }
}
