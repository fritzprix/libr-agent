use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::mcp::builtin::workspace::terminal_manager;

#[cfg(windows)]
use windows_sys::Win32::Globalization::{GetACP, MultiByteToWideChar};

#[cfg(windows)]
fn looks_like_utf16le(bytes: &[u8]) -> bool {
    if bytes.len() < 4 || !bytes.len().is_multiple_of(2) {
        return false;
    }

    // Heuristic: many NUL bytes in odd indices.
    let sample_len = bytes.len().min(200);
    let mut nul_count = 0;
    let mut checked = 0;

    for i in (1..sample_len).step_by(2) {
        checked += 1;
        if bytes[i] == 0 {
            nul_count += 1;
        }
    }

    checked > 0 && (nul_count * 4 >= checked * 3)
}

fn strip_ansi_escapes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }

        match chars.next() {
            // CSI: ESC [ ... <final byte>
            Some('[') => {
                for next in chars.by_ref() {
                    let code = next as u32;
                    if (0x40..=0x7E).contains(&code) {
                        break;
                    }
                }
            }
            // OSC: ESC ] ... BEL or ESC \
            Some(']') => {
                while let Some(next) = chars.next() {
                    if next == '\u{7}' {
                        break;
                    }

                    if next == '\u{1b}' {
                        if let Some('\\') = chars.peek().copied() {
                            let _ = chars.next();
                            break;
                        }
                    }
                }
            }
            // Other escapes: skip one char if present
            Some(_) | None => {}
        }
    }

    out
}

#[cfg(windows)]
fn decode_process_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    // UTF-8 BOM
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).to_string();
    }

    // UTF-16LE (common for some Windows tools)
    if looks_like_utf16le(bytes) {
        let start = if bytes.starts_with(&[0xFF, 0xFE]) {
            2
        } else {
            0
        };
        let u16_len = (bytes.len() - start) / 2;
        let mut wide = Vec::with_capacity(u16_len);
        for chunk in bytes[start..].chunks_exact(2) {
            wide.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        return String::from_utf16_lossy(&wide);
    }

    // UTF-8 fast path
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }

    // Fallback: decode using the system ANSI code page (e.g., CP949 on Korean Windows)
    let code_page = unsafe { GetACP() };
    let required = unsafe {
        MultiByteToWideChar(
            code_page,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        )
    };

    if required <= 0 {
        return String::from_utf8_lossy(bytes).to_string();
    }

    let mut wide: Vec<u16> = vec![0; required as usize];
    let converted = unsafe {
        MultiByteToWideChar(
            code_page,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide.as_mut_ptr(),
            required,
        )
    };

    if converted <= 0 {
        return String::from_utf8_lossy(bytes).to_string();
    }

    wide.truncate(converted as usize);
    String::from_utf16_lossy(&wide)
}

#[cfg(not(windows))]
fn decode_process_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

