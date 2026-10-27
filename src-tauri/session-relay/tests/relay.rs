//! Runs the real binary: the host reads its exit codes (3 = port in use, so no wake URL; 2 = bad
//! arguments) and relies on it forwarding to the unix socket.
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const RELAY: &str = env!("CARGO_BIN_EXE_session-relay");

fn temp_socket(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("session-relay-{name}-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
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

struct Relay(Child);

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_relay(port: u16, socket: &PathBuf) -> Relay {
    let child = Command::new(RELAY)
        .arg(port.to_string())
        .arg(socket)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let relay = Relay(child);
    let deadline = Instant::now() + Duration::from_secs(10);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "relay did not start listening");
        thread::sleep(Duration::from_millis(20));
    }
    relay
}

fn round_trip(port: u16, request: &str) -> String {
    let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
    client.write_all(request.as_bytes()).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn a_port_that_is_in_use_exits_with_code_3() {
    let taken = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = taken.local_addr().unwrap().port();

    let status = Command::new(RELAY)
        .arg(port.to_string())
        .stderr(Stdio::null())
        .status()
        .unwrap();

    assert_eq!(status.code(), Some(3));
}

#[test]
fn bad_arguments_exit_with_code_2() {
    for args in [vec![], vec!["not-a-port"], vec!["70000"]] {
        let status = Command::new(RELAY)
            .args(&args)
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(2), "{args:?}");
    }
}

#[test]
fn forwards_requests_to_the_unix_socket() {
    let socket = temp_socket("forward");
    spawn_echo_server(&socket);
    let port = free_port();
    let _relay = start_relay(port, &socket);

    assert_eq!(round_trip(port, "hello"), "echo:hello");
    let _ = std::fs::remove_file(socket);
}

#[test]
fn serves_many_connections_at_once() {
    let socket = temp_socket("parallel");
    spawn_echo_server(&socket);
    let port = free_port();
    let _relay = start_relay(port, &socket);

    let clients: Vec<_> = (0..32)
        .map(|index| thread::spawn(move || (index, round_trip(port, &format!("req{index}")))))
        .collect();
    for client in clients {
        let (index, response) = client.join().unwrap();
        assert_eq!(response, format!("echo:req{index}"));
    }
    let _ = std::fs::remove_file(socket);
}

#[test]
fn a_missing_socket_closes_the_connection_and_the_relay_keeps_running() {
    let socket = temp_socket("missing");
    let port = free_port();
    let _relay = start_relay(port, &socket);

    // The relay closes the connection: an empty answer or a reset, never a reply.
    let mut client = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let _ = client.write_all(b"lost");
    let mut response = String::new();
    let _ = client.read_to_string(&mut response);
    assert_eq!(response, "");

    spawn_echo_server(&socket);
    assert_eq!(round_trip(port, "again"), "echo:again");
    let _ = std::fs::remove_file(socket);
}

#[test]
fn the_relay_serves_every_connection_from_one_thread() {
    let socket = temp_socket("threads");
    spawn_echo_server(&socket);
    let port = free_port();
    let relay = start_relay(port, &socket);

    // Hold many connections open at the same time, then count the relay's threads.
    let held: Vec<TcpStream> = (0..64)
        .map(|_| TcpStream::connect(("127.0.0.1", port)).unwrap())
        .collect();
    thread::sleep(Duration::from_millis(200));
    let tasks = std::fs::read_dir(format!("/proc/{}/task", relay.0.id()));
    if let Ok(tasks) = tasks {
        assert_eq!(
            tasks.count(),
            1,
            "the relay must not spawn threads per connection"
        );
    }
    drop(held);
    let _ = std::fs::remove_file(socket);
}
