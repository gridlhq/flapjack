use crate::error::Result;
use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::Region;

#[derive(Clone)]
pub struct S3Config {
    pub bucket_name: String,
    pub region: String,
    pub endpoint: Option<String>,
}

impl S3Config {
    pub fn from_env() -> Option<Self> {
        let bucket_name = std::env::var("FLAPJACK_S3_BUCKET").ok()?;
        let region = std::env::var("FLAPJACK_S3_REGION").unwrap_or_else(|_| "us-east-1".into());
        let endpoint = std::env::var("FLAPJACK_S3_ENDPOINT").ok();
        Some(Self {
            bucket_name,
            region,
            endpoint,
        })
    }

    pub fn bucket_internal(&self) -> Result<Box<Bucket>> {
        let region = match &self.endpoint {
            Some(ep) => Region::Custom {
                region: self.region.clone(),
                endpoint: ep.clone(),
            },
            None => self
                .region
                .parse()
                .map_err(|e| crate::error::FlapjackError::S3(format!("Invalid region: {}", e)))?,
        };
        let creds = Credentials::default()
            .map_err(|e| crate::error::FlapjackError::S3(format!("S3 credentials: {}", e)))?;
        let bucket = Bucket::new(&self.bucket_name, region, creds)
            .map_err(|e| crate::error::FlapjackError::S3(format!("S3 bucket: {}", e)))?;
        Ok(bucket)
    }
}

pub async fn upload_snapshot(config: &S3Config, index_name: &str, data: &[u8]) -> Result<String> {
    let bucket = config.bucket_internal()?;
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let key = format!("snapshots/{}/{}.tar.gz", index_name, timestamp);

    bucket
        .put_object(&key, data)
        .await
        .map_err(|e| crate::error::FlapjackError::S3(format!("S3 upload: {}", e)))?;

    tracing::info!("Uploaded snapshot s3://{}/{}", config.bucket_name, key);
    Ok(key)
}

pub async fn download_snapshot(config: &S3Config, key: &str) -> Result<Vec<u8>> {
    let bucket = config.bucket_internal()?;
    let response = bucket
        .get_object(key)
        .await
        .map_err(|e| crate::error::FlapjackError::S3(format!("S3 download: {}", e)))?;
    if response.status_code() != 200 {
        return Err(crate::error::FlapjackError::S3(format!(
            "S3 download failed: HTTP {}",
            response.status_code()
        )));
    }
    Ok(response.to_vec())
}

pub async fn download_latest_snapshot(
    config: &S3Config,
    index_name: &str,
) -> Result<(String, Vec<u8>)> {
    let keys = list_snapshots(config, index_name).await?;
    let latest = keys.last().ok_or_else(|| {
        crate::error::FlapjackError::S3(format!("No snapshots found for {}", index_name))
    })?;
    let data = download_snapshot(config, latest).await?;
    Ok((latest.clone(), data))
}

pub async fn list_snapshots(config: &S3Config, index_name: &str) -> Result<Vec<String>> {
    let bucket = config.bucket_internal()?;
    let prefix = format!("snapshots/{}/", index_name);
    let results = bucket
        .list(prefix, None)
        .await
        .map_err(|e| crate::error::FlapjackError::S3(format!("S3 list: {}", e)))?;
    let mut keys: Vec<String> = results
        .into_iter()
        .flat_map(|r| r.contents)
        .map(|obj| obj.key)
        .collect();
    keys.sort();
    Ok(keys)
}

pub async fn delete_snapshot(config: &S3Config, key: &str) -> Result<()> {
    let bucket = config.bucket_internal()?;
    bucket
        .delete_object(key)
        .await
        .map_err(|e| crate::error::FlapjackError::S3(format!("S3 delete: {}", e)))?;
    Ok(())
}

pub async fn enforce_retention(config: &S3Config, index_name: &str, keep: usize) -> Result<usize> {
    let keys = list_snapshots(config, index_name).await?;
    if keys.len() <= keep {
        return Ok(0);
    }
    let to_delete = &keys[..keys.len() - keep];
    for key in to_delete {
        delete_snapshot(config, key).await?;
        tracing::info!("Deleted old snapshot: {}", key);
    }
    Ok(to_delete.len())
}
