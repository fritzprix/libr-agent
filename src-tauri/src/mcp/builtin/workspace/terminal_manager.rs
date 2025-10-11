use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
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
}

/// Thread-safe process registry with cancellation tokens
#[derive(Debug)]
pub struct ProcessRegistryData {
    pub entries: HashMap<String, ProcessEntry>,
    pub cancellation_tokens: HashMap<String, CancellationToken>,
}

pub type ProcessRegistry = Arc<RwLock<ProcessRegistryData>>;

/// Create a new process registry
pub fn create_process_registry() -> ProcessRegistry {
    Arc::new(RwLock::new(ProcessRegistryData {
        entries: HashMap::new(),
        cancellation_tokens: HashMap::new(),
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
        let content = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|e| format!("Failed to read file: {e}"))?;

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
        if let Ok(text) = String::from_utf8(buffer.clone()) {
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

    let content = tokio::fs::read_to_string(file_path)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let lines: Vec<String> = content.lines().take(n).map(|s| s.to_string()).collect();

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
}
