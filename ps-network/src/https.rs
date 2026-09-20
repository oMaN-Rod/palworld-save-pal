//! Self-signed certificate lifecycle for native HTTPS hosting. The server
//! asks for a certificate once per deployment: `ensure_certificate` loads
//! the PEM pair beside the database when one exists (and looks intact) and
//! generates + persists a fresh one otherwise. Deleting the two files is the
//! supported way to rotate.
use std::net::IpAddr;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum HttpsCertError {
    #[error("could not generate a self-signed certificate: {0}")]
    Generate(#[from] rcgen::Error),
    #[error("certificate file I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

pub const CERT_FILE: &str = "https-cert.pem";
pub const KEY_FILE: &str = "https-key.pem";

/// A PEM certificate/key pair ready to hand to a TLS listener.
#[derive(Debug, Clone)]
pub struct HttpsCert {
    pub cert_pem: String,
    pub key_pem: String,
}

/// Returns the deployment's certificate, generating and persisting one on
/// first use. `extra_ips` become IP subject-alternative names (the server
/// passes its tailscale address so the tailnet hostname matches); the
/// base set always covers localhost and loopback.
///
/// Browsers will still warn about the self-signed certificate until the
/// user trusts it — that is inherent to self-signing, not a SAN problem.
pub fn ensure_certificate(dir: &Path, extra_ips: &[IpAddr]) -> Result<HttpsCert, HttpsCertError> {
    let cert_path = dir.join(CERT_FILE);
    let key_path = dir.join(KEY_FILE);
    if let (Ok(cert_pem), Ok(key_pem)) = (
        std::fs::read_to_string(&cert_path),
        std::fs::read_to_string(&key_path),
    ) {
        if looks_like_pair(&cert_pem, &key_pem) {
            return Ok(HttpsCert { cert_pem, key_pem });
        }
    }
    let certificate = generate(extra_ips)?;
    std::fs::create_dir_all(dir)?;
    // Mode 0600 on the key where Unix permissions exist; the certificate is
    // public by definition.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::write(&key_path, &certificate.key_pem)
            .and_then(|()| std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)))
        {
            return Err(HttpsCertError::Io(error));
        }
        let _ = std::fs::set_permissions(&cert_path, std::fs::Permissions::from_mode(0o644));
    }
    #[cfg(not(unix))]
    std::fs::write(&key_path, &certificate.key_pem)?;
    std::fs::write(&cert_path, &certificate.cert_pem)?;
    tracing::info!(
        cert = %cert_path.display(),
        "generated a self-signed HTTPS certificate"
    );
    Ok(certificate)
}

/// Cheap structural check that both halves of the pair survived on disk:
/// correct PEM headers, nothing more. Full validation belongs to the TLS
/// stack that consumes them.
fn looks_like_pair(cert_pem: &str, key_pem: &str) -> bool {
    cert_pem.contains("-----BEGIN CERTIFICATE-----")
        && key_pem.contains("-----BEGIN PRIVATE KEY-----")
}

fn generate(extra_ips: &[IpAddr]) -> Result<HttpsCert, HttpsCertError> {
    let key = rcgen::KeyPair::generate()?;
    let mut params = rcgen::CertificateParams::new(vec!["localhost".to_string()])?;
    let mut sans = vec![
        rcgen::SanType::DnsName(
            rcgen::string::Ia5String::try_from("localhost").expect("localhost is an IA5 string"),
        ),
        rcgen::SanType::IpAddress("127.0.0.1".parse().expect("loopback v4 parses")),
        rcgen::SanType::IpAddress("::1".parse().expect("loopback v6 parses")),
    ];
    sans.extend(
        extra_ips
            .iter()
            .copied()
            .filter(|ip| !ip.is_loopback())
            .map(rcgen::SanType::IpAddress),
    );
    params.subject_alt_names = sans;
    // Long-lived on purpose: a self-signed cert that expires strands remote
    // users with a mystery failure. Rotation = delete the files.
    params.not_after = rcgen::date_time_ymd(2036, 1, 1);
    let cert = params.self_signed(&key)?;
    Ok(HttpsCert {
        cert_pem: cert.pem(),
        key_pem: key.serialize_pem(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_once_then_reuses_the_stored_pair() {
        let dir = tempfile::tempdir().unwrap();
        let first = ensure_certificate(dir.path(), &[]).unwrap();
        assert!(dir.path().join(CERT_FILE).is_file());
        assert!(dir.path().join(KEY_FILE).is_file());
        let second = ensure_certificate(dir.path(), &["100.115.95.115".parse().unwrap()]).unwrap();
        // The stored pair wins even when the caller's SAN wish differs —
        // regeneration only happens on a missing/broken pair.
        assert_eq!(first.cert_pem, second.cert_pem);
        assert_eq!(first.key_pem, second.key_pem);
    }

    #[test]
    fn a_corrupted_pair_is_regenerated() {
        let dir = tempfile::tempdir().unwrap();
        let first = ensure_certificate(dir.path(), &[]).unwrap();
        std::fs::write(dir.path().join(KEY_FILE), "not a pem").unwrap();
        let second = ensure_certificate(dir.path(), &[]).unwrap();
        assert_ne!(first.key_pem, second.key_pem);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(KEY_FILE)).unwrap(),
            second.key_pem
        );
    }
}
