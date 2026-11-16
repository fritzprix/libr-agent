//! Regression lock for GUI startup hosting its own backend (#2853).

#[test]
fn gui_setup_does_not_attach_to_a_shared_backend() {
    let source = include_str!("mod.rs");
    for forbidden in [
        "attach_or_start_backend",
        "init_shared_backend_client",
        "exit_after_backend_loss",
        "BackendStartup::Attached",
        "hold_frontend_attachment",
    ] {
        assert!(
            !source.contains(forbidden),
            "GUI setup must host locally and must not mention {forbidden}"
        );
    }
    assert!(
        source.contains("wait_until_own_record_live"),
        "GUI setup must wait until this process publishes its own backend"
    );
    assert!(
        source.contains("crate::server::init"),
        "GUI setup must start the in-process HTTP server"
    );
}
