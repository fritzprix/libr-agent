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
        description: r#"Fetch an image and include it in the conversation so you can visually analyse it.

**Supported formats:** JPEG, PNG, GIF, WebP, BMP, SVG

**Notes:**
- Maximum file size: 20 MB.
- Local paths must be inside the session workspace."#
            .to_string(),
        input_schema: object_prop(
            vec![(
                "url".to_string(),
                string_prop_required(
                    "URL or workspace-relative path of the image to fetch (e.g. https://example.com/photo.jpg or screenshots/capture.png).",
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
        description: r#"Fetch an audio file and include it in the conversation so you can analyse the audio.

**Supported formats:** MP3, WAV, OGG, AAC, FLAC, WEBM

**Notes:**
- Maximum file size: 20 MB.
- Local paths must be inside the session workspace."#
            .to_string(),
        input_schema: object_prop(
            vec![(
                "url".to_string(),
                string_prop_required(
                    "URL or workspace-relative path of the audio file to fetch (e.g. https://example.com/clip.mp3 or recordings/audio.wav).",
                ),
            )],
            vec!["url".to_string()],
            None,
        ),
        output_schema: None,
        annotations: None,
    }
}
