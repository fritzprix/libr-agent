# MediaAssist Plugin Architecture

This document describes the host MediaAssist plugin architecture in LibrAgent (`src-tauri/src/media_assist/`).
The MediaAssist subsystem provides an extensible, sandboxed out-of-process engine for audio, image, and video to text transcription and summarization.

---

## 1. Overview & Motivation

Multimodal perception (audio, video, high-resolution imagery) often requires specialized runtimes (e.g., local Whisper models, FFmpeg, vision scripts, or external CLI converters).
Bundling every multimodal model directly into the core LibrAgent Tauri binary would cause binary bloat, memory exhaustion, and native dependency conflicts across Linux, macOS, and Windows.

To solve this, LibrAgent establishes a **Host MediaAssist Plugin contract** (`INTERFACE_VERSION = 1`).
The plugin lives under the user's application data directory, allowing user-deployed or harness-provisioned scripts to process media while keeping the host application isolated, responsive, and secure.

---

## 2. Directory Layout & Discovery

The MediaAssist plugin directory is located at a fixed relative path within the application data directory:

```text
{app_data_dir}/harness-plugins/media-assist/v1/
├── manifest.json       # Required: plugin metadata and supported modalities
├── run                 # Required entrypoint on Unix (chmod +x)
├── run.exe             # Alternative entrypoint on Windows
├── run.cmd / run.bat   # Windows batch/cmd entrypoint wrapper
├── run.py              # Optional helper script invoked by run/run.cmd
├── README.md           # Optional documentation
└── fixtures/           # Optional test fixtures (single-level directory)
```

### Discovery Logic

The host checks for plugin installation via `media_assist::load_status(base_data_dir)`:
1. Verifies that `manifest.json` exists and parses valid JSON.
2. Checks that `interfaceVersion` equals `1` (`pub const INTERFACE_VERSION: u32 = 1`).
3. Verifies that the platform entrypoint exists:
   - **Unix**: Looks for an executable file named `run`.
   - **Windows**: Checks for `run.exe`, `run.cmd`, or `run.bat`.

---

## 3. Manifest Specification (`manifest.json`)

The manifest defines plugin capabilities and execution limits:

```json
{
  "interfaceVersion": 1,
  "name": "whisper-ffmpeg-media-assist",
  "timeoutMs": 120000,
  "modalities": ["audio", "image", "video"]
}
```

### Fields

| Field | Type | Description |
|---|---|---|
| `interfaceVersion` | `u32` | Must be `1`. Other versions result in an unsupported version error. |
| `name` | `string` | Human-readable name of the plugin. |
| `timeoutMs` | `u64` | Maximum execution time in milliseconds (default: 120,000 ms, clamped to 1,000–300,000 ms). |
| `modalities` | `string[]` | Supported media modalities: `"audio"`, `"image"`, `"video"`. |

---

## 4. Execution Protocol (IPC)

The host communicates with the plugin process via standard input and output (`stdin` / `stdout`) using JSON-encoded messages.

### Request Payload (`RunRequest`)

The host sends a single JSON line to the plugin's `stdin`:

```json
{
  "modality": "audio",
  "mimeType": "audio/wav",
  "path": "/path/to/media.wav",
  "dataBase64": null,
  "maxOutputChars": 8000
}
```

- `modality`: Targeted media type (`"audio"`, `"image"`, or `"video"`).
- `mimeType`: Standard MIME type (e.g., `audio/mp3`, `video/mp4`, `image/png`).
- `path`: Absolute filesystem path to the media file (if available locally).
- `dataBase64`: Base64-encoded media payload (if the media was received via memory or inline tool call). Materialization is capped at 20MB.
- `maxOutputChars`: Maximum length of output text (capped at 8,000 characters by default).

### Response Payload (`RunResponse`)

The plugin writes a JSON line to `stdout`:

```json
{
  "ok": true,
  "modality": "audio",
  "text": "Transcribed speech text from audio file...",
  "notes": "Processed using local Whisper base model in 1.4s",
  "error": null,
  "message": null
}
```

---

## 5. Security & Isolation

Running out-of-process plugins requires strict sandboxing and input validation:

1. **Isolated Environment**:
   - `crate::utils::env::apply_isolated_env_async(&mut command)` strips sensitive parent process tokens, credentials, and unwanted shell overrides.
2. **Path Sanitization & Whitelisting**:
   - Only approved filenames (`manifest.json`, `run`, `run.exe`, `run.cmd`, `run.bat`, `run.py`, `README.md`, `fixtures/<file>`) can be deployed.
   - Path traversal components (`..`, `.`, leading `/`, drive letters) are rejected.
   - Reserved Windows device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`) are blocked.
3. **Resource & Buffer Limits**:
   - **Deploy File Size**: Capped at 2MB per file.
   - **Base64 Media Materialization**: Capped at 20MB to prevent host memory exhaustion.
   - **Child Stderr**: Capped at 64KB buffered in memory to prevent runaway log buffering.
   - **Execution Timeout**: Enforced via `tokio::time::timeout` (1,000 ms to 300,000 ms).
   - **Automatic Cleanup**: `command.kill_on_drop(true)` ensures orphaned child processes are terminated immediately if the request is canceled or times out.

---

## 6. Integration Points

### 1. Tauri Commands

Exposed in `src-tauri/src/commands/media_assist_commands.rs` and registered in `lib.rs`:

- **`media_assist_plugin_status`**:
  Returns `PluginStatus` (`installed`, `path`, `modalities`, `timeoutMs`, `error`).
- **`media_assist_run_plugin`**:
  Executes the plugin against a `RunRequest` and returns a `RunResponse`.

### 2. Builtin Media MCP Server

The builtin Media server handler (`src-tauri/src/mcp/builtin/media/handlers/assist_plugin.rs`) exposes tools for agents to query plugin status or deploy runtime plugin scripts directly:
- Agents can query `media_assist_status` to determine if local transcription or vision capabilities are available.
- Agents can deploy scripts via `media_assist_deploy` during setup or benchmark harness execution.
