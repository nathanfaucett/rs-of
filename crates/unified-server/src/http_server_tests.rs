#[test]
fn http_setup_origin_is_exact_and_loopback_only() {
    for origin in [
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
        "http://localhost:3000",
        "https://unified.localhost",
    ] {
        assert!(
            crate::setup_host::allowed_origin(origin).is_ok(),
            "{origin}"
        );
    }
    for origin in [
        "http://evil.invalid",
        "http://localhost.evil.invalid",
        "http://localhost/path",
        "http://user@localhost",
        "http://localhost?code=secret",
        "http://localhost#code",
    ] {
        assert!(
            crate::setup_host::allowed_origin(origin).is_err(),
            "{origin}"
        );
    }
}
