#[cfg(debug_assertions)]
mod tests {
    use tauri_mcp_agent_lib::server::e2e_control::token::{
        authorize, e2e_control_enabled, init_capability_token, capability_token, E2E_CONTROL_HEADER,
    };
    use warp::http::HeaderMap;

    #[test]
    fn e2e_disabled_without_opt_in_env() {
        std::env::remove_var("LIBRAGENT_E2E_CONTROL");
        assert!(!e2e_control_enabled());
    }

    #[test]
    fn authorize_rejects_missing_header() {
        std::env::set_var("LIBRAGENT_E2E_CONTROL", "1");
        init_capability_token();
        let mut headers = HeaderMap::new();
        assert!(authorize(&headers).is_err());
        std::env::remove_var("LIBRAGENT_E2E_CONTROL");
    }

    #[test]
    fn authorize_accepts_matching_capability() {
        std::env::set_var("LIBRAGENT_E2E_CONTROL", "1");
        init_capability_token();
        let token = capability_token().expect("token");
        let mut headers = HeaderMap::new();
        headers.insert(E2E_CONTROL_HEADER, token.parse().unwrap());
        assert!(authorize(&headers).is_ok());
        std::env::remove_var("LIBRAGENT_E2E_CONTROL");
    }
}
