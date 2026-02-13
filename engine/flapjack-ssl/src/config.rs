use crate::error::{FlapjackError, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::net::IpAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig {
    pub public_ip: IpAddr,
    pub email: String,
    pub acme_directory: String,
    pub check_interval_secs: u64,
    pub renew_days_threshold: u64,
}

impl SslConfig {
    /// Load SSL configuration from environment variables.
    /// Always enabled (opinionated approach).
    ///
    /// Required: FLAPJACK_SSL_EMAIL
    /// Optional: FLAPJACK_PUBLIC_IP (auto-detects if not set)
    /// Optional: FLAPJACK_ACME_DIRECTORY (defaults to Let's Encrypt production)
    pub fn from_env() -> Result<Self> {
        let email = env::var("FLAPJACK_SSL_EMAIL").map_err(|_| {
            FlapjackError::Config("FLAPJACK_SSL_EMAIL is required for SSL auto-renewal".into())
        })?;

        let public_ip = match env::var("FLAPJACK_PUBLIC_IP") {
            Ok(ip_str) => ip_str.parse().map_err(|_| {
                FlapjackError::Config(format!("Invalid FLAPJACK_PUBLIC_IP: {}", ip_str))
            })?,
            Err(_) => Self::detect_public_ip()?,
        };

        let acme_directory = env::var("FLAPJACK_ACME_DIRECTORY")
            .unwrap_or_else(|_| "https://acme-v02.api.letsencrypt.org/directory".into());

        // Validate ACME directory URL is HTTPS (security requirement)
        if !acme_directory.starts_with("https://") {
            return Err(FlapjackError::Config(format!(
                "ACME directory must use HTTPS, got: {}",
                acme_directory
            )));
        }

        Ok(Self {
            public_ip,
            email,
            acme_directory,
            check_interval_secs: 86400, // 24 hours (opinionated, not configurable)
            renew_days_threshold: 3,    // 3 days (opinionated, not configurable)
        })
    }

    /// Auto-detect public IP address
    /// Try EC2 metadata first, then fallback to external service
    /// Note: This is a simple fallback - if detection fails, user must set FLAPJACK_PUBLIC_IP
    fn detect_public_ip() -> Result<IpAddr> {
        // For now, require explicit IP configuration
        // TODO: Use async IP detection during server startup instead
        Err(FlapjackError::Config(
            "Could not auto-detect public IP. Please set FLAPJACK_PUBLIC_IP environment variable."
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        env::set_var("FLAPJACK_SSL_EMAIL", "test@example.com");
        env::set_var("FLAPJACK_PUBLIC_IP", "127.0.0.1");

        let config = SslConfig::from_env().unwrap();
        assert_eq!(config.email, "test@example.com");
        assert_eq!(config.public_ip.to_string(), "127.0.0.1");
        assert_eq!(config.check_interval_secs, 86400);
        assert_eq!(config.renew_days_threshold, 3);

        env::remove_var("FLAPJACK_SSL_EMAIL");
        env::remove_var("FLAPJACK_PUBLIC_IP");
    }

    #[test]
    fn test_config_requires_email() {
        env::remove_var("FLAPJACK_SSL_EMAIL");
        env::set_var("FLAPJACK_PUBLIC_IP", "127.0.0.1");

        let result = SslConfig::from_env();
        assert!(result.is_err());

        env::remove_var("FLAPJACK_PUBLIC_IP");
    }
}
