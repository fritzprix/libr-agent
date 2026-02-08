use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

/// Process status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessStatus {
    Starting, // Spawning in progress
    Running,  // Actively running
    Finished, // Completed successfully
    Failed,   // Exited with error
    Killed,   // Terminated by user/system
}

/// Process metadata entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEntry {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub session_id: String,
    pub command: String,
    pub status: ProcessStatus,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub stdout_path: String,
    pub stderr_path: String,
    pub stdout_size: u64,
    pub stderr_size: u64,

    // Poll tracking fields for detecting excessive polling
    #[serde(default)]
    pub last_poll_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub poll_count: u32,
    #[serde(default)]
    pub consecutive_running_polls: u32,
    #[serde(default)]
    pub first_running_poll_at: Option<DateTime<Utc>>,
}

/// Stream type for identifying stdout or stderr
#[derive(Debug, Clone, Copy)]
pub enum StreamType {
    Stdout,
    Stderr,
}

/// Real-time streaming handle for background processes
/// Provides broadcast channels and circular buffers for efficient output access
#[derive(Debug)]
pub struct StreamingHandle {
    /// Broadcast channel for real-time stdout
    pub stdout_tx: broadcast::Sender<String>,

    /// Broadcast channel for real-time stderr
    pub stderr_tx: broadcast::Sender<String>,

    /// In-memory circular buffer (last N lines) for fast polling
    pub stdout_buffer: Arc<Mutex<VecDeque<String>>>,
    pub stderr_buffer: Arc<Mutex<VecDeque<String>>>,

    /// Buffer size limit (default: 1000 lines)
    pub buffer_limit: usize,
}

impl StreamingHandle {
    /// Create new streaming handle with specified buffer limit
    pub fn new(buffer_limit: usize) -> Self {
        let (stdout_tx, _) = broadcast::channel(1000);
        let (stderr_tx, _) = broadcast::channel(1000);

        Self {
            stdout_tx,
            stderr_tx,
            stdout_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(buffer_limit))),
            stderr_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(buffer_limit))),
            buffer_limit,
        }
    }

    /// Add line to stdout buffer (circular, drops oldest if full)
    pub async fn push_stdout(&self, line: String) {
        let mut buffer = self.stdout_buffer.lock().await;
        if buffer.len() >= self.buffer_limit {
            buffer.pop_front();
        }
        buffer.push_back(line.clone());
        drop(buffer);

        let _ = self.stdout_tx.send(line);
    }

    /// Add line to stderr buffer (circular, drops oldest if full)
    pub async fn push_stderr(&self, line: String) {
        let mut buffer = self.stderr_buffer.lock().await;
        if buffer.len() >= self.buffer_limit {
            buffer.pop_front();
        }
        buffer.push_back(line.clone());
        drop(buffer);

        let _ = self.stderr_tx.send(line);
    }

    /// Get last N lines from buffer
    pub async fn get_tail(&self, stream: StreamType, n: usize) -> Vec<String> {
        let buffer = match stream {
            StreamType::Stdout => self.stdout_buffer.lock().await,
            StreamType::Stderr => self.stderr_buffer.lock().await,
        };

        buffer.iter().rev().take(n).rev().cloned().collect()
    }
}

/// Thread-safe process registry with cancellation tokens and streaming handles
#[derive(Debug)]
pub struct ProcessRegistryData {
    pub entries: HashMap<String, ProcessEntry>,
    pub cancellation_tokens: HashMap<String, CancellationToken>,
    pub streaming_handles: HashMap<String, Arc<StreamingHandle>>,
}

pub type ProcessRegistry = Arc<RwLock<ProcessRegistryData>>;

/// Create a new process registry
pub fn create_process_registry() -> ProcessRegistry {
    Arc::new(RwLock::new(ProcessRegistryData {
        entries: HashMap::new(),
        cancellation_tokens: HashMap::new(),
        streaming_handles: HashMap::new(),
    }))
}

