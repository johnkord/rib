use async_trait::async_trait;
use thiserror::Error;
use std::sync::Arc;

#[derive(Debug, Error)]
pub enum ImageStoreError {
    #[error("duplicate")] Duplicate,
    #[error("not_found")] NotFound,
    #[error("other: {0}")] Other(String),
}

#[async_trait]
pub trait ImageStore: Send + Sync {
    async fn save(&self, hash: &str, mime: &str, bytes: &[u8]) -> Result<(), ImageStoreError>;
    async fn load(&self, hash: &str) -> Result<(Vec<u8>, String), ImageStoreError>;
}

// ---------------- Filesystem Implementation ----------------
pub struct FsImageStore {
    base: String,
}

impl FsImageStore {
    pub fn new() -> Self {
        let base = std::env::var("RIB_DATA_DIR").unwrap_or_else(|_| "data".into());
        Self { base }
    }
    fn path_for(&self, hash: &str) -> (String, String) {
        let dir = format!("{}/images/{}", self.base, &hash[0..2]);
        let full = format!("{}/{}", dir, hash);
        (dir, full)
    }
}

#[async_trait]
impl ImageStore for FsImageStore {
    async fn save(&self, hash: &str, _mime: &str, bytes: &[u8]) -> Result<(), ImageStoreError> {
        use std::io::Write;
        let (dir, full) = self.path_for(hash);
        std::fs::create_dir_all(&dir).map_err(|e| ImageStoreError::Other(e.to_string()))?;
        let mut f = match std::fs::OpenOptions::new().write(true).create_new(true).open(&full) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(ImageStoreError::Duplicate),
            Err(e) => return Err(ImageStoreError::Other(e.to_string())),
        };
        f.write_all(bytes).map_err(|e| ImageStoreError::Other(e.to_string()))?;
        Ok(())
    }
    async fn load(&self, hash: &str) -> Result<(Vec<u8>, String), ImageStoreError> {
        let (_, full) = self.path_for(hash);
        let bytes = std::fs::read(&full).map_err(|_| ImageStoreError::NotFound)?;
        let mime = infer::get(&bytes).map(|t| t.mime_type().to_string()).unwrap_or_else(|| "application/octet-stream".into());
        Ok((bytes, mime))
    }
}

// ---------------- S3 Implementation (MinIO compatible) ----------------
#[cfg(feature = "s3-image-store")]
pub struct S3ImageStore {
    bucket: String,
    client: aws_sdk_s3::Client,
    prefix: String,
}

#[cfg(feature = "s3-image-store")]
impl S3ImageStore {
    pub async fn new() -> anyhow::Result<Self> {
        let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "rib-images".into());
        let endpoint = std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".into());
        let region = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".into());
        let mut loader = aws_config::from_env().region(aws_sdk_s3::config::Region::new(region));
        // custom endpoint (MinIO)
        loader = loader.endpoint_url(endpoint);
        let conf = loader.load().await;
        let client = aws_sdk_s3::Client::new(&conf);
        Ok(Self { bucket, client, prefix: "images".into() })
    }
    fn key_for(&self, hash: &str) -> String { format!("{}/{}/{}", self.prefix, &hash[0..2], hash) }
}

#[cfg(feature = "s3-image-store")]
#[async_trait]
impl ImageStore for S3ImageStore {
    async fn save(&self, hash: &str, _mime: &str, bytes: &[u8]) -> Result<(), ImageStoreError> {
        use aws_sdk_s3::primitives::ByteStream;
        let key = self.key_for(hash);
        // Attempt HEAD to detect duplicate
        if self.client.head_object().bucket(&self.bucket).key(&key).send().await.is_ok() {
            return Err(ImageStoreError::Duplicate);
        }
        self.client.put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(bytes.to_vec()))
            .send().await.map_err(|e| ImageStoreError::Other(e.to_string()))?;
        Ok(())
    }
    async fn load(&self, hash: &str) -> Result<(Vec<u8>, String), ImageStoreError> {
        let key = self.key_for(hash);
        let obj = self.client.get_object().bucket(&self.bucket).key(&key).send().await
            .map_err(|_| ImageStoreError::NotFound)?;
        let data = obj.body.collect().await.map_err(|e| ImageStoreError::Other(e.to_string()))?;
        // ContentType may be None; fallback by sniffing
        let mut bytes = Vec::from(data.into_bytes().as_ref());
        let mime = infer::get(&bytes).map(|t| t.mime_type().to_string())
            .unwrap_or_else(|| "application/octet-stream".into());
        Ok((bytes, mime))
    }
}

// Factory helper used in main
pub async fn build_image_store() -> Arc<dyn ImageStore> {
    #[cfg(feature = "s3-image-store")]
    if std::env::var("S3_ENDPOINT").is_ok() {
        if let Ok(store) = S3ImageStore::new().await {
            return Arc::new(store);
        }
    }
    Arc::new(FsImageStore::new())
}
