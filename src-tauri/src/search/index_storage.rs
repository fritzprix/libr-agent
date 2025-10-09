/// Index persistence module for BM25 message search.
///
/// Provides atomic file I/O operations for storing and loading search indices.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Index metadata stored in the index file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    /// Version of the index format for schema migration
    pub version: u32,
    /// Session ID this index belongs to
    pub session_id: String,
    /// Number of documents in the index
    pub doc_count: usize,
    /// Timestamp when index was last built (Unix milliseconds)
    pub last_built_at: i64,
}

/// Persistable index data combining metadata with serialized BM25 data.
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexData {
    pub metadata: IndexMetadata,
    /// Serialized BM25 embedder and scorer data
    pub index_content: Vec<u8>,
}

/// Atomically writes index data to a file using temp file + rename pattern.
///
/// # Arguments
/// * `path` - Target file path for the index
/// * `data` - Index data to write
///
/// # Returns
/// Result indicating success or error message
pub fn write_index_atomic(path: &Path, data: &IndexData) -> Result<(), String> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create index directory: {e}"))?;
    }

    // Create temp file in same directory for atomic rename
    let temp_file = NamedTempFile::new_in(
        path.parent()
            .ok_or_else(|| "Invalid index path".to_string())?,
    )
    .map_err(|e| format!("Failed to create temp file: {e}"))?;

    // Serialize to bincode
    let encoded =
        bincode::serialize(data).map_err(|e| format!("Failed to serialize index: {e}"))?;

    // Write to temp file
    std::fs::write(temp_file.path(), &encoded)
        .map_err(|e| format!("Failed to write temp file: {e}"))?;

    // Atomic rename
    temp_file
        .persist(path)
        .map_err(|e| format!("Failed to persist index file: {e}"))?;

    Ok(())
}

/// Reads index data from a file.
///
/// # Arguments
/// * `path` - Path to the index file
///
/// # Returns
/// Result containing the index data or error message
#[allow(dead_code)]
pub fn read_index(path: &Path) -> Result<IndexData, String> {
    if !path.exists() {
        return Err(format!("Index file not found: {}", path.display()));
    }

    let encoded = std::fs::read(path).map_err(|e| format!("Failed to read index file: {e}"))?;

    let data: IndexData =
        bincode::deserialize(&encoded).map_err(|e| format!("Failed to deserialize index: {e}"))?;

    Ok(data)
}

/// Gets the default index directory path for a given session.
///
/// Indices are stored in: `{app_data_dir}/message_indices/{session_id}.idx`
pub fn get_index_path(session_id: &str) -> Result<PathBuf, String> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| "Failed to get data directory".to_string())?
        .join("com.fritzprix.synapticflow")
        .join("message_indices");

    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create indices directory: {e}"))?;

    Ok(data_dir.join(format!("{session_id}.idx")))
}

/// Deletes the index file for a given session if it exists.
///
/// # Arguments
/// * `session_id` - The session ID whose index should be deleted
///
/// # Returns
/// Result indicating success or error message
pub fn delete_index(session_id: &str) -> Result<(), String> {
    let index_path = get_index_path(session_id)?;

    if index_path.exists() {
        std::fs::remove_file(&index_path)
            .map_err(|e| format!("Failed to delete index file: {e}"))?;
        log::info!("🗑️  Deleted search index for session: {session_id}");
    } else {
        log::debug!("No search index file found for session: {session_id}");
    }

    Ok(())
}

/// Deletes all index files in the message indices directory.
/// This is useful when clearing all sessions and their associated data.
///
/// # Returns
/// Result indicating success or error message, with count of deleted files
#[allow(dead_code)]
pub fn delete_all_indices() -> Result<usize, String> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| "Failed to get data directory".to_string())?
        .join("com.fritzprix.synapticflow")
        .join("message_indices");

    if !data_dir.exists() {
        log::debug!("No message indices directory found");
        return Ok(0);
    }

    let mut deleted_count = 0;

    let entries = std::fs::read_dir(&data_dir)
        .map_err(|e| format!("Failed to read indices directory: {e}"))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("idx") {
            if let Err(e) = std::fs::remove_file(&path) {
                log::warn!("Failed to delete index file {path:?}: {e}");
            } else {
                deleted_count += 1;
            }
        }
    }

    log::info!("🗑️  Deleted {deleted_count} search index files");
    Ok(deleted_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_write_read_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let index_path = temp_dir.path().join("test.idx");

        let original_data = IndexData {
            metadata: IndexMetadata {
                version: 1,
                session_id: "test-session".to_string(),
                doc_count: 100,
                last_built_at: 1234567890,
            },
            index_content: vec![1, 2, 3, 4, 5],
        };

        // Write
        write_index_atomic(&index_path, &original_data).unwrap();

        // Read
        let read_data = read_index(&index_path).unwrap();

        assert_eq!(read_data.metadata.version, 1);
        assert_eq!(read_data.metadata.session_id, "test-session");
        assert_eq!(read_data.metadata.doc_count, 100);
        assert_eq!(read_data.index_content, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_read_nonexistent_file() {
        let result = read_index(Path::new("/nonexistent/path.idx"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
