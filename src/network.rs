use anyhow::{Context, Result};
use quinn::{Endpoint, ServerConfig, ClientConfig};
use std::{net::SocketAddr, sync::Arc};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};

/// Create a QUIC server endpoint
pub fn make_server_endpoint(bind_addr: SocketAddr) -> Result<(Endpoint, Vec<u8>)> {
    let (cert, key) = generate_self_signed_cert()?;
    let cert_der = cert.clone();
    
    let server_config = ServerConfig::with_single_cert(vec![cert], key)?;
    // Note: ServerConfig::with_single_cert automatically selects a provider if one is available,
    // but to be safe we could install the default globally.
    // However, for the client we manually built the config, which is why it failed.
    // Let's just install the default provider globally at the start of the function to be safe.
    let _ = rustls::crypto::ring::default_provider().install_default();
    
    let endpoint = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, cert_der.to_vec()))
}

/// Create a QUIC client endpoint
pub fn make_client_endpoint(bind_addr: SocketAddr, _server_cert: &[u8]) -> Result<Endpoint> {
    let mut client_config = ClientConfig::new(Arc::new(quinn::crypto::rustls::QuicClientConfig::try_from(
        rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .dangerous()
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth()
    )?));
    
    // Enable keep-alives
    let mut transport_config = quinn::TransportConfig::default();
    transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(5)));
    client_config.transport_config(Arc::new(transport_config));

    let mut endpoint = Endpoint::client(bind_addr)?;
    endpoint.set_default_client_config(client_config);
    Ok(endpoint)
}

fn generate_self_signed_cert() -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])?;
    let key_der = cert.key_pair.serialize_der();
    let cert_der = cert.cert.der().clone();
    
    let key = PrivateKeyDer::Pkcs8(key_der.into());
    let cert = cert_der.into_owned();
    
    Ok((cert, key))
}

#[derive(Debug)]
struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
        ]
    }
}
