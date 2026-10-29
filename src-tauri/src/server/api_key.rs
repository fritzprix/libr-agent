//! Session API key (issue #2348), first slice: key format, hashing, the `Bearer` check and
//! capturing a key injected through the environment.
//!
//! Issuing, listing, revoking and approving keys (and any key scope) are not here yet; they
//! wait on owner decisions in #2348. Until a key is injected the TCP API behaves as before.
//! Once one is injected, anything wrong with it fails closed: every request is rejected.

use sha2::{Digest, Sha256};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use warp::http::{header, HeaderMap};
use warp::reject::Reject;
use warp::{Filter, Rejection};

/// Environment variable a launcher (for example the bench) uses to inject the key.
pub const ENV_VAR: &str = "LIBRAGENT_SESSION_API_KEY";
/// Alternative to [`ENV_VAR`]: path of a private file holding the key. The file is read and
/// deleted at startup. Unlike the environment block, a file leaves no copy of the key in
/// `/proc/<pid>/environ` for same-user processes to read.
pub const ENV_FILE_VAR: &str = "LIBRAGENT_SESSION_API_KEY_FILE";
/// Prefix so secret scanners can recognise a leaked key.
pub const KEY_PREFIX: &str = "libr_";

const KEY_BYTES: usize = 32;
/// Base62 characters needed for 256 bits (62^43 > 2^256).
const KEY_CHARS: usize = 43;
const BASE62: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

pub(crate) type KeyHash = [u8; 32];

/// What startup found about an injected key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyState {
    /// SHA-256 of the key. Only the hash is kept.
    Key(KeyHash),
    /// A key was injected but cannot be used (blank, unreadable, exposed): reject everything
    /// instead of silently serving the API without authentication.
    DenyAll,
}

static ENV_KEY_STATE: OnceLock<KeyState> = OnceLock::new();

/// Rejection for a missing or wrong key.
#[derive(Debug)]
pub struct Unauthorized {
    /// A token was sent but is not accepted (RFC 6750 `invalid_token`).
    invalid_token: bool,
}

impl Unauthorized {
    /// Value of the `WWW-Authenticate` response header.
    pub fn www_authenticate(&self) -> &'static str {
        if self.invalid_token {
            "Bearer error=\"invalid_token\""
        } else {
            "Bearer"
        }
    }
}

impl Reject for Unauthorized {}

/// Rejection while no key is injected but a Docker session exists: a container can reach
/// host loopback on some platforms (Docker Desktop), so loopback alone is not enough.
#[derive(Debug)]
pub struct KeyRequired;

impl Reject for KeyRequired {}

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

pub(crate) fn hashes_equal(a: &KeyHash, b: &KeyHash) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn looks_like_generated_key(key: &str) -> bool {
    key.strip_prefix(KEY_PREFIX)
        .is_some_and(|body| body.len() == KEY_CHARS && body.bytes().all(|b| BASE62.contains(&b)))
}

/// Value of an RFC 6750 `Authorization: Bearer <token>` header.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// `Ok` when no key is configured anywhere, or when the request carries an accepted key
/// (either injected via environment or issued via agent pairing).
fn check_bearer(headers: &HeaderMap, expected_env: Option<KeyState>) -> Result<(), Unauthorized> {
    let token = bearer_token(headers);
    let invalid_token = token.is_some();

    // If an environment key is in DenyAll mode, fail closed immediately.
    if matches!(expected_env, Some(KeyState::DenyAll)) {
        return Err(Unauthorized { invalid_token });
    }

    let any_key_configured = expected_env.is_some() || super::pairing::has_any_active_issued_key();
    if !any_key_configured {
        return Ok(());
    }

    if let Some(token) = token {
        // 1. Check against environment injected key
        if let Some(KeyState::Key(expected_hash)) = expected_env {
            if hashes_equal(&hash_key(token), &expected_hash) {
                return Ok(());
            }
        }
        // 2. Check against issued pairing keys
        if super::pairing::is_token_accepted_by_issued_keys(token) {
            return Ok(());
        }

        // Token was sent, but matched neither
        return Err(Unauthorized {
            invalid_token: true,
        });
    }

    // No token provided, but at least one key is configured: reject.
    Err(Unauthorized {
        invalid_token: false,
    })
}

