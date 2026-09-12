use tracing_subscriber::EnvFilter;

/// Picks the cryptography rustls will use, before anything builds a TLS client.
///
/// Two dependency trees bring rustls into this binary with different providers
/// enabled: the Kubernetes client pulls `ring`, the AWS SDK pulls `aws-lc-rs`.
/// The process-level default is then ambiguous, and rustls refuses to guess --
/// it says so by panicking at the first TLS handshake, which for an operator is
/// the first thing it does.
///
/// Latent until the object store adapter arrived, because the SDK's legacy
/// transport used rustls 0.21, which has no provider to choose.
///
/// `install_default` returns an error if something already installed one, which
/// is not worth failing a start over: what matters is that there is one.
fn install_crypto_provider() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    install_crypto_provider();

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    aether_operator_core::run().await?;

    Ok(())
}
