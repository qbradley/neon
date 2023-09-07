use std::{num::{NonZeroUsize, NonZeroU32}, path::Path};

use anyhow::Context;
use remote_storage::{GenericRemoteStorage, RemoteStorageConfig, RemoteStorageKind, RemotePath};
use serde::{Serialize, Deserialize};

fn get_remote_storage() -> anyhow::Result<GenericRemoteStorage> {
    let remote_storage_config = RemoteStorageConfig {
        max_concurrent_syncs: NonZeroUsize::new(100).unwrap(),
        max_sync_errors: NonZeroU32::new(5).unwrap(),
        storage: RemoteStorageKind::Azure(),
    };
    let remote_storage = GenericRemoteStorage::from_config(&remote_storage_config).context("remote storage init")?;
    Ok(remote_storage)
}

#[tokio::test]
async fn azure_list() -> anyhow::Result<()> {
    let remote_storage = get_remote_storage()?;
    let prefix = std::env::var("AZURE_PREFIX")?;
    for file in remote_storage.list_prefixes(Some(&RemotePath::new(Path::new(&prefix))?)).await? {
        eprintln!("Prefix: {:?}", file);
    }
    for file in remote_storage.list_files(Some(&RemotePath::new(Path::new(&prefix))?)).await? {
        eprintln!("File: {:?}", file);
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Document {
    name: String,
    count: u32,
}

#[tokio::test]
async fn azure_upload() -> anyhow::Result<()> {
    let remote_storage = get_remote_storage()?;
    let path = RemotePath::new(Path::new("tests/azure_upload/document.txt"))?;
    let document = Document { name: String::from("Doc1"), count: 42 };
    let document_bytes = serde_json::to_vec(&document)?;
    let document_length = document_bytes.len();
    let document_cursor = std::io::Cursor::new(document_bytes);
    remote_storage.upload(document_cursor, document_length, &path, None).await?;
    Ok(())
}

#[tokio::test]
async fn azure_download() -> anyhow::Result<()> {
    let remote_storage = get_remote_storage()?;
    let path = RemotePath::new(Path::new(&std::env::var("AZURE_PATH")?))?;
    let mut download = remote_storage.download(&path).await?;
    let mut output = tokio::fs::File::create("/tmp/test.txt").await?;
    tokio::io::copy(&mut download.download_stream, &mut output).await?;
    Ok(())
}