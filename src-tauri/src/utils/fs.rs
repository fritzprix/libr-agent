use std::path::Path;
use std::process::Command;

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
