use std::path::Path;
use std::process::Command;
use tokio::fs;
use tokio::io::AsyncReadExt;

/// Reads a file into a byte vector, ensuring it does not exceed the specified maximum size.
///
/// This function opens the file and reads up to `max_size` bytes. If the file is larger,
/// it returns an error. This prevents large memory allocations and handles potential
/// race conditions (TOCTOU) where the file size changes between check and read.
///
/// # Arguments
/// * `path` - The path to the file to read.
/// * `max_size` - The maximum allowed file size in bytes.
///
/// # Returns
/// * `Ok(Vec<u8>)` - The file content if within limits.
/// * `Err(String)` - Error if file is too large or cannot be read.
pub async fn read_file_with_limit(path: &Path, max_size: u64) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path)
        .await
        .map_err(|e| format!("Failed to open file: {}", e))?;

    let metadata = file
        .metadata()
        .await
        .map_err(|e| format!("Failed to get metadata: {}", e))?;

    if metadata.len() > max_size {
        return Err(format!(
            "File too large: {} bytes (max: {} bytes)",
            metadata.len(),
            max_size
        ));
    }

    // Compute max_size + 1 with overflow checking to avoid panic/wrap on u64::MAX
    let max_size_plus_one = max_size
        .checked_add(1)
        .ok_or_else(|| "Configured maximum file size is too large to handle safely".to_string())?;

    // Allocate buffer based on file size, but capped at max_size_plus_one (for overflow check)
    let capacity = (metadata.len() as usize).min(max_size_plus_one as usize);
    let mut buffer = Vec::with_capacity(capacity);

    // Read up to max_size_plus_one bytes to detect if file is larger
    let bytes_read = file
        .take(max_size_plus_one)
        .read_to_end(&mut buffer)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    if bytes_read as u64 > max_size {
        return Err(format!(
            "File too large: exceeds maximum allowed size of {} bytes",
            max_size
        ));
    }

    Ok(buffer)
}

/// Opens the specified path in the system's default file manager.
///
/// This function handles the OS-specific commands to open a directory or file
/// in Explorer (Windows), Finder (macOS), or a compatible file manager (Linux).
///
/// # Arguments
/// * `path` - The path to open.
///
/// # Returns
/// * `Result<(), String>` - Ok if the command was spawned successfully, Err otherwise.
pub fn open_in_file_manager<P: AsRef<Path>>(path: P) -> Result<(), String> {
    let path = path.as_ref();

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open Finder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        let file_managers = ["nautilus", "dolphin", "thunar", "pcmanfm", "xdg-open"];
        let mut opened = false;
        let mut errors: Vec<String> = Vec::new();

        for fm in &file_managers {
            match Command::new(fm).arg(path).spawn() {
                Ok(_) => {
                    opened = true;
                    break;
                }
                Err(e) => {
                    errors.push(format!("{}: {}", fm, e));
                }
            }
        }

        if !opened {
            let error_details = if errors.is_empty() {
                String::new()
            } else {
                format!("\n\nAttempted commands and errors:\n{}", errors.join("\n"))
            };
            return Err(format!(
                "No file manager found. Supported: nautilus, dolphin, thunar, pcmanfm, xdg-open{}",
                error_details
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_read_file_with_limit_success() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Hello world").unwrap();
        // Flush to ensure data is written
        file.as_file().sync_all().unwrap();

        let path = file.path().to_path_buf();

        // "Hello world\n" is 12 bytes on unix (or more on windows)
        // Use a limit of 100 bytes
        let result = read_file_with_limit(&path, 100).await;
        assert!(result.is_ok(), "Should read small file successfully");

        let content = result.unwrap();
        assert!(!content.is_empty());
    }

    #[tokio::test]
    async fn test_read_file_with_limit_failure() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Hello world").unwrap();
        file.as_file().sync_all().unwrap();

        let path = file.path().to_path_buf();

        // Limit 5 bytes. File is > 10.
        let result = read_file_with_limit(&path, 5).await;
        assert!(result.is_err(), "Should fail for large file");

        let err = result.unwrap_err();
        assert!(err.contains("File too large"));
    }
}