/// Spawn process and stream stdout/stderr to files (common logic for sync/async)
/// Returns (pid, exit_code, stdout_content, stderr_content)
/// Respects cancellation token for graceful shutdown
pub async fn spawn_and_stream_to_files(
    mut cmd: Command,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    process_label: String,
    cancel_token: CancellationToken,
) -> Result<(Option<u32>, Option<i32>, String, String), String> {
    // Configure stdio pipes - critical for capturing output
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null()); // Explicitly close stdin to prevent blocking

    // SECURITY: Clear environment variables to prevent leaking sensitive
    // host secrets (like API keys) to untrusted code executing in the process.
    cmd.env_clear();
    for (k, v) in crate::mcp::utils::env::get_isolated_env() {
        cmd.env(k, v);
    }

    // Windows-specific: Ensure handle inheritance is enabled for stdio pipes
    #[cfg(target_os = "windows")]
    {
        info!(
            "Spawning Windows process with stdio redirection: {}",
            process_label
        );
        // Note: tokio::process::Command automatically handles handle inheritance
        // on Windows when Stdio::piped() is used, but we log for diagnostics

        // Log the full command for debugging Windows execution issues
        info!("Windows process command debug: {:?}", cmd.as_std());

        // Also log important environment variables that affect Windows process
        // execution (PATH, SystemRoot, COMSPEC, PSModulePath, ProgramFiles).
        // We avoid logging full PATH contents to reduce noise; instead log
        // lengths and key variable presence.
        let path_len = std::env::var("PATH").map(|p| p.len()).unwrap_or(0);
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "<not-set>".to_string());
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "<not-set>".to_string());
        let psmodulepath =
            std::env::var("PSModulePath").unwrap_or_else(|_| "<not-set>".to_string());
        let program_files =
            std::env::var("ProgramFiles").unwrap_or_else(|_| "<not-set>".to_string());

        info!("Windows env: PATH.len={}, SystemRoot={}, COMSPEC={}, PSModulePath.present={}, ProgramFiles={}",
            path_len, system_root, comspec, !psmodulepath.is_empty(), program_files);
    }

    let mut child = cmd.spawn().map_err(|e| {
        error!(
            "Failed to spawn process {}: {}. Check command path and permissions.",
            process_label, e
        );
        format!("Failed to spawn process: {e}")
    })?;

    let pid = child.id();
    info!("Process {} started with PID {:?}", process_label, pid);
    // Determine maximum output size from configuration (default 100MB)
    let max_output_size = crate::config::max_output_size();

    // Stream stdout to file
    let stdout_handle = if let Some(mut stdout) = child.stdout.take() {
        let stdout_path_clone = stdout_path.clone();
        let label = process_label.clone();
        let cancel_clone = cancel_token.clone();
        Some(tokio::spawn(async move {
            if let Ok(file) = tokio::fs::File::create(&stdout_path_clone).await {
                let mut writer = tokio::io::BufWriter::new(file);
                info!(
                    "Process {} stdout streaming started to {:?}",
                    label, stdout_path_clone
                );
                let mut total_written = 0u64;
                let mut buffer = [0u8; 8192];

                loop {
                    tokio::select! {
                        _ = cancel_clone.cancelled() => {
                            info!("Process {} stdout streaming cancelled", label);
                            break;
                        }
                        result = stdout.read(&mut buffer) => {
                            match result {
                                Ok(0) => break,
                                Ok(n) => {
                                    total_written += n as u64;
                                    if total_written > max_output_size {
                                        warn!(
                                            "Process {} stdout size limit exceeded, truncating",
                                            label
                                        );
                                        let _ = writer
                                            .write_all(b"\n[Output truncated: size limit exceeded]\n")
                                            .await;
                                        break;
                                    }
                                    if writer.write_all(&buffer[..n]).await.is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }
                }

                // Explicit flush before drop
                let _ = writer.flush().await;
                drop(writer);

                info!(
                    "Process {} stdout streaming completed, total bytes: {}",
                    label, total_written
                );
            } else {
                error!(
                    "Process {} failed to create stdout file: {:?}",
                    label, stdout_path_clone
                );
            }
        }))
    } else {
        None
    };

    // Stream stderr to file
    let stderr_handle = if let Some(mut stderr) = child.stderr.take() {
        let stderr_path_clone = stderr_path.clone();
        let label = process_label.clone();
        let cancel_clone = cancel_token.clone();
        Some(tokio::spawn(async move {
            if let Ok(file) = tokio::fs::File::create(&stderr_path_clone).await {
                let mut writer = tokio::io::BufWriter::new(file);
                info!(
                    "Process {} stderr streaming started to {:?}",
                    label, stderr_path_clone
                );
                let mut total_written = 0u64;
                let mut buffer = [0u8; 8192];

                loop {
                    tokio::select! {
                        _ = cancel_clone.cancelled() => {
                            info!("Process {} stderr streaming cancelled", label);
                            break;
                        }
                        result = stderr.read(&mut buffer) => {
                            match result {
                                Ok(0) => break,
                                Ok(n) => {
                                    total_written += n as u64;
                                    if total_written > max_output_size {
                                        warn!(
                                            "Process {} stderr size limit exceeded, truncating",
                                            label
                                        );
                                        let _ = writer
                                            .write_all(b"\n[Output truncated: size limit exceeded]\n")
                                            .await;
                                        break;
                                    }
                                    if writer.write_all(&buffer[..n]).await.is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    }
                }

                // Explicit flush before drop
                let _ = writer.flush().await;
                drop(writer);

                info!(
                    "Process {} stderr streaming completed, total bytes: {}",
                    label, total_written
                );
            } else {
                error!(
                    "Process {} failed to create stderr file: {:?}",
                    label, stderr_path_clone
                );
            }
        }))
    } else {
        None
    };

    // Wait for process completion or cancellation
    let exit_code = tokio::select! {
        _ = cancel_token.cancelled() => {
            info!("Process {} cancellation requested, killing child", process_label);

            // Try graceful kill first
            let _ = child.kill().await;

            // Wait a bit for process to die (configurable graceful shutdown timeout)
            let graceful_secs = crate::config::graceful_shutdown_timeout();
            match tokio::time::timeout(Duration::from_secs(graceful_secs), child.wait()).await {
                Ok(Ok(status)) => status.code(),
                _ => {
                    warn!("Process {} did not terminate gracefully", process_label);
                    None
                }
            }
        }
        result = child.wait() => {
            match result {
                Ok(status) => status.code(),
                Err(e) => {
                    error!("Process {} wait error: {}", process_label, e);
                    None
                }
            }
        }
    };

    // Wait for streaming tasks to complete
    if let Some(h) = stdout_handle {
        let _ = h.await;
    }
    if let Some(h) = stderr_handle {
        let _ = h.await;
    }

    // Add small delay for Windows file system sync to ensure data is fully written
    #[cfg(windows)]
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Read output files
    let stdout_bytes = tokio::fs::read(&stdout_path).await.unwrap_or_default();
    let stdout_content = strip_ansi_escapes(&decode_process_output(&stdout_bytes));

    let stderr_bytes = tokio::fs::read(&stderr_path).await.unwrap_or_default();
    let stderr_content = strip_ansi_escapes(&decode_process_output(&stderr_bytes));

    info!(
        "Process {} completed with exit code {:?}",
        process_label, exit_code
    );

    // Log sizes of captured outputs for debugging intermittent missing output
    // on Windows; this helps determine whether the process produced no data
    // or whether it failed before output was written.
    info!(
        "Process {} output sizes: stdout={} bytes, stderr={} bytes",
        process_label,
        stdout_content.len(),
        stderr_content.len()
    );

    // If both stdout and stderr are empty but process exit code is non-zero,
    // log a warning and point back to the command label so operations team
    // can correlate with Windows process details.
    if stdout_content.is_empty() && stderr_content.is_empty() {
        if let Some(code) = exit_code {
            if code != 0 {
                warn!(
                    "Process {} returned non-zero exit code {} but both stdout and stderr are empty",
                    process_label, code
                );
            }
        }
    }

    Ok((pid, exit_code, stdout_content, stderr_content))
}

/// Spawn process and stream output to both in-memory buffers and files (for async/long-running processes)
/// Returns (pid, exit_code, streaming_handle)
/// Provides real-time access to output through broadcast channels and circular buffers
pub async fn spawn_and_stream_hybrid(
    mut cmd: Command,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    process_label: String,
    cancel_token: CancellationToken,
) -> Result<
    (
        Option<u32>,
        Option<i32>,
        Arc<terminal_manager::StreamingHandle>,
    ),
    String,
> {
    // Configure stdio pipes
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null());

    // SECURITY: Clear environment variables to prevent leaking sensitive
    // host secrets (like API keys) to untrusted code executing in the process.
    cmd.env_clear();
    for (k, v) in crate::mcp::utils::env::get_isolated_env() {
        cmd.env(k, v);
    }

    #[cfg(target_os = "windows")]
    {
        info!(
            "Spawning Windows process with hybrid streaming: {}",
            process_label
        );
    }

    let mut child = cmd.spawn().map_err(|e| {
        error!(
            "Failed to spawn process {}: {}. Check command path and permissions.",
            process_label, e
        );
        format!("Failed to spawn process: {e}")
    })?;

    let pid = child.id();
    info!(
        "Process {} started with PID {:?} (hybrid streaming)",
        process_label, pid
    );

    // Create streaming handle with 1000 line buffer
    let streaming = Arc::new(terminal_manager::StreamingHandle::new(1000));
    let max_output_size = crate::config::max_output_size();

    // Stdout streaming: line-by-line with broadcast and file
    let stdout_handle = if let Some(stdout) = child.stdout.take() {
        let streaming_clone = streaming.clone();
        let stdout_path_clone = stdout_path.clone();
        let label = process_label.clone();
        let cancel_clone = cancel_token.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            match tokio::fs::File::create(&stdout_path_clone).await {
                Ok(file) => {
                    let mut writer = tokio::io::BufWriter::new(file);
                    let mut total_bytes = 0u64;
                    let mut lines_since_flush = 0;
                    const FLUSH_INTERVAL: usize = 10;

                    loop {
                        tokio::select! {
                            _ = cancel_clone.cancelled() => {
                                info!("Process {} stdout streaming cancelled", label);
                                break;
                            }
                            line_result = lines.next_line() => {
                                match line_result {
                                    Ok(Some(line)) => {
                                        let cleaned_line = strip_ansi_escapes(&line);
                                        let line_bytes = cleaned_line.len() as u64 + 1; // +1 for newline
                                        total_bytes += line_bytes;

                                        if total_bytes > max_output_size {
                                            warn!("Process {} stdout size limit exceeded", label);
                                            let _ = writer.write_all(b"\n[Output truncated: size limit exceeded]\n").await;
                                            break;
                                        }

                                        // 1. Send to broadcast channel + buffer
                                        streaming_clone.push_stdout(cleaned_line.clone()).await;

                                        // 2. Write to file with periodic flush
                                        if writer.write_all(cleaned_line.as_bytes()).await.is_ok() {
                                            let _ = writer.write_all(b"\n").await;
                                            lines_since_flush += 1;
                                            if lines_since_flush >= FLUSH_INTERVAL {
                                                let _ = writer.flush().await;
                                                lines_since_flush = 0;
                                            }
                                        }
                                    }
                                    Ok(None) => break, // EOF
                                    Err(e) => {
                                        warn!("Process {} stdout read error: {}", label, e);
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    let _ = writer.flush().await;
                    drop(writer);
                    info!(
                        "Process {} stdout hybrid streaming completed ({} bytes)",
                        label, total_bytes
                    );
                }
                Err(e) => {
                    error!("Process {} failed to create stdout file: {}", label, e);
                }
            }
        })
    } else {
        tokio::spawn(async {})
    };

    // Stderr streaming: same pattern
    let stderr_handle = if let Some(stderr) = child.stderr.take() {
        let streaming_clone = streaming.clone();
        let stderr_path_clone = stderr_path.clone();
        let label = process_label.clone();
        let cancel_clone = cancel_token.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            match tokio::fs::File::create(&stderr_path_clone).await {
                Ok(file) => {
                    let mut writer = tokio::io::BufWriter::new(file);
                    let mut total_bytes = 0u64;
                    let mut lines_since_flush = 0;
                    const FLUSH_INTERVAL: usize = 10;

                    loop {
                        tokio::select! {
                            _ = cancel_clone.cancelled() => {
                                info!("Process {} stderr streaming cancelled", label);
                                break;
                            }
                            line_result = lines.next_line() => {
                                match line_result {
                                    Ok(Some(line)) => {
                                        let cleaned_line = strip_ansi_escapes(&line);
                                        let line_bytes = cleaned_line.len() as u64 + 1;
                                        total_bytes += line_bytes;

                                        if total_bytes > max_output_size {
                                            warn!("Process {} stderr size limit exceeded", label);
                                            let _ = writer.write_all(b"\n[Output truncated: size limit exceeded]\n").await;
                                            break;
                                        }

                                        // 1. Send to broadcast channel + buffer
                                        streaming_clone.push_stderr(cleaned_line.clone()).await;

                                        // 2. Write to file with periodic flush
                                        if writer.write_all(cleaned_line.as_bytes()).await.is_ok() {
                                            let _ = writer.write_all(b"\n").await;
                                            lines_since_flush += 1;
                                            if lines_since_flush >= FLUSH_INTERVAL {
                                                let _ = writer.flush().await;
                                                lines_since_flush = 0;
                                            }
                                        }
                                    }
                                    Ok(None) => break, // EOF
                                    Err(e) => {
                                        warn!("Process {} stderr read error: {}", label, e);
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    let _ = writer.flush().await;
                    drop(writer);
                    info!(
                        "Process {} stderr hybrid streaming completed ({} bytes)",
                        label, total_bytes
                    );
                }
                Err(e) => {
                    error!("Process {} failed to create stderr file: {}", label, e);
                }
            }
        })
    } else {
        tokio::spawn(async {})
    };

    // Wait for process completion or cancellation
    let exit_code = tokio::select! {
        _ = cancel_token.cancelled() => {
            info!("Process {} cancellation requested, killing child", process_label);
            let _ = child.kill().await;

            let graceful_secs = crate::config::graceful_shutdown_timeout();
            match tokio::time::timeout(Duration::from_secs(graceful_secs), child.wait()).await {
                Ok(Ok(status)) => status.code(),
                _ => {
                    warn!("Process {} did not terminate gracefully", process_label);
                    None
                }
            }
        }
        result = child.wait() => {
            match result {
                Ok(status) => status.code(),
                Err(e) => {
                    error!("Process {} wait error: {}", process_label, e);
                    None
                }
            }
        }
    };

    // Wait for streaming tasks to complete
    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    info!(
        "Process {} completed with exit code {:?} (hybrid streaming)",
        process_label, exit_code
    );

    Ok((pid, exit_code, streaming))
}