/// Read last N lines from file (max 100, text only)
/// For large files (>1MB), uses optimized seek-from-end strategy
pub async fn tail_lines(file_path: &PathBuf, n: usize) -> Result<Vec<String>, String> {
    let n = n.min(100); // enforce max

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    // Get file size
    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|e| format!("Failed to get file metadata: {e}"))?;

    let file_size = metadata.len();

    // For small files (< 1MB), read entire file
    if file_size < 1_000_000 {
        // Windows: Use lossy UTF-8 conversion to handle non-UTF-8 console output
        // (cmd.exe outputs in system code page, not UTF-8)
        #[cfg(target_os = "windows")]
        let content = {
            let bytes = tokio::fs::read(file_path)
                .await
                .map_err(|e| format!("Failed to read file: {e}"))?;
            String::from_utf8_lossy(&bytes).to_string()
        };

        // Unix: Use strict UTF-8 (works fine on Unix systems)
        #[cfg(not(target_os = "windows"))]
        let content = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::InvalidData {
                    "Failed to read terminal output: Content appears to be binary or contains invalid UTF-8 characters".to_string()
                } else {
                    format!("Failed to read file: {e}")
                }
            })?;

        let lines: Vec<String> = content
            .lines()
            .rev()
            .take(n)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|s| s.to_string())
            .collect();

        return Ok(lines);
    }

    // For large files, use optimized approach
    // Read from end in chunks to find last N lines
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let mut file = tokio::fs::File::open(file_path)
        .await
        .map_err(|e| format!("Failed to open file: {e}"))?;

    let chunk_size = 8192u64;
    let mut pos = file_size;
    let mut buffer = Vec::new();
    let mut lines = Vec::new();

    while lines.len() < n && pos > 0 {
        let seek_pos = pos.saturating_sub(chunk_size);
        let read_size = (pos - seek_pos) as usize;

        file.seek(std::io::SeekFrom::Start(seek_pos))
            .await
            .map_err(|e| format!("Seek failed: {e}"))?;

        let mut chunk = vec![0u8; read_size];
        file.read_exact(&mut chunk)
            .await
            .map_err(|e| format!("Read failed: {e}"))?;

        buffer.splice(0..0, chunk);

        // Try to parse as UTF-8
        // Windows: Use lossy conversion for non-UTF-8 console output
        // Unix: Use strict UTF-8 validation
        #[cfg(target_os = "windows")]
        let text = String::from_utf8_lossy(&buffer).to_string();

        #[cfg(not(target_os = "windows"))]
        let text_opt = String::from_utf8(buffer.clone()).ok();

        #[cfg(target_os = "windows")]
        {
            let all_lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
            if all_lines.len() >= n {
                lines = all_lines.into_iter().rev().take(n).collect();
                lines.reverse();
                break;
            }
        }

        #[cfg(not(target_os = "windows"))]
        if let Some(text) = text_opt {
            let all_lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
            if all_lines.len() >= n {
                lines = all_lines.into_iter().rev().take(n).collect();
                lines.reverse();
                break;
            }
        }

        pos = seek_pos;
    }

    // If we couldn't get enough lines, use what we have
    if lines.is_empty() {
        #[cfg(target_os = "windows")]
        {
            let text = String::from_utf8_lossy(&buffer).to_string();
            lines = text
                .lines()
                .rev()
                .take(n)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|s| s.to_string())
                .collect();
        }

        #[cfg(not(target_os = "windows"))]
        if let Ok(text) = String::from_utf8(buffer) {
            lines = text
                .lines()
                .rev()
                .take(n)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|s| s.to_string())
                .collect();
        }
    }

    Ok(lines)
}

