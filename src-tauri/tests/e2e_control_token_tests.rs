//! E2E control plane guard tests (debug builds only).
//!
//! These exercise the pure pieces — the capability guard filter and the
//! constant-time token compare — without touching env vars or process globals,
//! so they are deterministic regardless of test order.

#[cfg(debug_assertions)]
mod tests {
    use tauri_mcp_agent_lib::server::access_guard::{access_guard, handle_rejection, loopback_peer_guard};
    use tauri_mcp_agent_lib::server::e2e_control::{
        capability_guard, constant_time_eq, header_matches, E2E_CONTROL_HEADER,
    };
    use warp::http::HeaderMap;
    use warp::Filter;

    const TOKEN: &str = "0f2a8c5e1b7d4e9a8c6f3d2b1a0e9c8d7f6e5d4c3b2a1908";

    fn guarded(token: Option<&str>) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        let filter = capability_guard(token.map(str::to_string)).map(|| "ok");
        filter.recover(handle_rejection)
    }

    #[test]
    fn constant_time_eq_accepts_equal_and_rejects_unequal() {
        assert!(constant_time_eq(TOKEN, TOKEN));
        assert!(!constant_time_eq(TOKEN, "wrong"));
        assert!(!constant_time_eq(TOKEN, &TOKEN[..TOKEN.len() - 1]));
        assert!(!constant_time_eq("", TOKEN));
    }

    #[test]
    fn header_matches_requires_exact_capability() {
        let mut headers = HeaderMap::new();
        headers.insert(E2E_CONTROL_HEADER, TOKEN.parse().unwrap());
        assert!(header_matches(&headers, TOKEN));
        assert!(!header_matches(&headers, "not-the-token"));
        assert!(!header_matches(&HeaderMap::new(), TOKEN));
    }

    #[tokio::test]
    async fn disabled_control_plane_answers_404_even_with_a_token() {
        let filter = guarded(None);
        let res = warp::test::request()
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&filter)
            .await;
        assert_eq!(res.status(), 404);
    }

    #[tokio::test]
    async fn correct_capability_header_is_accepted() {
        let res = warp::test::request()
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&guarded(Some(TOKEN)))
            .await;
        assert_eq!(res.status(), 200);
    }

    #[tokio::test]
    async fn wrong_or_missing_capability_header_is_403() {
        let wrong = warp::test::request()
            .header(E2E_CONTROL_HEADER, "wrong-value")
            .reply(&guarded(Some(TOKEN)))
            .await;
        assert_eq!(wrong.status(), 403);

        let missing = warp::test::request().reply(&guarded(Some(TOKEN))).await;
        assert_eq!(missing.status(), 403);
    }

    #[tokio::test]
    async fn non_loopback_peer_is_rejected_with_valid_capability() {
        let chain = loopback_peer_guard()
            .and(capability_guard(Some(TOKEN.to_string())))
            .map(|| "ok")
            .recover(handle_rejection);

        let lan = warp::test::request()
            .remote_addr("192.168.1.100:54321".parse().unwrap())
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&chain)
            .await;
        assert_eq!(lan.status(), 403);

        let loopback = warp::test::request()
            .remote_addr("127.0.0.1:54321".parse().unwrap())
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&chain)
            .await;
        assert_eq!(loopback.status(), 200);
    }

    #[tokio::test]
    async fn e2e_chain_pins_loopback_host_even_when_expose_would_allow_any() {
        // The e2e wiring composes access_guard(false) regardless of the
        // server-wide expose flag: a LAN Host header must still fail even
        // though an expose=true server would otherwise accept it.
        let e2e_chain = access_guard(false)
            .and(capability_guard(Some(TOKEN.to_string())))
            .map(|| "ok")
            .recover(handle_rejection);

        let res = warp::test::request()
            .header("host", "192.168.1.100:3030")
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&e2e_chain)
            .await;
        assert_eq!(res.status(), 403);

        let ok = warp::test::request()
            .header("host", "127.0.0.1:3030")
            .header(E2E_CONTROL_HEADER, TOKEN)
            .reply(&e2e_chain)
            .await;
        assert_eq!(ok.status(), 200);
    }
}
