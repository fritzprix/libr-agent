//! Host MediaAssist plugin (audio/image/video → text) under app data dir.
//!
//! Layout: `{base_data_dir}/harness-plugins/media-assist/v1/{manifest.json,run}`
//! Contract: https://github.com/fritzprix/libr-agent/issues/1926

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

pub const INTERFACE_VERSION: u32 = 1;
pub const PLUGIN_RELATIVE_DIR: &str = "harness-plugins/media-assist/v1";
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 300_000;
const MAX_OUTPUT_CHARS: usize = 8_000;
const MAX_DEPLOY_FILE_BYTES: usize = 2 * 1024 * 1024;
/// Cap decoded base64 media materialization (aligned with media tool limits).
const MAX_INPUT_BYTES: usize = 20 * 1024 * 1024;
/// Cap plugin stderr buffered into memory.
const MAX_PLUGIN_STDERR_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub interface_version: u32,
    pub name: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub modalities: Vec<String>,
}

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginStatus {
    pub installed: bool,
    pub path: String,
    pub modalities: Vec<String>,
    pub timeout_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    pub modality: String,
    pub mime_type: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub data_base64: Option<String>,
    #[serde(default)]
    pub max_output_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResponse {
    pub ok: bool,
    #[serde(default)]
    pub modality: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployFile {
    pub path: String,
    pub content: String,
    /// When true, content is base64-encoded bytes (for binary run helpers).
    #[serde(default)]
    pub base64: bool,
}

pub fn plugin_dir(base_data_dir: &Path) -> PathBuf {
    base_data_dir.join(PLUGIN_RELATIVE_DIR)
}

pub fn load_status(base_data_dir: &Path) -> PluginStatus {
    let dir = plugin_dir(base_data_dir);
    let path = dir.display().to_string();
    let manifest_path = dir.join("manifest.json");

    if !manifest_path.is_file() || resolve_run_executable(&dir).is_none() {
        return PluginStatus {
            installed: false,
            path,
            modalities: vec![],
            timeout_ms: DEFAULT_TIMEOUT_MS,
            error: None,
        };
    }

    match std::fs::read_to_string(&manifest_path)
        .map_err(|e| e.to_string())
        .and_then(|raw| serde_json::from_str::<PluginManifest>(&raw).map_err(|e| e.to_string()))
    {
        Ok(manifest) if manifest.interface_version == INTERFACE_VERSION => PluginStatus {
            installed: true,
            path,
            modalities: manifest.modalities,
            timeout_ms: manifest.timeout_ms.clamp(1_000, MAX_TIMEOUT_MS),
            error: None,
        },
        Ok(manifest) => PluginStatus {
            installed: false,
            path,
            modalities: vec![],
            timeout_ms: DEFAULT_TIMEOUT_MS,
            error: Some(format!(
                "unsupported interfaceVersion {} (want {INTERFACE_VERSION})",
                manifest.interface_version
            )),
        },
        Err(error) => PluginStatus {
            installed: false,
            path,
            modalities: vec![],
            timeout_ms: DEFAULT_TIMEOUT_MS,
            error: Some(error),
        },
    }
}

fn validate_deploy_relative_path(relative: &str) -> Result<PathBuf, String> {
    let trimmed = relative.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Err("deploy file path is empty".to_string());
    }
    if trimmed.ends_with('/') {
        return Err(format!(
            "deploy path must be a file, not a directory: {trimmed}"
        ));
    }
    if trimmed.starts_with('/') || trimmed.contains(':') {
        return Err(format!("deploy path must be relative: {trimmed}"));
    }
    let path = Path::new(&trimmed);
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let name = name.to_string_lossy();
                if name == ".." || name == "." {
                    return Err(format!("invalid path component in {trimmed}"));
                }
                if is_windows_reserved_device_name(&name) {
                    return Err(format!(
                        "deploy path uses a reserved Windows device name: {trimmed}"
                    ));
                }
            }
            _ => return Err(format!("invalid path component in {trimmed}")),
        }
    }
    let allowed = matches!(
        trimmed.as_str(),
        "manifest.json" | "run" | "run.exe" | "run.cmd" | "run.bat" | "run.py" | "README.md"
    ) || (trimmed.starts_with("fixtures/")
        && trimmed.len() > "fixtures/".len()
        && !trimmed[("fixtures/".len())..].contains('/'));
    if !allowed {
        return Err(format!(
            "deploy path '{trimmed}' not allowed (only manifest.json, run[, .exe/.cmd/.bat/.py], README.md, fixtures/<file>)"
        ));
    }
    Ok(PathBuf::from(trimmed))
}

