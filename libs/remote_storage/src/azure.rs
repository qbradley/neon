use anyhow::bail;

use crate::{RemoteStorage, RemotePath, DownloadError, StorageMetadata, Download};


#[cfg(feature = "azure")]
#[derive(Clone)]
pub struct AzureStorage {
    
}

#[async_trait::async_trait]
#[cfg(feature = "azure")]
impl RemoteStorage for AzureStorage {
    /// See the doc for `RemoteStorage::list_prefixes`
    /// Note: it wont include empty "directories"
    async fn list_prefixes(
        &self,
        prefix: Option<&RemotePath>,
    ) -> Result<Vec<RemotePath>, DownloadError> {
        Err(DownloadError::NotFound)
    }

    /// See the doc for `RemoteStorage::list_files`
    async fn list_files(&self, folder: Option<&RemotePath>) -> anyhow::Result<Vec<RemotePath>> {
        bail!("fail blog")
    }

    async fn upload(
        &self,
        from: impl tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send + Sync + 'static,
        from_size_bytes: usize,
        to: &RemotePath,
        metadata: Option<StorageMetadata>,
    ) -> anyhow::Result<()> {
        bail!("fail blog")
    }

    async fn download(&self, from: &RemotePath) -> Result<Download, DownloadError> {
        Err(DownloadError::NotFound)
    }

    async fn download_byte_range(
        &self,
        from: &RemotePath,
        start_inclusive: u64,
        end_exclusive: Option<u64>,
    ) -> Result<Download, DownloadError> {
        Err(DownloadError::NotFound)
    }
    async fn delete_objects<'a>(&self, paths: &'a [RemotePath]) -> anyhow::Result<()> {
        Ok(())
    }

    async fn delete(&self, path: &RemotePath) -> anyhow::Result<()> {
        bail!("fail blog")
    }
}