/// Read first N lines from file (max 100, text only)
pub async fn head_lines(file_path: &PathBuf, n: usize) -> Result<Vec<String>, String> {
    let n = n.min(100); // enforce max

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    // Windows: Use lossy UTF-8 conversion to handle non-UTF-8 console output
    // (cmd.exe outputs in system code page, not UTF-8)
    #[cfg(target_os = "windows")]
    let content = {
        let bytes = tokio::fs::read(file_path)
            .await
            .map_err(|e| format!("Failed to read file: {e}"))?;
        String::from_utf8_lossy(&bytes).to_string()
    };

    // Unix: Use strict UTF-8 (works fine on Unix systems)
    #[cfg(not(target_os = "windows"))]
    let content = tokio::fs::read_to_string(file_path)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                "Failed to read terminal output: Content appears to be binary or contains invalid UTF-8 characters".to_string()
            } else {
                format!("Failed to read file: {e}")
            }
        })?;

    let lines: Vec<String> = content.lines().take(n).map(|s| s.to_string()).collect();

    Ok(lines)
}

/// Read lines in range (1-based, inclusive, max 100 lines)
pub async fn read_lines_range(
    file_path: &PathBuf,
    start_line: usize,
    end_line: usize,
) -> Result<Vec<String>, String> {
    if !file_path.exists() {
        return Ok(Vec::new());
    }
    if start_line > end_line {
        return Ok(Vec::new());
    }

    // Safety clamp (max 100 lines to read)
    let count = end_line - start_line + 1;
    if count > 100 {
        return Err("Range too large (max 100 lines)".to_string());
    }

    // Windows: Use lossy UTF-8 conversion to handle non-UTF-8 console output
    #[cfg(target_os = "windows")]
    let content = {
        let bytes = tokio::fs::read(file_path)
            .await
            .map_err(|e| format!("Failed to read file: {e}"))?;
        String::from_utf8_lossy(&bytes).to_string()
    };

    // Unix: Use strict UTF-8 (works fine on Unix systems)
    #[cfg(not(target_os = "windows"))]
    let content = tokio::fs::read_to_string(file_path)
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                "Failed to read terminal output: Content appears to be binary or contains invalid UTF-8 characters".to_string()
            } else {
                format!("Failed to read file: {e}")
            }
        })?;

    // 0-based indexing for stream iter
    let skip = start_line.saturating_sub(1);

    let lines: Vec<String> = content
        .lines()
        .skip(skip)
        .take(count)
        .map(|s| s.to_string())
        .collect();

    Ok(lines)
}

