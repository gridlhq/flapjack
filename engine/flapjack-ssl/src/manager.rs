use super::acme::AcmeClient;
use super::config::SslConfig;
use crate::error::{FlapjackError, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// SSL certificate manager with automatic renewal
pub struct SslManager {
    pub config: SslConfig,
    acme_client: Option<Arc<AcmeClient>>,
    last_check: Arc<RwLock<Option<DateTime<Utc>>>>,
    last_renewal: Arc<RwLock<Option<DateTime<Utc>>>>,
    renewal_status: Arc<RwLock<RenewalStatus>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenewalStatus {
    pub enabled: bool,
    pub status: String, // "ok", "renewing", "failed"
    pub error: Option<String>,
    pub cert_expires_in_days: Option<i64>,
    pub next_check: Option<DateTime<Utc>>,
}

impl Default for RenewalStatus {
    fn default() -> Self {
        Self {
            enabled: true,
            status: "initializing".to_string(),
            error: None,
            cert_expires_in_days: None,
            next_check: None,
        }
    }
}

impl SslManager {
    /// Create a new SSL manager (always auto-enabled)
    pub async fn new(config: SslConfig) -> Result<Arc<Self>> {
        tracing::info!(
            "[SSL] Initializing SSL manager for IP: {}",
            config.public_ip
        );

        let acme_client = Arc::new(AcmeClient::new(&config.email, &config.acme_directory).await?);

        Ok(Arc::new(Self {
            config,
            acme_client: Some(acme_client),
            last_check: Arc::new(RwLock::new(None)),
            last_renewal: Arc::new(RwLock::new(None)),
            renewal_status: Arc::new(RwLock::new(RenewalStatus::default())),
        }))
    }

    /// Start the certificate renewal loop (background task)
    /// Checks every 24 hours, renews at <3 days remaining
    pub async fn start_renewal_loop(self: Arc<Self>) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(
            self.config.check_interval_secs,
        ));

        // Skip the first immediate tick
        interval.tick().await;

        loop {
            interval.tick().await;
            tracing::info!("[SSL] Running certificate expiry check...");

            if let Err(e) = self.check_and_renew().await {
                tracing::error!("[SSL] Renewal check failed: {}", e);
                eprintln!("ALERT: SSL renewal check failed: {}", e);

                // Update status with error
                let mut status = self.renewal_status.write().await;
                status.status = "failed".to_string();
                status.error = Some(e.to_string());
            }

            // Update next check time
            let mut status = self.renewal_status.write().await;
            status.next_check =
                Some(Utc::now() + Duration::seconds(self.config.check_interval_secs as i64));
        }
    }

    /// Check certificate expiry and renew if needed
    async fn check_and_renew(&self) -> Result<()> {
        // Update last check time
        *self.last_check.write().await = Some(Utc::now());

        // Check if certificate exists and get expiry
        let cert_path = self.get_cert_path();

        if !cert_path.exists() {
            tracing::warn!(
                "[SSL] Certificate not found at {:?}, requesting new certificate",
                cert_path
            );
            return self.renew_certificate().await;
        }

        // Parse certificate and check expiry
        let days_remaining = self.get_cert_expiry_days(&cert_path)?;

        // Update status
        {
            let mut status = self.renewal_status.write().await;
            status.cert_expires_in_days = Some(days_remaining);
            status.status = "ok".to_string();
            status.error = None;
        }

        tracing::info!("[SSL] Certificate expires in {} days", days_remaining);

        if days_remaining < self.config.renew_days_threshold as i64 {
            tracing::warn!(
                "[SSL] Certificate expires in {} days (threshold: {}), renewing...",
                days_remaining,
                self.config.renew_days_threshold
            );
            return self.renew_certificate().await;
        }

        Ok(())
    }

    /// Renew the certificate using ACME
    async fn renew_certificate(&self) -> Result<()> {
        // Update status
        {
            let mut status = self.renewal_status.write().await;
            status.status = "renewing".to_string();
        }

        let acme_client = self
            .acme_client
            .as_ref()
            .ok_or_else(|| FlapjackError::Ssl("ACME client not initialized".to_string()))?;

        tracing::info!("[SSL] Requesting new certificate from Let's Encrypt...");

        // Request new certificate
        let (cert_pem, _key_pem) = acme_client
            .request_certificate(&self.config.public_ip.to_string())
            .await?;

        // Write certificate files to disk
        self.write_certificate_files(&cert_pem)?;

        // Reload nginx to pick up new certificate
        self.reload_nginx()?;

        // Update last renewal time
        *self.last_renewal.write().await = Some(Utc::now());

        // Update status
        {
            let mut status = self.renewal_status.write().await;
            status.status = "ok".to_string();
            status.error = None;
            status.cert_expires_in_days = Some(6); // IP certs are 6 days
        }

        tracing::info!("[SSL] Certificate renewed successfully!");

        Ok(())
    }

    /// Write certificate files to Let's Encrypt directory structure
    fn write_certificate_files(&self, cert_pem: &str) -> Result<()> {
        let cert_dir =
            PathBuf::from("/etc/letsencrypt/live").join(self.config.public_ip.to_string());

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&cert_dir)
            .map_err(|e| FlapjackError::Ssl(format!("Failed to create cert directory: {}", e)))?;

        // Write fullchain.pem
        let fullchain_path = cert_dir.join("fullchain.pem");
        std::fs::write(&fullchain_path, cert_pem)
            .map_err(|e| FlapjackError::Ssl(format!("Failed to write certificate: {}", e)))?;

        tracing::info!("[SSL] Certificate written to {:?}", fullchain_path);

        Ok(())
    }

    /// Reload nginx to pick up new certificate
    fn reload_nginx(&self) -> Result<()> {
        use std::process::Command;

        tracing::info!("[SSL] Reloading nginx...");

        let output = Command::new("systemctl")
            .args(["reload", "nginx"])
            .output()
            .map_err(|e| FlapjackError::Ssl(format!("Failed to execute nginx reload: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FlapjackError::Ssl(format!(
                "Nginx reload failed: {}",
                stderr
            )));
        }

        tracing::info!("[SSL] Nginx reloaded successfully");
        Ok(())
    }

    /// Get the path to the certificate file
    fn get_cert_path(&self) -> PathBuf {
        PathBuf::from("/etc/letsencrypt/live")
            .join(self.config.public_ip.to_string())
            .join("fullchain.pem")
    }

    /// Parse certificate and get days until expiry
    fn get_cert_expiry_days(&self, cert_path: &PathBuf) -> Result<i64> {
        use x509_parser::prelude::*;

        let cert_pem = std::fs::read(cert_path)
            .map_err(|e| FlapjackError::Ssl(format!("Failed to read certificate: {}", e)))?;

        let (_, pem) = parse_x509_pem(&cert_pem)
            .map_err(|e| FlapjackError::Ssl(format!("Failed to parse certificate PEM: {}", e)))?;

        let cert = pem
            .parse_x509()
            .map_err(|e| FlapjackError::Ssl(format!("Failed to parse X509 certificate: {}", e)))?;

        // Get expiry time
        let not_after = cert.validity().not_after;
        let expiry_timestamp = not_after.timestamp();

        // Calculate days remaining
        let now = Utc::now().timestamp();
        let seconds_remaining = expiry_timestamp - now;
        let days_remaining = seconds_remaining / 86400;

        Ok(days_remaining)
    }

    /// Get the ACME client (for HTTP challenge handler)
    pub fn get_acme_client(&self) -> Option<Arc<AcmeClient>> {
        self.acme_client.clone()
    }

    /// Get current renewal status (for /internal/status endpoint)
    pub async fn get_status(&self) -> RenewalStatus {
        self.renewal_status.read().await.clone()
    }
}
