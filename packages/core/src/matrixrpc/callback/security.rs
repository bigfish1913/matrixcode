//! Security Validator
//!
//! Validates callback requests from external services.
//! Ensures request_id + service_id + token combination is valid.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::matrixrpc::ServiceId;

/// Security error types
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    /// Invalid token
    #[error("Invalid or expired token")]
    InvalidToken,

    /// Token expired
    #[error("Token expired at {0}")]
    TokenExpired(String),

    /// Service not authorized
    #[error("Service '{0}' is not authorized for this callback")]
    ServiceNotAuthorized(String),

    /// Request ID mismatch
    #[error("Request ID '{0}' does not match the token")]
    RequestIdMismatch(String),

    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// Token generation failed
    #[error("Token generation failed: {0}")]
    TokenGenerationFailed(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded for service '{0}'")]
    RateLimitExceeded(String),

    /// Internal error
    #[error("Internal security error: {0}")]
    Internal(String),
}

/// Token information
#[derive(Debug, Clone)]
pub struct TokenInfo {
    /// The token value
    pub token: String,

    /// Service ID that owns this token
    pub service_id: ServiceId,

    /// Request ID this token is associated with
    pub request_id: String,

    /// When the token was created
    pub created_at: Instant,

    /// When the token expires
    pub expires_at: Instant,

    /// Allowed callback types
    pub allowed_types: Vec<String>,

    /// Usage count
    pub usage_count: u32,

    /// Maximum allowed uses
    pub max_uses: u32,
}

impl TokenInfo {
    /// Create a new token info
    pub fn new(
        token: String,
        service_id: ServiceId,
        request_id: String,
        lifetime_secs: u64,
    ) -> Self {
        let now = Instant::now();
        Self {
            token,
            service_id,
            request_id,
            created_at: now,
            expires_at: now + Duration::from_secs(lifetime_secs),
            allowed_types: vec![
                "ai".to_string(), "tool".to_string(), "context".to_string(),
            ],
            usage_count: 0,
            max_uses: 10,
        }
    }

    /// Set allowed callback types
    pub fn with_allowed_types(mut self, types: Vec<String>) -> Self {
        self.allowed_types = types;
        self
    }

    /// Set maximum uses
    pub fn with_max_uses(mut self, max: u32) -> Self {
        self.max_uses = max;
        self
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }

    /// Check if token has remaining uses
    pub fn has_remaining_uses(&self) -> bool {
        self.usage_count < self.max_uses
    }

    /// Check if callback type is allowed
    pub fn is_type_allowed(&self, callback_type: &str) -> bool {
        self.allowed_types.contains(&callback_type.to_string())
    }

    /// Increment usage count
    pub fn increment_usage(&mut self) {
        self.usage_count += 1;
    }
}

/// Validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether validation passed
    pub is_valid: bool,

    /// The validated token info (if valid)
    pub token_info: Option<TokenInfo>,

    /// Error message (if invalid)
    pub error: Option<String>,
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn success(token_info: TokenInfo) -> Self {
        Self {
            is_valid: true,
            token_info: Some(token_info),
            error: None,
        }
    }

    /// Create a failed validation result
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            is_valid: false,
            token_info: None,
            error: Some(error.into()),
        }
    }
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Token lifetime in seconds
    pub token_lifetime_secs: u64,

    /// Maximum uses per token
    pub max_token_uses: u32,

    /// Maximum tokens per service
    pub max_tokens_per_service: u32,

    /// Rate limit: requests per minute per service
    pub rate_limit_per_minute: u32,

    /// Enable strict validation
    pub strict_validation: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            token_lifetime_secs: 300, // 5 minutes
            max_token_uses: 10,
            max_tokens_per_service: 100,
            rate_limit_per_minute: 60,
            strict_validation: true,
        }
    }
}

/// Rate limit tracker
#[derive(Debug, Clone)]
struct RateLimitEntry {
    /// Request timestamps
    timestamps: Vec<Instant>,
    /// Last cleanup time
    last_cleanup: Instant,
}

impl RateLimitEntry {
    fn new() -> Self {
        Self {
            timestamps: Vec::new(),
            last_cleanup: Instant::now(),
        }
    }

    fn add_request(&mut self) {
        self.timestamps.push(Instant::now());
        // Cleanup old entries every minute
        if self.last_cleanup.elapsed() > Duration::from_secs(60) {
            self.cleanup();
            self.last_cleanup = Instant::now();
        }
    }

    fn cleanup(&mut self) {
        let cutoff = Instant::now() - Duration::from_secs(60);
        self.timestamps.retain(|t| *t > cutoff);
    }

