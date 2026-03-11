use crate::mcp::utils::schema_builder::*;
use crate::mcp::MCPTool;

/// Returns all tools provided by the Media server.
pub fn all_tools() -> Vec<MCPTool> {
    vec![see_tool(), listen_tool()]
}

fn see_tool() -> MCPTool {
    MCPTool {
        name: "seeContent".to_string(),
        title: Some("See Content".to_string()),
        description: r#"Fetch an image from a URL or local file path and include it directly in the conversation context so you can visually analyse it.

**Supported sources:**
- Web URLs: `https://example.com/photo.jpg`
- Local files: `/absolute/path/to/image.png` or a path relative to the workspace
- File URIs: `file:///path/to/image.png`

**Supported formats:** JPEG, PNG, GIF, WebP, BMP, SVG

**Notes:**
- Maximum file size: 20 MB. Larger images are rejected to protect memory.
- Only models that support vision (e.g., GPT-4o, Claude 3, Gemini) can interpret the returned image.
- For local paths the file must be inside the session workspace."#
            .to_string(),
        input_schema: object_prop(
            vec![(
                "url".to_string(),
                string_prop_required(
                    "URL or file path of the image to fetch (e.g. https://example.com/img.png or /workspace/screenshot.png).",
                ),
            )],
            vec!["url".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}

fn listen_tool() -> MCPTool {
    MCPTool {
        name: "listenContent".to_string(),
        title: Some("Listen Content".to_string()),
        description: r#"Fetch an audio file from a URL or local file path and include it directly in the conversation context so you can analyse the audio.

**Supported sources:**
- Web URLs: `https://example.com/audio.mp3`
- Local files: `/absolute/path/to/audio.wav` or a path relative to the workspace
- File URIs: `file:///path/to/audio.wav`

**Supported formats:** MP3, WAV, OGG, AAC, FLAC, WEBM

**Notes:**
- Maximum file size: 20 MB. Larger files are rejected to protect memory.
- Only models that natively support audio input (e.g., GPT-4o Audio Preview) can interpret the returned audio. Other models will not understand it.
- For local paths the file must be inside the session workspace."#
            .to_string(),
        input_schema: object_prop(
            vec![(
                "url".to_string(),
                string_prop_required(
                    "URL or file path of the audio file to fetch (e.g. https://example.com/clip.mp3 or /workspace/recording.wav).",
                ),
            )],
            vec!["url".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}
