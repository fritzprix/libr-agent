//! Runs inside a managed Docker session: accepts TCP on `127.0.0.1:<port>` and forwards every
//! connection to the session's unix socket (`/run/libragent/api.sock`), which the host serves
//! with the Session API. See `docs/architecture/session-wake-contract.md`.
//!
//! Usage: `session-relay <port> [socket-path]`
//!
//! Exit codes: 2 for bad arguments, 3 when the port cannot be bound (already in use), so the
//! host can tell "do not hand out a wake URL" from a crash.

#[cfg(unix)]
mod relay {
    use std::io::{self, Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};
    use std::os::unix::net::UnixStream;
    use std::path::{Path, PathBuf};
    use std::thread;

    pub const DEFAULT_SOCKET_PATH: &str = "/run/libragent/api.sock";

    /// Forwards bytes both ways until either side closes.
    fn forward(client: TcpStream, socket_path: &Path) -> io::Result<()> {
        let upstream = UnixStream::connect(socket_path)?;
        let mut client_reader = client.try_clone()?;
        let mut upstream_writer = upstream.try_clone()?;

        let to_upstream = thread::spawn(move || {
            let _ = io::copy(&mut client_reader, &mut upstream_writer);
            let _ = upstream_writer.shutdown(Shutdown::Write);
        });

        let mut upstream_reader = upstream;
        let mut client_writer = client;
        let mut buffer = [0u8; 16 * 1024];
        loop {
            match upstream_reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    if client_writer.write_all(&buffer[..read]).is_err() {
                        break;
                    }
                }
            }
        }
        let _ = client_writer.shutdown(Shutdown::Write);
        let _ = to_upstream.join();
        Ok(())
    }

    /// Accepts connections on `listener` for as long as it lives.
    pub fn serve(listener: TcpListener, socket_path: PathBuf) {
        for incoming in listener.incoming() {
            let Ok(client) = incoming else { continue };
            let socket_path = socket_path.clone();
            thread::spawn(move || {
                if let Err(error) = forward(client, &socket_path) {
                    eprintln!("session-relay: {}: {error}", socket_path.display());
                }
            });
        }
    }
}

#[cfg(unix)]
fn main() {
    use std::net::{Ipv4Addr, TcpListener};
    use std::path::PathBuf;

    let mut args = std::env::args().skip(1);
    let Some(port) = args.next().and_then(|value| value.parse::<u16>().ok()) else {
        eprintln!("usage: session-relay <port> [socket-path]");
        std::process::exit(2);
    };
    let socket_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(relay::DEFAULT_SOCKET_PATH));

    let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("session-relay: cannot listen on 127.0.0.1:{port}: {error}");
            std::process::exit(3);
        }
    };
    relay::serve(listener, socket_path);
}

#[cfg(not(unix))]
fn main() {
    eprintln!("session-relay runs inside Linux containers only");
    std::process::exit(2);
}

#[cfg(all(test, unix))]
mod tests {
    use super::relay;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::thread;

    fn temp_socket(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("session-relay-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    /// A unix server that answers each connection with `echo:` plus what it read until EOF.
    fn spawn_echo_server(path: &PathBuf) {
        let listener = UnixListener::bind(path).unwrap();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                thread::spawn(move || {
                    let mut request = Vec::new();
                    stream.read_to_end(&mut request).unwrap();
                    let _ = stream.write_all(b"echo:");
                    let _ = stream.write_all(&request);
                });
            }
        });
    }

    fn start_relay(socket_path: PathBuf) -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || relay::serve(listener, socket_path));
        port
    }

    #[test]
    fn forwards_a_request_and_its_response() {
        let socket = temp_socket("forward");
        spawn_echo_server(&socket);
        let port = start_relay(socket.clone());

        let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
        client.write_all(b"hello").unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();

        assert_eq!(response, "echo:hello");
        let _ = std::fs::remove_file(socket);
    }

    #[test]
    fn serves_several_connections_at_once() {
        let socket = temp_socket("parallel");
        spawn_echo_server(&socket);
        let port = start_relay(socket.clone());

        let clients: Vec<_> = (0..8)
            .map(|index| {
                thread::spawn(move || {
                    let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
                    client.write_all(format!("req{index}").as_bytes()).unwrap();
                    client.shutdown(std::net::Shutdown::Write).unwrap();
                    let mut response = String::new();
                    client.read_to_string(&mut response).unwrap();
                    (index, response)
                })
            })
            .collect();
        for client in clients {
            let (index, response) = client.join().unwrap();
            assert_eq!(response, format!("echo:req{index}"));
        }
        let _ = std::fs::remove_file(socket);
    }

    #[test]
    fn a_missing_socket_closes_the_connection_without_killing_the_relay() {
        let socket = temp_socket("missing");
        let port = start_relay(socket.clone());

        let mut first = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut response = String::new();
        first.read_to_string(&mut response).unwrap();
        assert!(response.is_empty());

        // The relay keeps serving once the socket exists.
        spawn_echo_server(&socket);
        let mut second = TcpStream::connect(("127.0.0.1", port)).unwrap();
        second.write_all(b"again").unwrap();
        second.shutdown(std::net::Shutdown::Write).unwrap();
        let mut response = String::new();
        second.read_to_string(&mut response).unwrap();
        assert_eq!(response, "echo:again");
        let _ = std::fs::remove_file(socket);
    }
}
