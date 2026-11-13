use std::{io, time::Duration};

pub fn verification_client() -> io::Result<reqwest::Client> {
    reqwest::Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .tls_built_in_native_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(1))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    #[test]
    fn system_trust_client_does_not_require_setup() {
        super::verification_client().expect("system trust verification client");
    }
}
