use std::sync::Arc;

use hyper::http::uri::Authority;
use moka::future::Cache;
use rand::{thread_rng, Rng};
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, Ia5String, KeyPair};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    ServerConfig,
};

const KEY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/core/certs/roxy.key"
));
const CERT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/core/certs/roxy.cer"
));

const MAX_CAPACITY: u64 = 1000;

#[derive(Clone)]
pub struct TlsCertCache {
    key_pair: Arc<KeyPair>,
    cert: Arc<Certificate>,
    private_key: Arc<PrivateKeyDer<'static>>,
    inner: Cache<Authority, Arc<ServerConfig>>,
}

impl TlsCertCache {
    /// Creates a new TLS certificate cache.
    pub fn new() -> Self {
        let key_pair = KeyPair::from_pem(KEY).unwrap();
        let private_key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(key_pair.serialize_der()));

        let cert = CertificateParams::from_ca_cert_pem(CERT)
            .unwrap()
            .self_signed(&key_pair)
            .unwrap();

        Self {
            key_pair: Arc::new(key_pair),
            cert: Arc::new(cert),
            private_key: Arc::new(private_key),
            inner: Cache::new(MAX_CAPACITY),
        }
    }

    /// Gets a TLS configuration from the cache, or inserts a new one if it doesn't exist.
    pub async fn get_or_insert(&self, authority: &Authority) -> Arc<ServerConfig> {
        self.inner
            .entry(authority.clone())
            .or_insert_with(async move { Arc::new(self.generate_server_config(authority)) })
            .await
            .into_value()
    }

    /// Generates a TLS configuration for the given authority.
    fn generate_server_config(&self, authority: &Authority) -> ServerConfig {
        trace!("generating server config for {}", authority);
        let certs = vec![self.generate_cert(authority)];
        let mut server_cfg = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, self.private_key.clone_key())
            .unwrap();
        server_cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        server_cfg
    }

    /// Generates a certificate for the given authority.
    fn generate_cert(&self, authority: &Authority) -> CertificateDer<'static> {
        let mut params = CertificateParams::default();
        params.serial_number = Some(thread_rng().gen::<u64>().into());

        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, authority.host());
        params.distinguished_name = dn;

        params.subject_alt_names.push(rcgen::SanType::DnsName(
            Ia5String::try_from(authority.host()).unwrap(),
        ));

        params
            .signed_by(&self.key_pair, &self.cert, &self.key_pair)
            .expect("failed to sign certificate")
            .into()
    }
}

impl Default for TlsCertCache {
    fn default() -> Self {
        Self::new()
    }
}
