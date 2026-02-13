use crate::error::{FlapjackError, Result};
use dashmap::DashMap;
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt, NewAccount, NewOrder,
    RetryPolicy,
};
use std::net::IpAddr;
use std::sync::Arc;

/// ACME client for handling Let's Encrypt certificate operations
pub struct AcmeClient {
    /// ACME account (persisted)
    account: Arc<Account>,
    /// Challenge responses (token -> key_authorization)
    /// Stored in-memory during http-01 validation
    challenges: Arc<DashMap<String, String>>,
    /// ACME directory URL
    #[allow(dead_code)]
    acme_directory: String,
}

impl AcmeClient {
    /// Create a new ACME client or load existing account
    pub async fn new(email: &str, acme_directory: &str) -> Result<Self> {
        tracing::info!("[SSL] Initializing ACME client for {}", email);

        // Determine if using staging or production
        let directory_url = if acme_directory.contains("staging") {
            LetsEncrypt::Staging.url().to_owned()
        } else {
            LetsEncrypt::Production.url().to_owned()
        };

        // Create a new account
        let (account, _credentials) = Account::builder()?
            .create(
                &NewAccount {
                    contact: &[&format!("mailto:{}", email)],
                    terms_of_service_agreed: true,
                    only_return_existing: false,
                },
                directory_url.clone(),
                None,
            )
            .await
            .map_err(|e| FlapjackError::Acme(format!("Failed to create ACME account: {}", e)))?;

        tracing::info!("[SSL] ACME account created successfully");

        Ok(Self {
            account: Arc::new(account),
            challenges: Arc::new(DashMap::new()),
            acme_directory: directory_url,
        })
    }

    /// Request a new certificate for the given IP address
    /// Returns (certificate_pem, private_key_pem)
    pub async fn request_certificate(&self, ip: &str) -> Result<(String, String)> {
        tracing::info!("[SSL] Requesting certificate for IP: {}", ip);

        // Parse IP address
        let ip_addr: IpAddr = ip
            .parse()
            .map_err(|e| FlapjackError::Acme(format!("Invalid IP address: {}", e)))?;

        // Create a new order for this IP address
        let identifier = Identifier::Ip(ip_addr);

        let mut order = self
            .account
            .new_order(&NewOrder::new(&[identifier]))
            .await
            .map_err(|e| FlapjackError::Acme(format!("Failed to create ACME order: {}", e)))?;

        tracing::info!("[SSL] ACME order created");

        // Track tokens for this specific order (for cleanup)
        let mut order_tokens = Vec::new();

        // Get authorizations
        let mut authorizations = order.authorizations();

        while let Some(authz_result) = authorizations.next().await {
            let mut authz = authz_result
                .map_err(|e| FlapjackError::Acme(format!("Failed to get authorization: {}", e)))?;

            // Skip already-valid authorizations
            if matches!(authz.status, AuthorizationStatus::Valid) {
                continue;
            }

            // Find the http-01 challenge
            let mut challenge = authz
                .challenge(ChallengeType::Http01)
                .ok_or_else(|| FlapjackError::Acme("No http-01 challenge found".to_string()))?;

            // Store the challenge response in memory
            let token = challenge.token.clone();
            let key_authorization = challenge.key_authorization().as_str().to_string();
            self.challenges.insert(token.clone(), key_authorization);
            order_tokens.push(token.clone());

            tracing::info!("[SSL] Stored http-01 challenge token: {}", token);

            // Signal that we're ready for validation
            challenge.set_ready().await.map_err(|e| {
                FlapjackError::Acme(format!("Failed to set challenge ready: {}", e))
            })?;

            tracing::info!("[SSL] Challenge marked as ready, waiting for validation...");
        }

        // Poll until the order is ready
        tracing::info!("[SSL] Polling for order ready status...");
        let poll_result = order.poll_ready(&RetryPolicy::default()).await;

        // Clean up only this order's challenges (not all challenges globally)
        for token in &order_tokens {
            self.challenges.remove(token);
        }
        tracing::debug!("[SSL] Cleaned up {} challenge tokens", order_tokens.len());

        // Check poll result after cleanup
        let _status = poll_result
            .map_err(|e| FlapjackError::Acme(format!("Failed to poll order ready: {}", e)))?;

        // Finalize the order (generates CSR internally and returns private key)
        tracing::info!("[SSL] Finalizing order...");
        let private_key_pem = order
            .finalize()
            .await
            .map_err(|e| FlapjackError::Acme(format!("Failed to finalize order: {}", e)))?;

        // Poll for the certificate
        tracing::info!("[SSL] Polling for certificate...");
        let cert_chain_pem = order
            .poll_certificate(&RetryPolicy::default())
            .await
            .map_err(|e| FlapjackError::Acme(format!("Failed to poll certificate: {}", e)))?;

        tracing::info!("[SSL] Certificate issued successfully");

        Ok((cert_chain_pem, private_key_pem))
    }

    /// Get the challenge response for a given token (used by HTTP handler)
    pub fn get_challenge_response(&self, token: &str) -> Option<String> {
        self.challenges.get(token).map(|v| v.clone())
    }
}
