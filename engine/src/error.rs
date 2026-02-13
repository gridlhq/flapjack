use http::StatusCode;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum FlapjackError {
    #[error("Tenant not found: {0}")]
    TenantNotFound(String),

    #[error("Index already exists for tenant: {0}")]
    IndexAlreadyExists(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("Query too complex: {0}")]
    QueryTooComplex(String),

    #[error("Invalid schema: {0}")]
    InvalidSchema(String),

    #[error("Invalid document: {0}")]
    InvalidDocument(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Type mismatch for field {field}: expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },

    #[error("Field not found in schema: {0}")]
    FieldNotFound(String),

    #[error("Too many concurrent writes: {current} active, max {max}")]
    TooManyConcurrentWrites { current: usize, max: usize },

    #[error("Buffer size {requested} exceeds max {max} bytes")]
    BufferSizeExceeded { requested: usize, max: usize },

    #[error("Document size {size} exceeds max {max} bytes")]
    DocumentTooLarge { size: usize, max: usize },

    #[error("Batch size {size} exceeds max {max} documents")]
    BatchTooLarge { size: usize, max: usize },

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Write queue full (1000 operations pending)")]
    QueueFull,

    #[error("IO error: {0}")]
    Io(String),

    #[error("Tantivy error: {0}")]
    Tantivy(String),

    #[error("Query parse error: {0}")]
    QueryParse(String),

    #[error("JSON error: {0}")]
    Json(String),

    #[error("S3 error: {0}")]
    S3(String),

    #[error("SSL error: {0}")]
    Ssl(String),

    #[error("ACME error: {0}")]
    Acme(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Memory pressure: {allocated_mb} MB allocated of {limit_mb} MB limit ({level})")]
    MemoryPressure {
        allocated_mb: usize,
        limit_mb: usize,
        level: String,
    },
}

pub type Result<T> = std::result::Result<T, FlapjackError>;

impl From<std::io::Error> for FlapjackError {
    fn from(e: std::io::Error) -> Self {
        FlapjackError::Io(e.to_string())
    }
}

impl From<tantivy::TantivyError> for FlapjackError {
    fn from(e: tantivy::TantivyError) -> Self {
        FlapjackError::Tantivy(e.to_string())
    }
}

impl From<tantivy::query::QueryParserError> for FlapjackError {
    fn from(e: tantivy::query::QueryParserError) -> Self {
        FlapjackError::QueryParse(e.to_string())
    }
}

impl From<serde_json::Error> for FlapjackError {
    fn from(e: serde_json::Error) -> Self {
        FlapjackError::Json(e.to_string())
    }
}

impl From<flapjack_ssl::FlapjackError> for FlapjackError {
    fn from(e: flapjack_ssl::FlapjackError) -> Self {
        // Map SSL crate errors to main crate errors
        match e {
            flapjack_ssl::FlapjackError::Config(msg) => FlapjackError::Config(msg),
            flapjack_ssl::FlapjackError::Ssl(msg) => FlapjackError::Ssl(msg),
            flapjack_ssl::FlapjackError::Acme(msg) => FlapjackError::Acme(msg),
            _ => FlapjackError::Ssl(e.to_string()),
        }
    }
}

impl FlapjackError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            FlapjackError::TenantNotFound(_) => StatusCode::NOT_FOUND,
            FlapjackError::IndexAlreadyExists(_) => StatusCode::CONFLICT,
            FlapjackError::InvalidQuery(_) => StatusCode::BAD_REQUEST,
            FlapjackError::QueryTooComplex(_) => StatusCode::BAD_REQUEST,
            FlapjackError::InvalidSchema(_) => StatusCode::BAD_REQUEST,
            FlapjackError::MissingField(_) => StatusCode::BAD_REQUEST,
            FlapjackError::TypeMismatch { .. } => StatusCode::BAD_REQUEST,
            FlapjackError::FieldNotFound(_) => StatusCode::BAD_REQUEST,
            FlapjackError::TooManyConcurrentWrites { .. } => StatusCode::SERVICE_UNAVAILABLE,
            FlapjackError::BufferSizeExceeded { .. } => StatusCode::BAD_REQUEST,
            FlapjackError::DocumentTooLarge { .. } => StatusCode::BAD_REQUEST,
            FlapjackError::BatchTooLarge { .. } => StatusCode::BAD_REQUEST,
            FlapjackError::TaskNotFound(_) => StatusCode::NOT_FOUND,
            FlapjackError::QueueFull => StatusCode::TOO_MANY_REQUESTS,
            FlapjackError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            FlapjackError::Tantivy(_) => StatusCode::INTERNAL_SERVER_ERROR,
            FlapjackError::QueryParse(_) => StatusCode::BAD_REQUEST,
            FlapjackError::Json(_) => StatusCode::BAD_REQUEST,
            FlapjackError::InvalidDocument(_) => StatusCode::BAD_REQUEST,
            FlapjackError::S3(_) => StatusCode::INTERNAL_SERVER_ERROR,
            FlapjackError::Ssl(_) => StatusCode::INTERNAL_SERVER_ERROR,
            FlapjackError::Acme(_) => StatusCode::INTERNAL_SERVER_ERROR,
            FlapjackError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            FlapjackError::MemoryPressure { .. } => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

// Axum IntoResponse implementation (feature-gated)
#[cfg(feature = "axum-support")]
use axum::response::{IntoResponse, Json, Response};
#[cfg(feature = "axum-support")]
use serde::Serialize;

#[cfg(feature = "axum-support")]
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
}

