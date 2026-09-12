pub mod archive;
pub mod edge;
pub mod identity_instance;
pub mod identity_instance_backup;
pub mod identity_instance_upgrade;
pub mod manifest_store;

use futures::try_join;

use crate::domain::OperatorError;

pub async fn run() -> Result<(), OperatorError> {
    // Read before anything watches. A data plane that archives nothing is a
    // decision somebody can make, and it is a different one from a store being
    // unreachable, so it is said once here rather than discovered per instance.
    let store = archive::ArchiveStore::from_env();
    match &store {
        Some(store) => tracing::info!(
            endpoint = store.endpoint_url.as_deref().unwrap_or("aws"),
            "archives from this data plane go to this store"
        ),
        None => tracing::warn!("no object store is configured: this data plane archives nothing"),
    }

    let manifests: std::sync::Arc<dyn identity_instance_backup::ManifestWriter> = match &store {
        Some(store) => std::sync::Arc::new(manifest_store::S3ManifestWriter::new(store)),
        None => std::sync::Arc::new(NoManifests),
    };

    try_join!(
        identity_instance::run(),
        identity_instance_upgrade::run(),
        identity_instance_backup::run(manifests),
    )?;
    Ok(())
}

/// What writes manifests when there is nowhere to write them.
///
/// Refuses rather than succeeding quietly. The caller treats a failed manifest
/// as a warning beside a successful archive, which is the right handling; a
/// writer that returned `Ok` would report a manifest that does not exist.
struct NoManifests;

#[async_trait::async_trait]
impl identity_instance_backup::ManifestWriter for NoManifests {
    async fn write(&self, _: &str, _: &str, _: Vec<u8>) -> Result<(), OperatorError> {
        Err(OperatorError::Configuration {
            message: "no object store is configured for this data plane".to_string(),
        })
    }
}
