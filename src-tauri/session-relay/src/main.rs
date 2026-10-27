//! Runs inside a managed Docker session: accepts TCP on `127.0.0.1:<port>` and forwards every
//! connection to the session's unix socket (`/run/libragent/api.sock`), which the host serves
//! with the Session API. See `docs/architecture/session-wake-contract.md`.
//!
//! Usage: `session-relay <port> [socket-path]`
//!
//! Exit codes: 2 for bad arguments, 3 when the port cannot be bound (already in use), so the
//! host can tell "do not hand out a wake URL" from a crash, and 4 when accepting connections
//! fails for a reason that will not go away by itself (for example no file descriptors left).
//!
//! One thread runs an event loop for all connections: each connection is a task on a
//! current-thread runtime, not a thread.

#[cfg(unix)]
mod relay {
    use std::path::{Path, PathBuf};
    use tokio::io::copy_bidirectional;
    use tokio::net::{TcpListener, TcpStream, UnixStream};

    /// An accept error that belongs to one connection (the peer gave up before it was accepted).
    /// Anything else is not going to clear up by trying again.
    pub fn is_transient_accept_error(kind: std::io::ErrorKind) -> bool {
        use std::io::ErrorKind;
        matches!(
            kind,
            ErrorKind::ConnectionAborted | ErrorKind::ConnectionReset | ErrorKind::Interrupted
        )
    }

    pub const DEFAULT_SOCKET_PATH: &str = "/run/libragent/api.sock";

    /// Forwards bytes both ways until either side closes.
    async fn forward(mut client: TcpStream, socket_path: &Path) -> std::io::Result<()> {
        let mut upstream = UnixStream::connect(socket_path).await?;
        copy_bidirectional(&mut client, &mut upstream).await?;
        Ok(())
    }

    /// Accepts connections on `listener` until accepting fails for a reason that is not about a
    /// single connection, which is returned.
    pub async fn serve(listener: TcpListener, socket_path: PathBuf) -> std::io::Error {
        loop {
            let client = match listener.accept().await {
                Ok((client, _)) => client,
                Err(error) if is_transient_accept_error(error.kind()) => continue,
                Err(error) => return error,
            };
            let socket_path = socket_path.clone();
            tokio::spawn(async move {
                if let Err(error) = forward(client, &socket_path).await {
                    eprintln!("session-relay: {}: {error}", socket_path.display());
                }
            });
        }
    }
}

#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
async fn main() {
    use std::net::Ipv4Addr;
    use std::path::PathBuf;
    use tokio::net::TcpListener;

    let mut args = std::env::args().skip(1);
    let Some(port) = args.next().and_then(|value| value.parse::<u16>().ok()) else {
        eprintln!("usage: session-relay <port> [socket-path]");
        std::process::exit(2);
    };
    let socket_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(relay::DEFAULT_SOCKET_PATH));

    let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("session-relay: cannot listen on 127.0.0.1:{port}: {error}");
            std::process::exit(3);
        }
    };
    let error = relay::serve(listener, socket_path).await;
    eprintln!("session-relay: cannot accept connections: {error}");
    std::process::exit(4);
}

#[cfg(not(unix))]
fn main() {
    eprintln!("session-relay runs inside Linux containers only");
    std::process::exit(2);
}

#[cfg(all(test, unix))]
mod tests {
    use super::relay::is_transient_accept_error;
    use std::io::ErrorKind;

    #[test]
    fn only_errors_about_one_connection_are_retried() {
        for kind in [
            ErrorKind::ConnectionAborted,
            ErrorKind::ConnectionReset,
            ErrorKind::Interrupted,
        ] {
            assert!(is_transient_accept_error(kind), "{kind:?}");
        }
        // For example EMFILE (no file descriptors left) or a closed listener: retrying would
        // spin without waiting.
        for kind in [
            ErrorKind::Other,
            ErrorKind::OutOfMemory,
            ErrorKind::InvalidInput,
            ErrorKind::PermissionDenied,
            ErrorKind::NotConnected,
        ] {
            assert!(!is_transient_accept_error(kind), "{kind:?}");
        }
    }
}
