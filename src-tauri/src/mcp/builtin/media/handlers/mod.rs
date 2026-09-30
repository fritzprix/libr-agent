//! Media builtin MCP tool handlers.
//!
//! Shared MIME/source/fetch helpers live in sibling modules; tool entrypoints
//! remain in `see_content`, `listen_content`, `capture_screen`, and
//! `assist_plugin`.

mod assist_plugin;
mod capture_screen;
mod fetch;
mod listen_content;
mod mime;
mod see_content;
mod source;

pub use assist_plugin::{handle_assist_plugin_status, handle_deploy_assist_plugin};
pub use capture_screen::handle_capture_screen;
pub use listen_content::handle_listen_content;
pub use see_content::{handle_see_content, MAX_SEE_CONTENT_SUCCESSES_PER_SESSION};

#[cfg(test)]
mod tool_description_tests {
    use super::MAX_SEE_CONTENT_SUCCESSES_PER_SESSION;

    #[test]
    fn see_tool_description_mentions_hard_session_limit() {
        let tools = super::super::tools::all_tools();
        let see = tools
            .iter()
            .find(|t| t.name == "seeContent")
            .expect("seeContent tool");
        assert!(see
            .description
            .contains(&MAX_SEE_CONTENT_SUCCESSES_PER_SESSION.to_string()));
        assert!(see.description.contains("Hard session limit"));
    }

    #[test]
    fn media_tool_descriptions_state_audio_vs_visual_role_boundary() {
        let tools = super::super::tools::all_tools();
        let see = tools
            .iter()
            .find(|t| t.name == "seeContent")
            .expect("seeContent tool");
        let listen = tools
            .iter()
            .find(|t| t.name == "listenContent")
            .expect("listenContent tool");
        assert!(see.description.contains("When to use:"));
        assert!(see.description.contains("listenContent"));
        assert!(see.description.to_lowercase().contains("not invent"));
        assert!(listen.description.contains("When to use:"));
        assert!(listen.description.contains("seeContent"));
        assert!(listen
            .description
            .to_lowercase()
            .contains("prefer this over inventing"));
    }
}
