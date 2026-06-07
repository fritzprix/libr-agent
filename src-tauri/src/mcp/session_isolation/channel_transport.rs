use super::channel_events::{
    try_emit_channel_event, try_parse_channel_event, ChannelEventSender,
};
use futures::{SinkExt, StreamExt};
use rmcp::{
    service::{RoleClient, RxJsonRpcMessage, TxJsonRpcMessage},
    transport::Transport,
};
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;
use std::process::Stdio;
use std::sync::Arc;
use tokio::{
    process::{Child, ChildStdin, ChildStdout},
    sync::Mutex,
};
use tokio_util::{
    bytes::{Buf, BufMut, BytesMut},
    codec::{Decoder, Encoder, FramedRead, FramedWrite},
};

const MAX_WAIT_ON_DROP_SECS: u64 = 3;

type ClientStdioWriter = FramedWrite<ChildStdin, JsonRpcMessageCodec<TxJsonRpcMessage<RoleClient>>>;
type ClientStdioWriterHandle = Arc<Mutex<Option<ClientStdioWriter>>>;

struct ChildWithCleanup {
    inner: Option<Child>,
}

impl Drop for ChildWithCleanup {
    fn drop(&mut self) {
        if let Some(mut child) = self.inner.take() {
            tokio::spawn(async move {
                if let Err(error) = child.start_kill() {
                    log::warn!("Error killing channel-aware MCP child process: {}", error);
                }
            });
        }
    }
}

pub struct ChannelAwareStdioTransport {
    child: ChildWithCleanup,
    transport: ChannelAwareAsyncRwTransport,
}

struct ChannelAwareAsyncRwTransport {
    read: FramedRead<ChildStdout, ChannelInterceptCodec<RxJsonRpcMessage<RoleClient>>>,
    write: ClientStdioWriterHandle,
}

impl ChannelAwareStdioTransport {
    pub async fn graceful_shutdown(&mut self) -> std::io::Result<()> {
        if let Some(mut child) = self.child.inner.take() {
            self.transport.shutdown_io().await?;

            let wait_fut = child.wait();
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(MAX_WAIT_ON_DROP_SECS)) => {
                    if let Err(error) = child.start_kill() {
                        log::warn!("Error killing channel-aware MCP child: {error}");
                        return Err(error);
                    }
                },
                result = wait_fut => {
                    if let Err(error) = result {
                        log::warn!("Error waiting for channel-aware MCP child: {error}");
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn spawn_channel_aware_stdio(
    mut command: tokio::process::Command,
    server_name: String,
    event_tx: ChannelEventSender,
) -> std::io::Result<ChannelAwareStdioTransport> {
    command.stdin(Stdio::piped()).stdout(Stdio::piped());

    let mut child = command.spawn()?;
    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("stdin was already taken"))?;
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("stdout was already taken"))?;

    let transport =
        ChannelAwareAsyncRwTransport::new(child_stdout, child_stdin, server_name, event_tx);

    Ok(ChannelAwareStdioTransport {
        child: ChildWithCleanup { inner: Some(child) },
        transport,
    })
}

impl ChannelAwareAsyncRwTransport {
    fn new(
        read: ChildStdout,
        write: ChildStdin,
        server_name: String,
        event_tx: ChannelEventSender,
    ) -> Self {
        Self {
            read: FramedRead::new(read, ChannelInterceptCodec::new(server_name, event_tx)),
            write: Arc::new(Mutex::new(Some(FramedWrite::new(
                write,
                JsonRpcMessageCodec::<TxJsonRpcMessage<RoleClient>>::default(),
            )))),
        }
    }

    async fn shutdown_io(&mut self) -> std::io::Result<()> {
        let mut write = self.write.lock().await;
        drop(write.take());
        Ok(())
    }
}

impl Transport<RoleClient> for ChannelAwareStdioTransport {
    type Error = std::io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + 'static {
        self.transport.send(item)
    }

    fn receive(
        &mut self,
    ) -> impl std::future::Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        self.transport.receive()
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        self.graceful_shutdown().await
    }
}

impl Transport<RoleClient> for ChannelAwareAsyncRwTransport {
    type Error = std::io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send + 'static {
        let lock = self.write.clone();
        async move {
            let mut write = lock.lock().await;
            if let Some(ref mut write) = *write {
                write.send(item).await.map_err(Into::into)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "Transport is closed",
                ))
            }
        }
    }

    fn receive(
        &mut self,
    ) -> impl std::future::Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        let next = self.read.next();
        async move {
            next.await.and_then(|result| {
                result
                    .inspect_err(|error| {
                        log::error!("Error reading from channel-aware MCP transport: {}", error);
                    })
                    .ok()
            })
        }
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        self.shutdown_io().await
    }
}

#[derive(Debug, Clone)]
struct ChannelInterceptCodec<T> {
    server_name: String,
    event_tx: ChannelEventSender,
    next_index: usize,
    max_length: usize,
    is_discarding: bool,
    _marker: PhantomData<fn() -> T>,
}

