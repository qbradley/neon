use std::{num::{NonZeroUsize, NonZeroU32}, path::Path};

use anyhow::Context;
use remote_storage::{GenericRemoteStorage, RemoteStorageConfig, RemoteStorageKind, RemotePath};

#[tokio::test]
async fn azure_list() -> anyhow::Result<()> {
    let remote_storage_config = RemoteStorageConfig {
        max_concurrent_syncs: NonZeroUsize::new(100).unwrap(),
        max_sync_errors: NonZeroU32::new(5).unwrap(),
        storage: RemoteStorageKind::Azure(),
    };
    let prefix = std::env::var("AZURE_PREFIX")?;
    let remote_storage = GenericRemoteStorage::from_config(&remote_storage_config).context("remote storage init")?;
    for file in remote_storage.list_prefixes(Some(&RemotePath::new(Path::new(&prefix))?)).await? {
        eprintln!("Prefix: {:?}", file);
    }
    for file in remote_storage.list_files(Some(&RemotePath::new(Path::new(&prefix))?)).await? {
        eprintln!("File: {:?}", file);
    }
    Ok(())
}