fn is_windows_reserved_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn is_run_entry(rel: &Path) -> bool {
    matches!(
        rel.to_string_lossy().as_ref(),
        "run" | "run.exe" | "run.cmd" | "run.bat"
    )
}

/// Resolve the plugin executable. On Windows prefer `run.exe` / `run.cmd` / `run.bat`.
fn resolve_run_executable(dir: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // Extensionless `run` is not a valid Win32 application (os error 193).
        for name in ["run.exe", "run.cmd", "run.bat"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        let candidate = dir.join("run");
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    }
}

fn spawn_plugin_command(run_path: &Path, dir: &Path) -> Result<tokio::process::Child, String> {
    let extension = run_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let mut command = if extension == "cmd" || extension == "bat" {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(run_path);
        cmd
    } else {
        Command::new(run_path)
    };

    crate::utils::env::apply_isolated_env_async(&mut command);

    command
        .current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("failed to spawn plugin run: {e}"))
}

pub fn deploy_files(base_data_dir: &Path, files: &[DeployFile]) -> Result<PluginStatus, String> {
    if files.is_empty() {
        return Err("deploy requires at least one file".to_string());
    }

    let mut has_manifest = false;
    let mut has_run = false;
    let mut prepared: Vec<(PathBuf, Vec<u8>)> = Vec::new();

    for file in files {
        let rel = validate_deploy_relative_path(&file.path)?;
        let bytes = if file.base64 {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(file.content.trim())
                .map_err(|e| format!("invalid base64 for {}: {e}", file.path))?
        } else {
            file.content.as_bytes().to_vec()
        };
        if bytes.len() > MAX_DEPLOY_FILE_BYTES {
            return Err(format!(
                "file {} exceeds max size {} bytes",
                file.path, MAX_DEPLOY_FILE_BYTES
            ));
        }
        if rel == Path::new("manifest.json") {
            has_manifest = true;
            let manifest: PluginManifest = serde_json::from_slice(&bytes)
                .map_err(|e| format!("invalid manifest.json: {e}"))?;
            if manifest.interface_version != INTERFACE_VERSION {
                return Err(format!(
                    "manifest interfaceVersion must be {INTERFACE_VERSION}"
                ));
            }
        }
        if is_run_entry(&rel) {
            has_run = true;
        }
        prepared.push((rel, bytes));
    }

    if !has_manifest || !has_run {
        return Err(
            "deploy must include manifest.json and run (or run.exe/run.cmd/run.bat on Windows; optional run.py helper)"
                .to_string(),
        );
    }

    let dest = plugin_dir(base_data_dir);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)
            .map_err(|e| format!("failed to clear existing plugin dir: {e}"))?;
    }
    std::fs::create_dir_all(&dest).map_err(|e| format!("failed to create plugin dir: {e}"))?;

    for (rel, bytes) in prepared {
        let target = dest.join(&rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
        }
        std::fs::write(&target, &bytes)
            .map_err(|e| format!("failed to write {}: {e}", target.display()))?;
        if is_run_entry(&rel) {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&target)
                    .map_err(|e| format!("stat run: {e}"))?
                    .permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&target, perms).map_err(|e| format!("chmod run: {e}"))?;
            }
        }
    }

    let status = load_status(base_data_dir);
    if !status.installed {
        return Err(status
            .error
            .unwrap_or_else(|| "plugin deployed but status check failed".to_string()));
    }
    Ok(status)
}

/// Media path passed to the plugin. Ephemeral paths (from dataBase64) are deleted on drop.
struct MaterializedInput {
    path: PathBuf,
    ephemeral: bool,
}

