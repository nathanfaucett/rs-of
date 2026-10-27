use alloc::{string::String, vec, vec::Vec};

use base64::Engine;

use crate::PasswordConfig;

pub fn generate_random_string<const N: usize>() -> String {
    // TODO: use generic_const_exprs once stablized to enforce N > 0 at compile time
    debug_assert_ne!(N, 0);
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("Failed to generate random bytes");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn random_bytes(size: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; size];
    getrandom::fill(&mut bytes).expect("Failed to generate random bytes");
    bytes.to_vec()
}

pub fn encrypt_password(config: &PasswordConfig, input: &str) -> argon2::Result<String> {
    #[cfg(feature = "std")]
    let _observation = PasswordObservation::start("hash");
    argon2::hash_encoded(
        input.as_bytes(),
        random_bytes(config.salt_length).as_slice(),
        &argon2_config(config),
    )
}

pub fn verify_password(input: &str, encrypted_password: &str) -> argon2::Result<bool> {
    if encrypted_password.is_empty() {
        return Ok(false);
    }
    #[cfg(feature = "std")]
    let _observation = PasswordObservation::start("verify");
    argon2::verify_encoded(encrypted_password, input.as_bytes())
}

#[cfg(feature = "std")]
struct PasswordObservation(Option<(&'static str, std::time::Instant)>);

#[cfg(feature = "std")]
impl PasswordObservation {
    fn start(phase: &'static str) -> Self {
        let started = std::env::var_os("OF_SETUP_TIMEOUT_TIMING").map(|_| {
            eprintln!(
                "seam=password phase={phase}_start pid={} thread={:?} wall_ms={}",
                std::process::id(),
                std::thread::current().id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock")
                    .as_millis()
            );
            (phase, std::time::Instant::now())
        });
        Self(started)
    }
}

#[cfg(feature = "std")]
impl Drop for PasswordObservation {
    fn drop(&mut self) {
        if let Some((phase, started)) = self.0 {
            eprintln!(
                "seam=password phase={phase}_end pid={} thread={:?} wall_ms={} elapsed_ms={}",
                std::process::id(),
                std::thread::current().id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system clock")
                    .as_millis(),
                started.elapsed().as_millis()
            );
        }
    }
}

fn argon2_config(config: &PasswordConfig) -> argon2::Config<'_> {
    argon2::Config {
        variant: argon2::Variant::Argon2id,
        hash_length: config.hash_length,
        lanes: config.parallelism,
        mem_cost: config.memory_mib * 1024,
        time_cost: config.iterations,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn missing_local_verifier_denies_without_a_password_parser_error() {
        assert!(
            !super::verify_password("presented-secret", "").expect("missing verifier is a denial")
        );
    }
}