/// Reads and deletes the key file. A file that group or others can access is refused (and
/// still deleted): the key in it must be treated as exposed.
fn read_and_delete_key_file(path: &Path) -> io::Result<String> {
    let result = read_private_file(path);
    let _ = std::fs::remove_file(path);
    result
}

fn read_private_file(path: &Path) -> io::Result<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(path)?.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "key file must not be accessible by group or others (use mode 0600)",
            ));
        }
    }
    std::fs::read_to_string(path)
}

/// Decides the key state from what the environment held. The second value is a message for
/// the operator, if anything about the key needs attention.
fn resolve_env_key(
    key: Option<&str>,
    file_contents: Option<io::Result<String>>,
) -> Option<(KeyState, Option<String>)> {
    let (source, value) = match (file_contents, key) {
        (Some(Err(error)), _) => {
            return Some((
                KeyState::DenyAll,
                Some(format!("{ENV_FILE_VAR} could not be used ({error})")),
            ));
        }
        (Some(Ok(contents)), _) => (ENV_FILE_VAR, contents),
        (None, Some(key)) => (ENV_VAR, key.to_string()),
        (None, None) => return None,
    };
    let value = value.trim();
    if value.is_empty() {
        return Some((
            KeyState::DenyAll,
            Some(format!("{source} is set but empty")),
        ));
    }
    let note = (!looks_like_generated_key(value)).then(|| {
        format!(
            "the key from {source} is not a generated key ({KEY_PREFIX} + {KEY_CHARS} characters)"
        )
    });
    Some((KeyState::Key(hash_key(value)), note))
}

/// Reads the injected key, keeps only its hash and removes both variables from the process
/// environment so no child process (shells, MCP servers, `docker`) inherits them.
///
/// Call it first in `main`, while the process is still single-threaded. Removing a variable
/// stops inheritance; it does not stop same-user processes reading the original environment
/// block (`/proc/<pid>/environ` on Linux), which is why [`ENV_FILE_VAR`] exists.
pub fn capture_env_key() {
    let key = std::env::var_os(ENV_VAR).map(|value| value.to_string_lossy().into_owned());
    let file = std::env::var_os(ENV_FILE_VAR).map(PathBuf::from);
    std::env::remove_var(ENV_VAR);
    std::env::remove_var(ENV_FILE_VAR);

    let file_contents = file.map(|path| read_and_delete_key_file(&path));
    let Some((state, note)) = resolve_env_key(key.as_deref(), file_contents) else {
        return;
    };
    match (&state, note) {
        (KeyState::DenyAll, Some(note)) => {
            eprintln!("Session API key: {note}; every Session API request will be rejected");
        }
        (_, Some(note)) => eprintln!("Session API key: warning: {note}"),
        _ => {}
    }
    let _ = ENV_KEY_STATE.set(state);
}

/// True when a key was injected (usable or not). The key itself is not kept, so a process
/// restart cannot carry it over.
pub fn is_configured() -> bool {
    ENV_KEY_STATE.get().is_some()
}

/// `docker_sessions` reports whether a Docker session (managed or attached) exists; it is
/// asked only while no key is injected.
fn guard<F, Fut>(
    expected: Option<KeyState>,
    docker_sessions: F,
) -> impl Filter<Extract = (), Error = Rejection> + Clone
where
    F: Fn() -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = bool> + Send,
{
    warp::header::headers_cloned()
        .and_then(move |headers: HeaderMap| {
            let docker_sessions = docker_sessions.clone();
            async move {
                let has_any_key = expected.is_some() || super::pairing::has_any_active_issued_key();
                if !has_any_key && docker_sessions().await {
                    return Err(warp::reject::custom(KeyRequired));
                }
                check_bearer(&headers, expected).map_err(warp::reject::custom)
            }
        })
        .untuple_one()
}

/// Whether any Docker session (managed or attached) exists; a failed read counts as yes (fail closed).
async fn docker_sessions_exist() -> bool {
    use crate::repositories::session_repository::SessionRepository;

    match crate::state::try_get_session_repository() {
        Some(repo) => repo.has_docker_sessions().await.unwrap_or(true),
        None => false,
    }
}