impl MaterializedInput {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for MaterializedInput {
    fn drop(&mut self) {
        if self.ephemeral {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

async fn materialize_input_path(
    plugin_root: &Path,
    request: &RunRequest,
) -> Result<MaterializedInput, String> {
    if let Some(path) = request
        .path
        .as_ref()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
    {
        let path = if path.starts_with("file:") {
            let url = url::Url::parse(path).map_err(|e| format!("invalid file URL: {e}"))?;
            url.to_file_path()
                .map_err(|_| format!("file URL is not a local path: {path}"))?
        } else {
            PathBuf::from(path)
        };
        if path.is_file() {
            return Ok(MaterializedInput {
                path,
                ephemeral: false,
            });
        }
        // Container paths (e.g. /app/…) are not visible on the host. Fall through
        // to dataBase64 when the caller also provided inline bytes.
        let has_base64 = request
            .data_base64
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if !has_base64 {
            return Err(format!("media path not found: {}", path.display()));
        }
    }

    let data = request
        .data_base64
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "run requires path or dataBase64".to_string())?;

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| format!("invalid dataBase64: {e}"))?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "dataBase64 exceeds max size {MAX_INPUT_BYTES} bytes"
        ));
    }

    let tmp_dir = plugin_root.join(".tmp");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("tmp dir: {e}"))?;
    let ext = mime_extension(&request.mime_type);
    let tmp_path = tmp_dir.join(format!(
        "input-{}-{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4(),
        ext
    ));
    tokio::fs::write(&tmp_path, &bytes)
        .await
        .map_err(|e| format!("write temp media: {e}"))?;
    Ok(MaterializedInput {
        path: tmp_path,
        ephemeral: true,
    })
}

fn mime_extension(mime: &str) -> &'static str {
    match mime.to_lowercase().as_str() {
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/ogg" => "ogg",
        "audio/flac" => "flac",
        "audio/mp4" | "audio/m4a" => "m4a",
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        _ => "bin",
    }
}