#[cfg(feature = "axum-support")]
impl IntoResponse for FlapjackError {
    fn into_response(self) -> Response {
        let (status, error_code, message, suggestion) = match &self {
            FlapjackError::TenantNotFound(tenant) => (
                StatusCode::NOT_FOUND,
                "index_not_found",
                format!("Index '{}' does not exist", tenant),
                Some("Create the index first with POST /indexes".to_string()),
            ),
            FlapjackError::IndexAlreadyExists(tenant) => (
                StatusCode::CONFLICT,
                "index_already_exists",
                format!("Index '{}' already exists", tenant),
                None,
            ),
            FlapjackError::InvalidQuery(msg) => {
                (StatusCode::BAD_REQUEST, "invalid_query", msg.clone(), None)
            }
            FlapjackError::QueryTooComplex(msg) => (
                StatusCode::BAD_REQUEST,
                "query_too_complex",
                msg.clone(),
                Some("Simplify your query or reduce filter complexity".to_string()),
            ),
            FlapjackError::InvalidSchema(msg) => {
                (StatusCode::BAD_REQUEST, "invalid_schema", msg.clone(), None)
            }
            FlapjackError::MissingField(field) => (
                StatusCode::BAD_REQUEST,
                "missing_field",
                format!("Required field '{}' is missing", field),
                None,
            ),
            FlapjackError::TypeMismatch {
                field,
                expected,
                actual,
            } => (
                StatusCode::BAD_REQUEST,
                "type_mismatch",
                format!("Field '{}' expected {}, got {}", field, expected, actual),
                None,
            ),
            FlapjackError::FieldNotFound(field) => (
                StatusCode::BAD_REQUEST,
                "field_not_found",
                format!("Field '{}' not found in schema", field),
                None,
            ),
            FlapjackError::TooManyConcurrentWrites { current, max } => (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_concurrent_writes",
                format!(
                    "Too many concurrent writes: {} active, max {}",
                    current, max
                ),
                Some("Retry after a short delay".to_string()),
            ),
            FlapjackError::BufferSizeExceeded { requested, max } => (
                StatusCode::BAD_REQUEST,
                "buffer_size_exceeded",
                format!("Buffer size {} exceeds max {} bytes", requested, max),
                None,
            ),
            FlapjackError::DocumentTooLarge { size, max } => (
                StatusCode::BAD_REQUEST,
                "document_too_large",
                format!("Document size {} exceeds max {} bytes", size, max),
                Some("Split document into smaller chunks".to_string()),
            ),
            FlapjackError::BatchTooLarge { size, max } => (
                StatusCode::BAD_REQUEST,
                "batch_too_large",
                format!("Batch size {} exceeds max {} documents", size, max),
                Some("Split batch into smaller chunks".to_string()),
            ),
            FlapjackError::TaskNotFound(task_id) => (
                StatusCode::NOT_FOUND,
                "task_not_found",
                format!("Task '{}' not found", task_id),
                Some("Task may have been evicted (max 1000 tasks per tenant)".to_string()),
            ),
            FlapjackError::QueueFull => (
                StatusCode::SERVICE_UNAVAILABLE,
                "queue_full",
                "Write queue full (1000 operations pending)".to_string(),
                Some("Retry after a short delay".to_string()),
            ),
            FlapjackError::Io(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                format!("IO error: {}", e),
                None,
            ),
            FlapjackError::Tantivy(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                format!("Internal error: {}", e),
                None,
            ),
            FlapjackError::QueryParse(e) => (
                StatusCode::BAD_REQUEST,
                "query_parse_error",
                format!("Query parse error: {}", e),
                None,
            ),
            FlapjackError::Json(e) => (
                StatusCode::BAD_REQUEST,
                "json_error",
                format!("JSON error: {}", e),
                None,
            ),
            FlapjackError::InvalidDocument(msg) => (
                StatusCode::BAD_REQUEST,
                "invalid_document",
                msg.clone(),
                Some("Check document structure and field types".to_string()),
            ),
            FlapjackError::S3(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "s3_error",
                format!("S3 error: {}", e),
                Some(
                    "Check FLAPJACK_S3_BUCKET, FLAPJACK_S3_REGION, and AWS credentials".to_string(),
                ),
            ),
            FlapjackError::Ssl(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "ssl_error",
                format!("SSL error: {}", e),
                None,
            ),
            FlapjackError::Acme(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "acme_error",
                format!("ACME error: {}", e),
                Some("Check FLAPJACK_SSL_EMAIL and ensure port 80 is accessible".to_string()),
            ),
            FlapjackError::Config(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "config_error",
                format!("Configuration error: {}", e),
                None,
            ),
            FlapjackError::MemoryPressure {
                allocated_mb,
                limit_mb,
                ref level,
            } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "memory_pressure",
                format!(
                    "Memory pressure: {} MB allocated of {} MB limit ({})",
                    allocated_mb, limit_mb, level
                ),
                Some("Retry after a short delay".to_string()),
            ),
        };

        let error_response = ErrorResponse {
            error: error_code.to_string(),
            message,
            request_id: format!("req_fj_{}", uuid::Uuid::new_v4()),
            suggestion,
            docs: Some(format!("https://flapjack.dev/docs/errors/{}", error_code)),
        };

        let mut response = (status, Json(error_response)).into_response();
        if matches!(&self, FlapjackError::MemoryPressure { .. }) {
            response
                .headers_mut()
                .insert("Retry-After", "5".parse().unwrap());
        }
        response
    }
}