/// Requires the injected key on every request. Without one, requests pass unless a Docker
/// session (managed or attached) exists; then they are refused. The key is never given to
/// containers.
pub fn api_key_guard() -> impl Filter<Extract = (), Error = Rejection> + Clone {
    guard(ENV_KEY_STATE.get().copied(), docker_sessions_exist)
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

    fn key_state(key: &str) -> Option<KeyState> {
        Some(KeyState::Key(hash_key(key)))
    }

    #[test]
    fn generated_keys_have_the_documented_format() {
        let key = generate_key().unwrap();
        let body = key.strip_prefix(KEY_PREFIX).expect("prefix");
        assert_eq!(body.len(), KEY_CHARS);
        assert!(body.bytes().all(|byte| BASE62.contains(&byte)));
        assert!(looks_like_generated_key(&key));
        assert_ne!(key, generate_key().unwrap());
        assert!(!looks_like_generated_key("libr_short"));
        assert!(!looks_like_generated_key("secret"));
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
    fn no_injected_key_allows_every_request() {
        assert!(check_bearer(&headers(None), None).is_ok());
        assert!(check_bearer(&headers(Some("Bearer anything")), None).is_ok());
    }

    #[test]
    fn injected_key_needs_the_matching_bearer_token() {
        let expected = key_state("libr_secret");
        for ok in [
            "Bearer libr_secret",
            "bearer libr_secret",
            "BEARER libr_secret",
        ] {
            assert!(check_bearer(&headers(Some(ok)), expected).is_ok(), "{ok}");
        }
        for bad in [
            None,
            Some("Bearer wrong"),
            Some("Bearer"),
            Some("Bearer "),
            Some("Basic libr_secret"),
            Some("libr_secret"),
        ] {
            assert!(check_bearer(&headers(bad), expected).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn deny_all_rejects_even_a_well_formed_token() {
        for authorization in [None, Some("Bearer libr_secret"), Some("Bearer ")] {
            assert!(
                check_bearer(&headers(authorization), Some(KeyState::DenyAll)).is_err(),
                "{authorization:?}"
            );
        }
    }

    #[test]
    fn challenge_names_invalid_token_only_when_a_token_was_sent() {
        let expected = key_state("libr_secret");
        let missing = check_bearer(&headers(None), expected).unwrap_err();
        assert_eq!(missing.www_authenticate(), "Bearer");
        let wrong = check_bearer(&headers(Some("Bearer nope")), expected).unwrap_err();
        assert_eq!(wrong.www_authenticate(), "Bearer error=\"invalid_token\"");
    }

    #[test]
    fn unusable_injected_values_fail_closed() {
        for (key, file) in [
            (Some(""), None),
            (Some("  \n"), None),
            (None, Some(Ok(String::new()))),
            (None, Some(Ok("\n".to_string()))),
            (
                Some("libr_valid"),
                Some(Err(io::Error::from(io::ErrorKind::NotFound))),
            ),
        ] {
            let (state, note) = resolve_env_key(key, file).expect("a key was injected");
            assert_eq!(state, KeyState::DenyAll, "{key:?}");
            assert!(note.is_some(), "{key:?}");
        }
    }

    #[test]
    fn usable_injected_values_are_hashed_and_nothing_injected_means_no_state() {
        assert!(resolve_env_key(None, None).is_none());

        let generated = generate_key().unwrap();
        let (state, note) = resolve_env_key(Some(&format!(" {generated}\n")), None).unwrap();
        assert_eq!(state, KeyState::Key(hash_key(&generated)));
        assert!(note.is_none());

        // The file wins over the variable, and a non-generated key only warns.
        let (state, note) =
            resolve_env_key(Some("from_env"), Some(Ok("from_file".to_string()))).unwrap();
        assert_eq!(state, KeyState::Key(hash_key("from_file")));
        assert!(note.unwrap().contains(ENV_FILE_VAR));
    }

    #[test]
    fn key_file_is_read_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session-api-key");
        std::fs::write(&path, "libr_from_file\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert_eq!(
            read_and_delete_key_file(&path).unwrap().trim(),
            "libr_from_file"
        );
        assert!(!path.exists());
        assert!(read_and_delete_key_file(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn key_file_open_to_group_or_others_is_refused_and_deleted() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session-api-key");
        std::fs::write(&path, "libr_exposed").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let error = read_and_delete_key_file(&path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn guard_answers_401_with_www_authenticate() {
        let routes = guard(key_state("libr_secret"), docker_sessions)
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
        assert_eq!(
            wrong.headers().get("www-authenticate").unwrap(),
            "Bearer error=\"invalid_token\""
        );

        let allowed = warp::test::request()
            .header("authorization", "Bearer libr_secret")
            .reply(&routes)
            .await;
        assert_eq!(allowed.status(), 200);
    }

    #[tokio::test]
    async fn deny_all_guard_rejects_through_the_real_handler() {
        let routes = guard(Some(KeyState::DenyAll), docker_sessions)
            .map(|| "ok")
            .recover(handle_rejection);
        let reply = warp::test::request()
            .header("authorization", "Bearer libr_secret")
            .reply(&routes)
            .await;
        assert_eq!(reply.status(), 401);
    }

    async fn docker_sessions() -> bool {
        true
    }

    async fn no_docker_sessions() -> bool {
        false
    }

    #[tokio::test]
    async fn guard_is_a_no_op_without_an_injected_key() {
        let routes = guard(None, no_docker_sessions)
            .map(|| "ok")
            .recover(handle_rejection);
        assert_eq!(warp::test::request().reply(&routes).await.status(), 200);
    }

    #[tokio::test]
    async fn without_a_key_docker_sessions_sessions_close_the_api() {
        let routes = guard(None, docker_sessions)
            .map(|| "ok")
            .recover(handle_rejection);
        for request in [
            warp::test::request(),
            // A container can spoof Host and send any token; neither helps.
            warp::test::request().header("host", "127.0.0.1:3030"),
            warp::test::request().header("authorization", "Bearer libr_anything"),
        ] {
            let reply = request.reply(&routes).await;
            assert_eq!(reply.status(), 401);
            assert!(String::from_utf8_lossy(reply.body()).contains("Docker sessions exist"));
        }
    }

    #[tokio::test]
    async fn with_a_key_docker_sessions_sessions_change_nothing() {
        let routes = guard(key_state("libr_secret"), docker_sessions)
            .map(|| "ok")
            .recover(handle_rejection);
        let ok = warp::test::request()
            .header("authorization", "Bearer libr_secret")
            .reply(&routes)
            .await;
        assert_eq!(ok.status(), 200);
        assert_eq!(warp::test::request().reply(&routes).await.status(), 401);
    }

    #[tokio::test]
    async fn guard_with_issued_key_and_docker_sessions_permits_valid_and_rejects_revoked() {
        use crate::server::pairing;

        let temp_dir = tempfile::tempdir().unwrap();
        let _guard = pairing::test_env_data_dir(temp_dir.path());

        let raw_key = generate_key().unwrap();
        let hash = pairing::hash_to_hex(&hash_key(&raw_key));
        let issued = pairing::IssuedApiKey {
            id: "key-1".to_string(),
            name: "test-agent".to_string(),
            key_hash: hash,
            created_at: chrono::Utc::now().to_rfc3339(),
            revoked_at: None,
        };
        pairing::save_issued_keys_all(&[issued]).unwrap();

        // No env key (None), but docker_sessions returns true
        let routes = guard(None, docker_sessions)
            .map(|| "ok")
            .recover(handle_rejection);

        // 1. Valid issued key with docker sessions -> 200 OK
        let ok = warp::test::request()
            .header("authorization", format!("Bearer {raw_key}"))
            .reply(&routes)
            .await;
        assert_eq!(ok.status(), 200);

        // 2. Missing authorization with docker sessions -> 401 Unauthorized
        let missing = warp::test::request().reply(&routes).await;
        assert_eq!(missing.status(), 401);

        // 3. Revoke key -> no active keys left -> returns 401 KeyRequired
        pairing::revoke_issued_key("key-1").unwrap();
        let revoked = warp::test::request()
            .header("authorization", format!("Bearer {raw_key}"))
            .reply(&routes)
            .await;
        assert_eq!(revoked.status(), 401);
        assert!(String::from_utf8_lossy(revoked.body()).contains("Docker sessions exist"));
    }
}