pub async fn run_plugin(base_data_dir: &Path, request: RunRequest) -> Result<RunResponse, String> {
    let status = load_status(base_data_dir);
    if !status.installed {
        return Ok(RunResponse {
            ok: false,
            modality: Some(request.modality),
            text: None,
            error: Some("plugin_not_installed".to_string()),
            message: Some(
                status
                    .error
                    .unwrap_or_else(|| "MediaAssist plugin not installed".to_string()),
            ),
            notes: None,
        });
    }

    let modality = request.modality.trim().to_lowercase();
    if !matches!(modality.as_str(), "audio" | "image" | "video") {
        return Err(format!("unsupported modality '{modality}'"));
    }

    if !status.modalities.is_empty() {
        let allowed: HashSet<String> = status
            .modalities
            .iter()
            .map(|m| m.trim().to_lowercase())
            .collect();
        if !allowed.contains(&modality) {
            return Ok(RunResponse {
                ok: false,
                modality: Some(modality.clone()),
                text: None,
                error: Some("unsupported_modality".to_string()),
                message: Some(format!(
                    "plugin modalities {:?} do not include {modality}",
                    status.modalities
                )),
                notes: None,
            });
        }
    }

    let dir = plugin_dir(base_data_dir);
    let run_path = resolve_run_executable(&dir).ok_or_else(|| {
        "plugin run executable missing (expected run, or run.exe/run.cmd/run.bat on Windows)"
            .to_string()
    })?;
    // Hold MaterializedInput until function returns so ephemeral base64 temps are cleaned up.
    let media_input = materialize_input_path(&dir, &request).await?;

    let max_chars = request
        .max_output_chars
        .unwrap_or(MAX_OUTPUT_CHARS)
        .clamp(256, MAX_OUTPUT_CHARS);

    let stdin_payload = serde_json::json!({
        "interfaceVersion": INTERFACE_VERSION,
        "modality": modality,
        "mimeType": request.mime_type,
        "path": media_input.path().display().to_string(),
        "maxOutputChars": max_chars,
    });

    let mut child = spawn_plugin_command(&run_path, &dir)?;

    if let Some(mut stdin) = child.stdin.take() {
        let bytes = serde_json::to_vec(&stdin_payload).map_err(|e| e.to_string())?;
        stdin
            .write_all(&bytes)
            .await
            .map_err(|e| format!("plugin stdin write failed: {e}"))?;
        stdin
            .shutdown()
            .await
            .map_err(|e| format!("plugin stdin close failed: {e}"))?;
    }

    let mut stdout_reader = child
        .stdout
        .take()
        .ok_or_else(|| "plugin stdout pipe missing".to_string())?;
    let mut stderr_reader = child
        .stderr
        .take()
        .ok_or_else(|| "plugin stderr pipe missing".to_string())?;

    // Bound buffered output so a runaway plugin cannot exhaust host memory.
    let stdout_cap = max_chars
        .saturating_mul(4)
        .saturating_add(64 * 1024)
        .max(64 * 1024);
    let stderr_cap = MAX_PLUGIN_STDERR_BYTES;

    let timeout = Duration::from_millis(status.timeout_ms);
    let (status_code, stdout_bytes, stderr_bytes) = tokio::time::timeout(timeout, async {
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();
        let mut stdout_limited = (&mut stdout_reader).take(stdout_cap as u64);
        let mut stderr_limited = (&mut stderr_reader).take(stderr_cap as u64);
        let read_both = async {
            tokio::try_join!(
                stdout_limited.read_to_end(&mut stdout_buf),
                stderr_limited.read_to_end(&mut stderr_buf),
            )
            .map_err(|e| format!("plugin output read failed: {e}"))?;
            Ok::<_, String>((stdout_buf, stderr_buf))
        };
        let (buffers, exit_status) = tokio::try_join!(read_both, async {
            child
                .wait()
                .await
                .map_err(|e| format!("plugin wait failed: {e}"))
        })?;
        Ok::<_, String>((exit_status, buffers.0, buffers.1))
    })
    .await
    .map_err(|_| format!("plugin timed out after {}ms", status.timeout_ms))??;

    let stdout = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
    let stderr = String::from_utf8_lossy(&stderr_bytes).trim().to_string();

    if !status_code.success() {
        return Ok(RunResponse {
            ok: false,
            modality: Some(modality),
            text: None,
            error: Some("exec_failed".to_string()),
            message: Some(if stderr.is_empty() {
                format!("plugin exited with {status_code}")
            } else {
                stderr.chars().take(2000).collect()
            }),
            notes: None,
        });
    }

    let mut parsed: RunResponse = serde_json::from_str(&stdout).map_err(|e| {
        format!(
            "plugin stdout is not valid RunResponse JSON: {e}; stdout={}",
            stdout.chars().take(500).collect::<String>()
        )
    })?;

    if let Some(text) = parsed.text.as_mut() {
        if text.chars().count() > max_chars {
            *text = text.chars().take(max_chars).collect();
        }
    }

    if parsed.ok
        && parsed
            .text
            .as_ref()
            .map(|t| t.trim().is_empty())
            .unwrap_or(true)
    {
        parsed.ok = false;
        parsed.error = Some("empty_text".to_string());
        parsed.message = Some("plugin returned ok without text".to_string());
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn deploy_allows_run_py_helper() {
        assert!(validate_deploy_relative_path("run.py").is_ok());
    }

    #[test]
    fn deploy_rejects_trailing_slash_and_nested_fixtures() {
        assert!(validate_deploy_relative_path("fixtures/").is_err());
        assert!(validate_deploy_relative_path("fixtures/a/b.bin").is_err());
        assert!(validate_deploy_relative_path("fixtures/sample.wav").is_ok());
    }

    #[test]
    fn deploy_rejects_windows_reserved_device_names() {
        assert!(validate_deploy_relative_path("NUL").is_err());
        assert!(validate_deploy_relative_path("con.txt").is_err());
    }

    #[test]
    fn deploy_rejects_path_traversal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = deploy_files(
            tmp.path(),
            &[DeployFile {
                path: "../evil".into(),
                content: "x".into(),
                base64: false,
            }],
        );
        assert!(err.is_err(), "parent path must be rejected");
    }

    #[test]
    fn deploy_rejects_disallowed_relative_paths() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = deploy_files(
            tmp.path(),
            &[
                DeployFile {
                    path: "manifest.json".into(),
                    content: serde_json::json!({
                        "interfaceVersion": 1,
                        "name": "x",
                        "modalities": ["audio"]
                    })
                    .to_string(),
                    base64: false,
                },
                DeployFile {
                    path: "lib/helper.py".into(),
                    content: "print(1)\n".into(),
                    base64: false,
                },
            ],
        );
        assert!(err.is_err(), "lib/helper.py must be rejected");
    }

    #[test]
    fn deploy_and_status_roundtrip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let base = tmp.path();
        let manifest = serde_json::json!({
            "interfaceVersion": 1,
            "name": "test-assist",
            "timeoutMs": 5000,
            "modalities": ["audio", "image", "video"]
        })
        .to_string();
        let run = if cfg!(windows) {
            "@echo off\r\necho {\"ok\":true,\"text\":\"hi\",\"modality\":\"audio\"}\r\n"
        } else {
            "#!/bin/sh\necho '{\"ok\":true,\"text\":\"hi\",\"modality\":\"audio\"}'\n"
        };
        let run_name = if cfg!(windows) { "run.cmd" } else { "run" };
        let status = deploy_files(
            base,
            &[
                DeployFile {
                    path: "manifest.json".into(),
                    content: manifest,
                    base64: false,
                },
                DeployFile {
                    path: run_name.into(),
                    content: run.into(),
                    base64: false,
                },
            ],
        )
        .expect("deploy");
        assert!(status.installed);
        assert!(plugin_dir(base).join(run_name).is_file());
        assert_eq!(load_status(base).modalities.len(), 3);
    }

    #[tokio::test]
    async fn run_plugin_cleans_ephemeral_base64_tmp() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let base = tmp.path();
        let run = if cfg!(windows) {
            "@echo off\r\necho {\"ok\":true,\"text\":\"from-base64\",\"modality\":\"audio\"}\r\n"
        } else {
            "#!/bin/sh\necho '{\"ok\":true,\"text\":\"from-base64\",\"modality\":\"audio\"}'\n"
        };
        let run_name = if cfg!(windows) { "run.cmd" } else { "run" };
        deploy_files(
            base,
            &[
                DeployFile {
                    path: "manifest.json".into(),
                    content: serde_json::json!({
                        "interfaceVersion": 1,
                        "name": "tmp-cleanup",
                        "timeoutMs": 10000,
                        "modalities": ["audio"]
                    })
                    .to_string(),
                    base64: false,
                },
                DeployFile {
                    path: run_name.into(),
                    content: run.into(),
                    base64: false,
                },
            ],
        )
        .expect("deploy");

        let response = run_plugin(
            base,
            RunRequest {
                modality: "audio".into(),
                mime_type: "audio/wav".into(),
                path: None,
                data_base64: Some(base64::engine::general_purpose::STANDARD.encode(b"RIFF")),
                max_output_chars: None,
            },
        )
        .await
        .expect("run_plugin");

        assert!(response.ok, "plugin should succeed: {response:?}");
        let tmp_dir = plugin_dir(base).join(".tmp");
        if tmp_dir.is_dir() {
            let leftovers: Vec<_> = std::fs::read_dir(&tmp_dir)
                .expect("read .tmp")
                .filter_map(|e| e.ok())
                .collect();
            assert!(
                leftovers.is_empty(),
                "ephemeral base64 inputs must be removed; found {leftovers:?}"
            );
        }
    }

    #[tokio::test]
    async fn materialize_falls_back_to_base64_when_host_path_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let input = materialize_input_path(
            tmp.path(),
            &RunRequest {
                modality: "audio".into(),
                mime_type: "audio/wav".into(),
                path: Some("/app/missing-on-host.wav".into()),
                data_base64: Some(base64::engine::general_purpose::STANDARD.encode(b"RIFF")),
                max_output_chars: None,
            },
        )
        .await
        .expect("fallback to dataBase64");
        assert!(input.path.is_file());
        assert!(input.ephemeral);
        drop(input);
    }
}