/// Get file size in bytes
pub async fn get_file_size(file_path: &PathBuf) -> u64 {
    tokio::fs::metadata(file_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    #[tokio::test]
    async fn test_create_process_registry() {
        let registry = create_process_registry();
        assert!(registry.read().await.entries.is_empty());
    }

    #[tokio::test]
    async fn test_tail_lines() {
        // Create temp file
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_tail.txt");

        let content = "line1\nline2\nline3\nline4\nline5\n";
        fs::write(&test_file, content).await.unwrap();

        // Test tail
        let lines = tail_lines(&test_file, 3).await.unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line3");
        assert_eq!(lines[2], "line5");

        // Cleanup
        let _ = fs::remove_file(&test_file).await;
    }

    #[tokio::test]
    async fn test_tail_lines_max_limit() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_tail_max.txt");

        // Create 200 lines
        let mut content = String::new();
        for i in 1..=200 {
            content.push_str(&format!("line{}\n", i));
        }
        fs::write(&test_file, content).await.unwrap();

        // Request 200 lines, should get max 100
        let lines = tail_lines(&test_file, 200).await.unwrap();
        assert_eq!(lines.len(), 100);
        assert_eq!(lines[0], "line101");
        assert_eq!(lines[99], "line200");

        // Cleanup
        let _ = fs::remove_file(&test_file).await;
    }

    #[tokio::test]
    async fn test_head_lines() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_head.txt");

        let content = "line1\nline2\nline3\nline4\nline5\n";
        fs::write(&test_file, content).await.unwrap();

        let lines = head_lines(&test_file, 3).await.unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[2], "line3");

        let _ = fs::remove_file(&test_file).await;
    }

    #[tokio::test]
    async fn test_get_file_size() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_size.txt");

        let content = "Hello, World!";
        fs::write(&test_file, content).await.unwrap();

        let size = get_file_size(&test_file).await;
        assert_eq!(size, content.len() as u64);

        let _ = fs::remove_file(&test_file).await;
    }

    #[tokio::test]
    async fn test_nonexistent_file() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("nonexistent.txt");

        let lines = tail_lines(&test_file, 10).await.unwrap();
        assert!(lines.is_empty());

        let lines = head_lines(&test_file, 10).await.unwrap();
        assert!(lines.is_empty());

        let size = get_file_size(&test_file).await;
        assert_eq!(size, 0);
    }

    #[test]
    fn test_process_entry_initialization_with_poll_tracking() {
        let entry = ProcessEntry {
            id: "test-123".to_string(),
            session_id: "session-1".to_string(),
            command: "test command".to_string(),
            status: ProcessStatus::Starting,
            pid: None,
            exit_code: None,
            started_at: Utc::now(),
            finished_at: None,
            stdout_path: "/tmp/stdout".to_string(),
            stderr_path: "/tmp/stderr".to_string(),
            stdout_size: 0,
            stderr_size: 0,
            last_poll_at: None,
            poll_count: 0,
            consecutive_running_polls: 0,
            first_running_poll_at: None,
        };

        assert_eq!(entry.poll_count, 0);
        assert_eq!(entry.consecutive_running_polls, 0);
        assert!(entry.last_poll_at.is_none());
        assert!(entry.first_running_poll_at.is_none());
    }

    #[test]
    fn test_process_entry_serialization_with_poll_fields() {
        let entry = ProcessEntry {
            id: "test-456".to_string(),
            session_id: "session-2".to_string(),
            command: "echo hello".to_string(),
            status: ProcessStatus::Running,
            pid: Some(12345),
            exit_code: None,
            started_at: Utc::now(),
            finished_at: None,
            stdout_path: "/tmp/test/stdout".to_string(),
            stderr_path: "/tmp/test/stderr".to_string(),
            stdout_size: 100,
            stderr_size: 50,
            last_poll_at: Some(Utc::now()),
            poll_count: 5,
            consecutive_running_polls: 3,
            first_running_poll_at: Some(Utc::now()),
        };

        // Test serialization
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: ProcessEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.id, deserialized.id);
        assert_eq!(entry.poll_count, deserialized.poll_count);
        assert_eq!(
            entry.consecutive_running_polls,
            deserialized.consecutive_running_polls
        );
        assert!(deserialized.last_poll_at.is_some());
        assert!(deserialized.first_running_poll_at.is_some());
    }

    #[test]
    fn test_process_entry_deserialization_backward_compatibility() {
        // Old JSON without poll tracking fields
        let old_json = r#"{
            "id": "test-789",
            "session_id": "session-3",
            "command": "ls -la",
            "status": "Running",
            "pid": 99999,
            "exit_code": null,
            "started_at": "2025-01-01T00:00:00Z",
            "finished_at": null,
            "stdout_path": "/tmp/old/stdout",
            "stderr_path": "/tmp/old/stderr",
            "stdout_size": 200,
            "stderr_size": 100
        }"#;

        // Should deserialize successfully with default values for new fields
        let entry: ProcessEntry = serde_json::from_str(old_json).unwrap();

        assert_eq!(entry.id, "test-789");
        assert_eq!(entry.poll_count, 0); // Default value
        assert_eq!(entry.consecutive_running_polls, 0); // Default value
        assert!(entry.last_poll_at.is_none()); // Default value
        assert!(entry.first_running_poll_at.is_none()); // Default value
    }
}
