use std::{sync::{Arc, Mutex}, ops::DerefMut, task::Poll};

use anyhow::{bail, anyhow};
use azure_storage_blobs::{prelude::{BlobClient, ContainerClient}, blob::operations::{GetBlobResponse, GetBlobBuilder}};
use futures_io::SeekFrom;
use tokio::{io::ReadBuf, pin};
use tokio_stream::StreamExt;
use tokio_util::io::StreamReader;
use std::iter::Iterator;
use tracing::debug;

use crate::{RemoteStorage, RemotePath, DownloadError, StorageMetadata, Download};


#[derive(Clone)]
pub struct AzureStorage {
    container_client: ContainerClient,
}

#[cfg(feature = "azure")]
impl AzureStorage {
    pub fn new() -> anyhow::Result<Self> {
        debug!("Creating azure remote storage");

        bail!("fail blog")
    }

    pub fn get_client(&self, path: &RemotePath) -> BlobClient {
        let blob_name = path.get_path().to_string_lossy();
        let client =
            self.container_client
            .blob_client(blob_name);
        client
    }

    async fn download_stream(
        &self,
        blob: GetBlobBuilder,
    ) -> Result<Download, DownloadError> {
        let stream = blob.into_stream();
        let stream = get_blob_bytes(stream);
        let mut stream = Box::pin(StreamReader::new(stream));
        let (mut write_stream, read_stream) = tokio::io::duplex(64 * 1024);

        // TODO: Find a way to wrap AsyncRead+Send as AsyncRead+Send+Sync without these copies
        // - does Download::download_stream have to be Sync, could this be removed from requirement on neon side?
        // - could Pageable<GetBlobResponse> implement Sync?
        tokio::spawn(async move {
            match tokio::io::copy(&mut stream, &mut write_stream).await {
                Ok(_amount) => (),
                Err(e) => {
                    eprintln!("Copy error: {}", e);
                    // expect writeStream to drop
                }
            }
        });
        Ok(Download {
            download_stream: Box::pin(read_stream),
            metadata: None
        })
    }
}

enum StreamWrapperState {
    Start, NeedsReset, Resetting
}

#[pin_project::pin_project]
struct StreamWrapper<S> where S : tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send + Sync {
    state: StreamWrapperState,
    len: usize,
    #[pin]
    stream: Arc<Mutex<S>>
}

impl<S> StreamWrapper<S> where S : tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send + Sync {
    pub fn new(stream: S, len: usize) -> Self {
        Self {
            state: StreamWrapperState::Start,
            len: len,
            stream: Arc::new(Mutex::new(stream))
        }
    }
}

impl<S> std::fmt::Debug for StreamWrapper<S> where S : tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send + Sync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StreamWrapper")
    }
}

impl<S> Clone for StreamWrapper<S> where S : tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send + Sync {
    fn clone(&self) -> Self {
        Self {
            state: StreamWrapperState::NeedsReset,
            len: self.len,
            stream: self.stream.clone()
        }
    }
}

impl<S> futures_io::AsyncRead for StreamWrapper<S> where S : tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send + Sync {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<futures_io::Result<usize>> {
        let this = self.project();
        let mut read_buf = ReadBuf::new(buf);
        let mut stream = this.stream.lock().unwrap();
        let stream = stream.deref_mut();
        let stream = std::pin::Pin::new(stream);
        match this.state {
            StreamWrapperState::Start =>
                match tokio::io::AsyncRead::poll_read(stream, cx, &mut read_buf) {
                    Poll::Ready(_) => Poll::Ready(Ok(read_buf.filled().len())),
                    Poll::Pending => Poll::Pending,
                },
            StreamWrapperState::NeedsReset => {
                match tokio::io::AsyncSeek::start_seek(stream, SeekFrom::Start(0)) {
                    Ok(()) => {
                        *this.state = StreamWrapperState::Resetting;
                        Poll::Pending
                    },
                    Err(e) => Poll::Ready(Err(futures_io::Error::new(std::io::ErrorKind::Other, e)))
                }
            },
            StreamWrapperState::Resetting => {
                if let Poll::Ready(_) = tokio::io::AsyncSeek::poll_complete(stream, cx) {
                    *this.state = StreamWrapperState::Start;
                }
                Poll::Pending
            }
        }
    }
}