impl<T> ChannelInterceptCodec<T> {
    fn new(server_name: String, event_tx: ChannelEventSender) -> Self {
        Self {
            server_name,
            event_tx,
            next_index: 0,
            max_length: usize::MAX,
            is_discarding: false,
            _marker: PhantomData,
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum ChannelInterceptCodecError {
    #[error("max line length exceeded")]
    MaxLineLengthExceeded,
    #[error("serde error {0}")]
    Serde(#[from] serde_json::Error),
    #[error("io error {0}")]
    Io(#[from] std::io::Error),
}

impl From<ChannelInterceptCodecError> for std::io::Error {
    fn from(value: ChannelInterceptCodecError) -> Self {
        match value {
            ChannelInterceptCodecError::MaxLineLengthExceeded => {
                std::io::Error::new(std::io::ErrorKind::InvalidData, value)
            }
            ChannelInterceptCodecError::Serde(error) => error.into(),
            ChannelInterceptCodecError::Io(error) => error,
        }
    }
}

fn without_carriage_return(bytes: &[u8]) -> &[u8] {
    if let Some(&b'\r') = bytes.last() {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    }
}

fn is_standard_method(method: &str) -> bool {
    matches!(
        method,
        "initialize"
            | "ping"
            | "prompts/get"
            | "prompts/list"
            | "resources/list"
            | "resources/read"
            | "resources/subscribe"
            | "resources/unsubscribe"
            | "resources/templates/list"
            | "tools/call"
            | "tools/list"
            | "completion/complete"
            | "logging/setLevel"
            | "roots/list"
            | "sampling/createMessage"
    ) || is_standard_notification(method)
}

fn is_standard_notification(method: &str) -> bool {
    matches!(
        method,
        "notifications/cancelled"
            | "notifications/initialized"
            | "notifications/message"
            | "notifications/progress"
            | "notifications/prompts/list_changed"
            | "notifications/resources/list_changed"
            | "notifications/resources/updated"
            | "notifications/roots/list_changed"
            | "notifications/tools/list_changed"
    )
}

fn should_ignore_notification(json_value: &serde_json::Value, method: &str) -> bool {
    let is_notification = json_value.get("id").is_none();

    if is_notification && !is_standard_method(method) {
        log::debug!("Ignoring unknown MCP notification method: {method}");
        return true;
    }

    if method.starts_with("notifications/") && !is_standard_notification(method) {
        log::debug!("Ignoring non-standard MCP notification method: {method}");
        return true;
    }

    false
}

fn try_parse_with_compatibility<T: DeserializeOwned>(
    line: &[u8],
) -> Result<Option<T>, ChannelInterceptCodecError> {
    if let Ok(line_str) = std::str::from_utf8(line) {
        match serde_json::from_slice::<T>(line) {
            Ok(item) => Ok(Some(item)),
            Err(error) => {
                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(line_str) {
                    if let Some(method) = json_value.get("method").and_then(|value| value.as_str())
                    {
                        if should_ignore_notification(&json_value, method) {
                            return Ok(None);
                        }
                    }
                }

                log::debug!(
                    "Failed to parse MCP message in channel-aware transport: {} | Error: {}",
                    line_str,
                    error
                );
                Err(ChannelInterceptCodecError::Serde(error))
            }
        }
    } else {
        serde_json::from_slice(line)
            .map(Some)
            .map_err(ChannelInterceptCodecError::Serde)
    }
}

impl<T: DeserializeOwned> Decoder for ChannelInterceptCodec<T> {
    type Item = T;
    type Error = ChannelInterceptCodecError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            let read_to = std::cmp::min(self.max_length.saturating_add(1), buf.len());
            let newline_offset = buf[self.next_index..read_to]
                .iter()
                .position(|byte| *byte == b'\n');

            match (self.is_discarding, newline_offset) {
                (true, Some(offset)) => {
                    buf.advance(offset + self.next_index + 1);
                    self.is_discarding = false;
                    self.next_index = 0;
                }
                (true, None) => {
                    buf.advance(read_to);
                    self.next_index = 0;
                    if buf.is_empty() {
                        return Ok(None);
                    }
                }
                (false, Some(offset)) => {
                    let newline_index = offset + self.next_index;
                    self.next_index = 0;
                    let line = buf.split_to(newline_index + 1);
                    let line = &line[..line.len() - 1];
                    let line = without_carriage_return(line);

                    if let Some(event) = try_parse_channel_event(line, &self.server_name) {
                        try_emit_channel_event(&self.event_tx, event, &self.server_name);
                        return Ok(None);
                    }

                    return try_parse_with_compatibility(line);
                }
                (false, None) if buf.len() > self.max_length => {
                    self.is_discarding = true;
                    return Err(ChannelInterceptCodecError::MaxLineLengthExceeded);
                }
                (false, None) => {
                    self.next_index = read_to;
                    return Ok(None);
                }
            }
        }
    }

    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Ok(match self.decode(buf)? {
            Some(frame) => Some(frame),
            None => {
                self.next_index = 0;
                if buf.is_empty() || buf == &b"\r"[..] {
                    None
                } else {
                    let line = buf.split_to(buf.len());
                    let line = without_carriage_return(&line);

                    if let Some(event) = try_parse_channel_event(line, &self.server_name) {
                        try_emit_channel_event(&self.event_tx, event, &self.server_name);
                        return Ok(None);
                    }

                    try_parse_with_compatibility(line)?
                }
            }
        })
    }
}

#[derive(Debug, Clone)]
struct JsonRpcMessageCodec<T> {
    _marker: PhantomData<fn() -> T>,
}

impl<T> Default for JsonRpcMessageCodec<T> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<T: Serialize> Encoder<T> for JsonRpcMessageCodec<T> {
    type Error = ChannelInterceptCodecError;

    fn encode(&mut self, item: T, buf: &mut BytesMut) -> Result<(), Self::Error> {
        serde_json::to_writer(buf.writer(), &item)?;
        buf.put_u8(b'\n');
        Ok(())
    }
}
