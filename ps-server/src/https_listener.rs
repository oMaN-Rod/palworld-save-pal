//! Native HTTPS hosting: an axum `Listener` that terminates TLS on the
//! server's own port using the self-signed certificate the network policy
//! owns (see `ps_network::https`). Swapped in wherever the policy sets
//! `https_enabled`; plain HTTP keeps the tokio TcpListener everywhere else.
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::serve::{Listener, ListenerExt, TapIo};
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio_rustls::rustls::ServerConfig;

/// The decrypted stream's shape — the tap callback's parameter.
type TlsStream = tokio_rustls::server::TlsStream<tokio::net::TcpStream>;
/// A fn POINTER (not an anonymous closure) so the return type of
/// `tls_listener` can name it and stay concrete; axum's `Connected` impl
/// for ConnectInfo is keyed on the TapIo type.
type TlsTap = fn(&mut TlsStream);

pub struct TlsListener {
    tcp: TcpListener,
    acceptor: tokio_rustls::TlsAcceptor,
}

impl Listener for TlsListener {
    type Io = TlsStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match self.tcp.accept().await {
                Ok((stream, addr)) => match self.acceptor.accept(stream).await {
                    Ok(tls) => return (tls, addr),
                    // A declined handshake only costs that one connection
                    // (most often a plain-HTTP caller of a TLS port); the
                    // listener itself keeps serving.
                    Err(error) => {
                        tracing::debug!(%addr, %error, "TLS handshake declined");
                    }
                },
                Err(error) => {
                    tracing::debug!(%error, "TCP accept failed; backing off");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.tcp.local_addr()
    }
}

/// Builds the TLS listener over an already-bound TCP listener, loading (or
/// generating) the self-signed certificate pair from `cert_dir` — the
/// deployment's app directory, the one place guaranteed writable.
///
/// The `.tap_io` wrap is load-bearing: axum ships its `Connected`
/// (ConnectInfo) implementation for TapIo-wrapped listeners keyed on the
/// listener's `Addr`, which is how the gate keeps seeing the real peer
/// address through the TLS layer.
pub fn tls_listener(
    tcp: TcpListener,
    cert_dir: &Path,
) -> anyhow::Result<TapIo<TlsListener, TlsTap>> {
    let certificate = ps_network::https::ensure_certificate(cert_dir, &[])?;
    let certs: Vec<CertificateDer<'static>> =
        CertificateDer::pem_slice_iter(certificate.cert_pem.as_bytes())
            .collect::<Result<_, _>>()
            .map_err(|error| anyhow::anyhow!("stored HTTPS certificate is unreadable: {error}"))?;
    let key: PrivateKeyDer<'static> =
        PrivateKeyDer::from_pem_slice(certificate.key_pem.as_bytes())
            .map_err(|error| anyhow::anyhow!("stored HTTPS key is unreadable: {error}"))?;
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|error| anyhow::anyhow!("TLS protocol versions rejected: {error}"))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|error| anyhow::anyhow!("HTTPS certificate/key pair rejected: {error}"))?;
    tracing::info!(
        fingerprint = %sha256_fingerprint(&certificate.cert_pem),
        "native HTTPS enabled (self-signed certificate)"
    );
    let listener = TlsListener {
        tcp,
        acceptor: tokio_rustls::TlsAcceptor::from(Arc::new(config)),
    };
    // TCP_NODELAY on the decrypted stream, matching axum's own TcpListener
    // accept behavior; the tap is also what unlocks ConnectInfo.
    let tap: TlsTap = |stream| {
        let _ = stream.get_ref().0.set_nodelay(true);
    };
    Ok(listener.tap_io(tap))
}

/// First 8 bytes of the certificate's SHA-256, hex — enough for an operator
/// to eyeball that the browser warning matches this deployment.
fn sha256_fingerprint(cert_pem: &str) -> String {
    use sha2::{Digest, Sha256};
    let der = CertificateDer::pem_slice_iter(cert_pem.as_bytes())
        .next()
        .map(|cert| cert.expect("generation produced a parseable certificate").to_vec())
        .unwrap_or_default();
    let digest = Sha256::digest(&der);
    digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}