#[async_trait::async_trait]
impl<S> azure_core::SeekableStream for StreamWrapper<S> where S : tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send + Sync {
    async fn reset(&mut self) -> azure_core::error::Result<()> {
        self.state = StreamWrapperState::NeedsReset;
        Ok(())
    }

    fn len(&self) -> usize {
        self.len
    }
}

fn get_blob_bytes<S : tokio_stream::Stream<Item = Result<GetBlobResponse, azure_core::Error>>>(input : S) -> impl tokio_stream::Stream<Item = Result<bytes::Bytes, std::io::Error>> {
    async_stream::stream! {
        for await value in input {
            yield match value {
                Ok(blob) => match blob.data.collect().await {
                    Ok(bytes) => Ok(bytes),
                    Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e))
                },
                Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e))
            }
        }
    }
}

#[async_trait::async_trait]
impl RemoteStorage for AzureStorage {
    /// See the doc for `RemoteStorage::list_prefixes`
    /// Note: it wont include empty "directories"
    async fn list_prefixes(
        &self,
        prefix: Option<&RemotePath>,
    ) -> Result<Vec<RemotePath>, DownloadError> {
        let mut blobs =
            self.container_client
                .list_blobs()
                .delimiter("/");
        if let Some(prefix) = prefix {
            blobs = blobs.prefix(prefix.to_string());
        }
        let mut blob_stream = blobs.into_stream();
        
        let mut results: Vec<RemotePath> = Vec::new();
        while let Some(item) = blob_stream.next().await {
            if let Ok(list) = item {
                for blob_prefix in list.blobs.prefixes() {
                    results.push(RemotePath::from_string(&blob_prefix.name).map_err(DownloadError::Other)?);
                }
            }
        }

        Ok(results)
    }

    /// See the doc for `RemoteStorage::list_files`
    async fn list_files(&self, folder: Option<&RemotePath>) -> anyhow::Result<Vec<RemotePath>> {
        let mut blobs = self.container_client.list_blobs();
        if let Some(prefix) = folder {
            blobs = blobs.prefix(prefix.to_string());
        }
        let mut blob_stream = blobs.into_stream();
        
        let mut results: Vec<RemotePath> = Vec::new();
        while let Some(item) = blob_stream.next().await {
            if let Ok(list) = item {
                for blob in list.blobs.blobs() {
                    results.push(RemotePath::from_string(&blob.name)?);
                }
            }
        }

        Ok(results)
    }

    async fn upload(
        &self,
        from: impl tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send + Sync + 'static,
        from_size_bytes: usize,
        to: &RemotePath,
        _metadata: Option<StorageMetadata>,
    ) -> anyhow::Result<()> {
        let blob_name = to.to_string();
        let client = self.container_client.blob_client(blob_name);
        let body = azure_core::Body::SeekableStream(Box::new(StreamWrapper::new(from, from_size_bytes)));
        match client.put_block_blob(body).await {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e))
        }
    }

    async fn download(&self, from: &RemotePath) -> Result<Download, DownloadError> {
        let blob_name = from.to_string();
        let client = self.container_client.blob_client(blob_name);
        let blob = client.get();
        self.download_stream(blob).await
    }

    async fn download_byte_range(
        &self,
        from: &RemotePath,
        start_inclusive: u64,
        end_exclusive: Option<u64>,
    ) -> Result<Download, DownloadError> {
        let blob_name = from.to_string();
        let client = self.container_client.blob_client(blob_name);
        let blob = client.get();
        let end = match end_exclusive {
            Some(value) => value,
            None => client.get_properties().await.map_err(|e| DownloadError::Other(anyhow!(e)))?.blob.properties.content_length,
        };
        let blob = blob.range(azure_core::request_options::Range { start: start_inclusive, end: end });
        self.download_stream(blob).await
    }
    
    async fn delete_objects<'a>(&self, paths: &'a [RemotePath]) -> anyhow::Result<()> {
        for path in paths {
            self.delete(path).await?;
        }
        Ok(())
    }

    async fn delete(&self, path: &RemotePath) -> anyhow::Result<()> {
        let blob_name = path.to_string();
        let client = self.container_client.blob_client(blob_name);
        client.delete().await?;
        Ok(())
    }
}
