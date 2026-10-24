use std::{sync::Arc, time::Duration};

use of_idp_client::apis::{configuration::Configuration, default_api::jwks};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

async fn response(bytes: &'static [u8], delay: Duration) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("controlled listener");
    let base = format!(
        "http://{}",
        listener.local_addr().expect("listener address")
    );
    let task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("generated request");
        let mut request = [0; 4096];
        let count = stream.read(&mut request).await.expect("request bytes");
        assert!(
            std::str::from_utf8(&request[..count])
                .expect("HTTP request")
                .starts_with("GET /.well-known/jwks.json ")
        );
        stream.write_all(bytes).await.expect("controlled response");
        tokio::time::sleep(delay).await;
    });
    (base, task)
}
fn controlled(base_path: String) -> Configuration {
    Configuration {
        base_path,
        client: reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_millis(100))
            .timeout(Duration::from_millis(250))
            .build()
            .expect("controlled HTTP transport"),
        ..Default::default()
    }
}

#[tokio::test]
async fn generated_transport_rejects_streaming_overflow_before_end_of_body() {
    let (base, server) = response(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n40\r\nxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r\n", Duration::from_secs(5)).await;
    let mut config = controlled(base);
    config.response_limit = 32;
    let error = tokio::time::timeout(Duration::from_millis(200), jwks(&config))
        .await
        .expect("refuse before body completes or request timeout")
        .expect_err("streaming size limit");
    assert!(error.to_string().contains("response exceeds byte limit"));
    server.abort();
}

#[tokio::test]
async fn generated_transport_bounds_errors_and_rejects_malformed_json_and_redirects() {
    for wire in [
        b"HTTP/1.1 400 Bad Request\r\nContent-Length: 99999999\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 3\r\n\r\nxxx".as_slice(),
        b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/never-follow\r\nContent-Length: 0\r\n\r\n".as_slice(),
    ] {
        let (base, server) = response(wire, Duration::ZERO).await;
        let error = jwks(&controlled(base)).await.expect_err("reject bounded error, malformed JSON or redirect");
        assert!(!matches!(error, of_idp_client::apis::Error::Reqwest(_)), "redirect must be returned, not followed");
        server.await.expect("controlled response task");
    }
}

#[tokio::test]
async fn generated_transport_honors_request_and_time_bounds_and_https_only_default() {
    let config = Configuration {
        base_path: "http://127.0.0.1:1".into(),
        ..Default::default()
    };
    let error = jwks(&config)
        .await
        .expect_err("production default refuses plaintext");
    assert!(error.to_string().contains("builder error"));
    let mut config = controlled("http://127.0.0.1:1".into());
    config.request_limit = 8;
    assert!(
        jwks(&config)
            .await
            .expect_err("bound request before connect")
            .to_string()
            .contains("request exceeds")
    );
    let mut headers = controlled("http://127.0.0.1:1".into());
    headers.request_limit = 64;
    headers.user_agent = Some("x".repeat(128));
    assert!(
        jwks(&headers)
            .await
            .expect_err("header bytes are bounded before connect")
            .to_string()
            .contains("request exceeds")
    );
    let (base, server) = response(
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n",
        Duration::from_secs(5),
    )
    .await;
    let started = std::time::Instant::now();
    assert!(jwks(&controlled(base)).await.is_err());
    assert!(started.elapsed() < Duration::from_secs(1));
    server.abort();
}

#[tokio::test]
async fn generated_transport_verifies_tls_instead_of_accepting_unknown_certificates() {
    let key = rcgen::generate_simple_self_signed(vec!["localhost".into()])
        .expect("untrusted test certificate");
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![key.cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(key.signing_key.serialize_der()).into(),
        )
        .expect("TLS server");
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("TLS listener");
    let base_path = format!(
        "https://localhost:{}",
        listener.local_addr().expect("TLS address").port()
    );
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("TLS attempt");
        assert!(
            acceptor.accept(stream).await.is_err(),
            "untrusted certificate aborts handshake"
        );
    });
    assert!(
        jwks(&Configuration {
            base_path,
            ..Default::default()
        })
        .await
        .is_err()
    );
    task.await.expect("TLS rejection observed");
}