    fn count_last_minute(&self) -> u32 {
        let cutoff = Instant::now() - Duration::from_secs(60);
        self.timestamps.iter().filter(|t| **t > cutoff).count() as u32
    }
}

/// Security Validator
///
/// Validates callback requests from external services.
/// Manages token generation, validation, and rate limiting.
pub struct SecurityValidator {
    /// Configuration
    config: SecurityConfig,

    /// Active tokens
    tokens: Arc<RwLock<HashMap<String, TokenInfo>>>,

    /// Service to token mapping
    service_tokens: Arc<RwLock<HashMap<ServiceId, Vec<String>>>>,

    /// Rate limit tracking
    rate_limits: Arc<RwLock<HashMap<ServiceId, RateLimitEntry>>>,
}

impl SecurityValidator {
    /// Create a new security validator
    pub fn new() -> Self {
        Self::with_config(SecurityConfig::default())
    }

    /// Create a new security validator with configuration
    pub fn with_config(config: SecurityConfig) -> Self {
        Self {
            config,
            tokens: Arc::new(RwLock::new(HashMap::new())),
            service_tokens: Arc::new(RwLock::new(HashMap::new())),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate a new token for a callback request
    pub async fn generate_token(
        &self,
        service_id: ServiceId,
        request_id: String,
        allowed_types: Vec<String>,
    ) -> Result<String, SecurityError> {
        // Check if service has too many tokens
        {
            let service_tokens = self.service_tokens.read().await;
            if let Some(tokens) = service_tokens.get(&service_id) {
                if tokens.len() >= self.config.max_tokens_per_service as usize {
                    return Err(SecurityError::RateLimitExceeded(service_id.to_string()));
                }
            }
        }

        // Generate unique token
        let token = format!(
            "cb_{}_{}",
            uuid::Uuid::new_v4().to_string(),
            &request_id[..8.min(request_id.len())]
        );

        // Create token info
        let token_info = TokenInfo::new(
            token.clone(),
            service_id.clone(),
            request_id,
            self.config.token_lifetime_secs,
        )
        .with_allowed_types(allowed_types)
        .with_max_uses(self.config.max_token_uses);

        // Store token
        {
            let mut tokens = self.tokens.write().await;
            tokens.insert(token.clone(), token_info);
        }

        // Update service token mapping
        {
            let mut service_tokens = self.service_tokens.write().await;
            service_tokens
                .entry(service_id)
                .or_insert_with(Vec::new)
                .push(token.clone());
        }

        Ok(token)
    }

    /// Validate a callback request
    pub async fn validate(
        &self,
        token: &str,
        service_id: &ServiceId,
        request_id: &str,
        callback_type: &str,
    ) -> ValidationResult {
        // Check rate limit first
        {
            let mut rate_limits = self.rate_limits.write().await;
            let entry = rate_limits
                .entry(service_id.clone())
                .or_insert_with(RateLimitEntry::new);

            if entry.count_last_minute() >= self.config.rate_limit_per_minute {
                return ValidationResult::failure(SecurityError::RateLimitExceeded(
                    service_id.to_string(),
                ).to_string());
            }

            entry.add_request();
        }

        // Look up token
        let mut tokens = self.tokens.write().await;
        let token_info = match tokens.get_mut(token) {
            Some(info) => info,
            None => return ValidationResult::failure(SecurityError::InvalidToken.to_string()),
        };

        // Check expiration
        if token_info.is_expired() {
            tokens.remove(token);
            return ValidationResult::failure(
                SecurityError::TokenExpired("token has expired".to_string()).to_string(),
            );
        }

        // Check remaining uses
        if !token_info.has_remaining_uses() {
            return ValidationResult::failure("Token usage limit exceeded".to_string());
        }

        // Validate service ID
        if token_info.service_id != *service_id {
            return ValidationResult::failure(
                SecurityError::ServiceNotAuthorized(service_id.to_string()).to_string(),
            );
        }

        // Validate request ID
        if self.config.strict_validation && token_info.request_id != request_id {
            return ValidationResult::failure(
                SecurityError::RequestIdMismatch(request_id.to_string()).to_string(),
            );
        }

        // Validate callback type
        if !token_info.is_type_allowed(callback_type) {
            return ValidationResult::failure(format!(
                "Callback type '{}' is not allowed for this token",
                callback_type
            ));
        }

        // Increment usage
        token_info.increment_usage();

        ValidationResult::success(token_info.clone())
    }

    /// Invalidate a token
    pub async fn invalidate_token(&self, token: &str) -> Result<(), SecurityError> {
        let token_info = {
            let mut tokens = self.tokens.write().await;
            tokens.remove(token)
        };

        if let Some(info) = token_info {
            // Remove from service mapping
            let mut service_tokens = self.service_tokens.write().await;
            if let Some(tokens) = service_tokens.get_mut(&info.service_id) {
                tokens.retain(|t| t != token);
            }
        }

        Ok(())
    }

    /// Invalidate all tokens for a service
    pub async fn invalidate_service_tokens(&self, service_id: &ServiceId) {
        let tokens_to_remove = {
            let service_tokens = self.service_tokens.read().await;
            service_tokens.get(service_id).cloned().unwrap_or_default()
        };

        {
            let mut tokens = self.tokens.write().await;
            for token in &tokens_to_remove {
                tokens.remove(token);
            }
        }

        {
            let mut service_tokens = self.service_tokens.write().await;
            service_tokens.remove(service_id);
        }
    }

    /// Clean up expired tokens
    pub async fn cleanup_expired(&self) -> usize {
        let expired_tokens: Vec<String> = {
            let tokens = self.tokens.read().await;
            tokens
                .iter()
                .filter(|(_, info)| info.is_expired())
                .map(|(token, _)| token.clone())
                .collect()
        };

        let count = expired_tokens.len();

        for token in &expired_tokens {
            self.invalidate_token(token).await.ok();
        }

        count
    }

    /// Get active token count
    pub async fn token_count(&self) -> usize {
        self.tokens.read().await.len()
    }

    /// Get token info
    pub async fn get_token_info(&self, token: &str) -> Option<TokenInfo> {
        self.tokens.read().await.get(token).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_token() {
        let validator = SecurityValidator::new();
        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();

        let token = validator
            .generate_token(service_id.clone(), request_id.clone(), vec!["ai".to_string(), "tool".to_string()])
            .await
            .unwrap();

        assert!(token.starts_with("cb_"));
        assert!(validator.token_count().await == 1);
    }

    #[tokio::test]
    async fn test_validate_token() {
        let validator = SecurityValidator::new();
        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();

        let token = validator
            .generate_token(service_id.clone(), request_id.clone(), vec!["ai".to_string()])
            .await
            .unwrap();

        let result = validator
            .validate(&token, &service_id, &request_id, "ai")
            .await;

        assert!(result.is_valid);
        assert!(result.token_info.is_some());
    }

    #[tokio::test]
    async fn test_validate_invalid_token() {
        let validator = SecurityValidator::new();
        let service_id = ServiceId::new("test-service");

        let result = validator
            .validate("invalid_token", &service_id, "req-001", "ai")
            .await;

        assert!(!result.is_valid);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_validate_wrong_service() {
        let validator = SecurityValidator::new();
        let service_id1 = ServiceId::new("service1");
        let service_id2 = ServiceId::new("service2");
        let request_id = "req-001".to_string();

        let token = validator
            .generate_token(service_id1.clone(), request_id.clone(), vec!["ai".to_string()])
            .await
            .unwrap();

        let result = validator
            .validate(&token, &service_id2, &request_id, "ai")
            .await;

        assert!(!result.is_valid);
    }

    #[tokio::test]
    async fn test_validate_wrong_callback_type() {
        let validator = SecurityValidator::new();
        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();

        let token = validator
            .generate_token(service_id.clone(), request_id.clone(), vec!["ai".to_string()])
            .await
            .unwrap();

        let result = validator
            .validate(&token, &service_id, &request_id, "tool")
            .await;

        assert!(!result.is_valid);
    }

    #[tokio::test]
    async fn test_invalidate_token() {
        let validator = SecurityValidator::new();
        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();

        let token = validator
            .generate_token(service_id.clone(), request_id.clone(), vec!["ai".to_string()])
            .await
            .unwrap();

        validator.invalidate_token(&token).await.unwrap();
        assert!(validator.token_count().await == 0);
    }

    #[tokio::test]
    async fn test_token_usage_limit() {
        let config = SecurityConfig {
            max_token_uses: 2,
            ..Default::default()
        };
        let validator = SecurityValidator::with_config(config);
        let service_id = ServiceId::new("test-service");
        let request_id = "req-001".to_string();

        let token = validator
            .generate_token(service_id.clone(), request_id.clone(), vec!["ai".to_string()])
            .await
            .unwrap();

        // First use
        let result1 = validator.validate(&token, &service_id, &request_id, "ai").await;
        assert!(result1.is_valid);

        // Second use
        let result2 = validator.validate(&token, &service_id, &request_id, "ai").await;
        assert!(result2.is_valid);

        // Third use should fail
        let result3 = validator.validate(&token, &service_id, &request_id, "ai").await;
        assert!(!result3.is_valid);
    }
}