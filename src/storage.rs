use async_trait::async_trait;
use log::{error, info, warn};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImageStoreError {
    #[error("duplicate")]
    Duplicate,
    #[error("not_found")]
    NotFound,
    #[error("other: {0}")]
    Other(String),
}

#[async_trait]
pub trait ImageStore: Send + Sync {
    async fn save(&self, hash: &str, mime: &str, bytes: &[u8]) -> Result<(), ImageStoreError>;
    async fn load(&self, hash: &str) -> Result<(Vec<u8>, String), ImageStoreError>;
    async fn delete(&self, hash: &str) -> Result<(), ImageStoreError>;
}

// ---------------- S3 Implementation (MinIO compatible; ONLY supported backend) ----------------
pub struct S3ImageStore {
    bucket: String,
    client: aws_sdk_s3::Client,
    prefix: String,
}

impl S3ImageStore {
    pub async fn new() -> anyhow::Result<Self> {
        use aws_credential_types::provider::SharedCredentialsProvider;
        use aws_credential_types::Credentials;

        let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "rib-images".into());
        let endpoint = std::env::var("S3_ENDPOINT")
            .map_err(|_| anyhow::anyhow!("S3_ENDPOINT must be set (MinIO / S3 endpoint)"))?;
        let region = std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".into());
        let region_clone_for_hint = region.clone();
        let access = std::env::var("S3_ACCESS_KEY").unwrap_or_default();
        let secret = std::env::var("S3_SECRET_KEY").unwrap_or_default();

        // Use new defaults builder (avoids deprecation warning from from_env)
        let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new(region));
        loader = loader.endpoint_url(endpoint);
        if !access.is_empty() && !secret.is_empty() {
            let creds = Credentials::new(access, secret, None, None, "static");
            loader = loader.credentials_provider(SharedCredentialsProvider::new(creds));
        }
        let conf = loader.load().await;
        // Force path-style addressing (required for most MinIO/local endpoints without wildcard DNS)
        let s3_conf = aws_sdk_s3::config::Builder::from(&conf)
            .force_path_style(true)
            .build();
        let client = aws_sdk_s3::Client::from_conf(s3_conf);
        info!("Initialized S3/MinIO client (path-style addressing enabled)");

        // Ensure bucket exists (create if missing)
        if let Err(e) = client.head_bucket().bucket(&bucket).send().await {
            warn!("head_bucket failed for '{bucket}' (will attempt create): {e:?}");
            let mut attempt = 0u32;
            let max_attempts = 8;
            loop {
                attempt += 1;
                match client.create_bucket().bucket(&bucket).send().await {
                    Ok(_) => {
                        info!("created bucket '{bucket}' (attempt {attempt})");
                        break;
                    }
                    Err(e2) => {
                        if attempt >= max_attempts {
                            let region_hint = if region_clone_for_hint != "us-east-1" {
                                " (if this is not MinIO you may need a CreateBucketConfiguration for non-us-east-1 regions)"
                            } else {
                                ""
                            };
                            error!("create_bucket failed for '{bucket}' after {attempt} attempts: {e2:?}");
                            return Err(anyhow::anyhow!(
                                "failed to ensure bucket '{bucket}': {e2}{region_hint}"
                            ));
                        } else {
                            let backoff_ms = 200 * attempt.pow(2); // quadratic backoff
                            warn!("create_bucket attempt {attempt} failed for '{bucket}': {e2:?} (retrying in {backoff_ms}ms)");
                            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms as u64))
                                .await;
                        }
                    }
                }
            }
        }

        Ok(Self {
            bucket,
            client,
            prefix: "images".into(),
        })
    }
    fn key_for(&self, hash: &str) -> String {
        format!("{}/{}/{}", self.prefix, &hash[0..2], hash)
    }
}

#[async_trait]
impl ImageStore for S3ImageStore {
    async fn save(&self, hash: &str, _mime: &str, bytes: &[u8]) -> Result<(), ImageStoreError> {
        use aws_sdk_s3::primitives::ByteStream;
        let key = self.key_for(hash);
        // Attempt HEAD to detect duplicate
        if self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .is_ok()
        {
            return Err(ImageStoreError::Duplicate);
        }
        let put = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(bytes.to_vec()))
            // Best-effort content type detection (helps when serving directly from S3/MinIO)
            .content_type(
                infer::get(bytes)
                    .map(|t| t.mime_type().to_string())
                    .unwrap_or_else(|| "application/octet-stream".into()),
            );
        if let Err(e) = put.send().await {
            // Log full debug (including SDK classification) but return concise error upstream
            error!(
                "put_object failed hash={hash} key={key} bucket={} err={:?}",
                self.bucket, e
            );
            // Map common cases for nicer operator hints
            let hint = if e.to_string().contains("NoSuchBucket") {
                " (bucket missing or not yet propagated)"
            } else if e.to_string().contains("AccessDenied") {
                " (check S3_ACCESS_KEY/S3_SECRET_KEY permissions)"
            } else {
                ""
            };
            return Err(ImageStoreError::Other(format!("{}{}", e.to_string(), hint)));
        }
        Ok(())
    }
    async fn load(&self, hash: &str) -> Result<(Vec<u8>, String), ImageStoreError> {
        let key = self.key_for(hash);
        let obj = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|_| ImageStoreError::NotFound)?;
        let data = obj
            .body
            .collect()
            .await
            .map_err(|e| ImageStoreError::Other(e.to_string()))?;
        // ContentType may be None; fallback by sniffing
        let bytes = Vec::from(data.into_bytes().as_ref());
        let mime = infer::get(&bytes)
            .map(|t| t.mime_type().to_string())
            .unwrap_or_else(|| "application/octet-stream".into());
        Ok((bytes, mime))
    }
    async fn delete(&self, hash: &str) -> Result<(), ImageStoreError> {
        let key = self.key_for(hash);
        // Best-effort delete: treat not found as success
        let _ = self
            .client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await;
        Ok(())
    }
}

// Factory helper used in main (now S3-only; panic early if misconfigured)
pub async fn build_image_store() -> Arc<dyn ImageStore> {
    match S3ImageStore::new().await {
        Ok(store) => Arc::new(store),
        Err(e) => panic!("Failed to initialize S3 image store: {e}"),
    }
}

// (Re-export removed; tests use their own mock implementation.)
