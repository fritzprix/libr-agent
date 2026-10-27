//! Runs inside a managed Docker session: accepts TCP on `127.0.0.1:<port>` and forwards every
//! connection to the session's unix socket (`/run/libragent/api.sock`), which the host serves
//! with the Session API. See `docs/architecture/session-wake-contract.md`.
//!
//! Usage: `session-relay <port> [socket-path]`
//!
//! Exit codes: 2 for bad arguments, 3 when the port cannot be bound (already in use), so the
//! host can tell "do not hand out a wake URL" from a crash.
//!
//! One thread runs an event loop for all connections: each connection is a task on a
//! current-thread runtime, not a thread.

#[cfg(unix)]
mod relay {
    use std::path::{Path, PathBuf};
    use tokio::io::copy_bidirectional;
    use tokio::net::{TcpListener, TcpStream, UnixStream};

    pub const DEFAULT_SOCKET_PATH: &str = "/run/libragent/api.sock";

    /// Forwards bytes both ways until either side closes.
    async fn forward(mut client: TcpStream, socket_path: &Path) -> std::io::Result<()> {
        let mut upstream = UnixStream::connect(socket_path).await?;
        copy_bidirectional(&mut client, &mut upstream).await?;
        Ok(())
    }

    /// Accepts connections on `listener` for as long as it lives.
    pub async fn serve(listener: TcpListener, socket_path: PathBuf) {
        loop {
            let Ok((client, _)) = listener.accept().await else {
                continue;
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
    relay::serve(listener, socket_path).await;
}

#[cfg(not(unix))]
fn main() {
    eprintln!("session-relay runs inside Linux containers only");
    std::process::exit(2);
}
