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

    #[error("Index paused for migration: {0}")]
    IndexPaused(String),
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
            FlapjackError::IndexPaused(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── status_code mapping ─────────────────────────────────────────────

    #[test]
    fn tenant_not_found_is_404() {
        let e = FlapjackError::TenantNotFound("test".into());
        assert_eq!(e.status_code(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn index_already_exists_is_409() {
        let e = FlapjackError::IndexAlreadyExists("test".into());
        assert_eq!(e.status_code(), StatusCode::CONFLICT);
    }

    #[test]
    fn invalid_query_is_400() {
        let e = FlapjackError::InvalidQuery("bad".into());
        assert_eq!(e.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn query_too_complex_is_400() {
        let e = FlapjackError::QueryTooComplex("complex".into());
        assert_eq!(e.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn missing_field_is_400() {
        let e = FlapjackError::MissingField("id".into());
        assert_eq!(e.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn type_mismatch_is_400() {
        let e = FlapjackError::TypeMismatch {
            field: "price".into(),
            expected: "integer".into(),
            actual: "string".into(),
        };
        assert_eq!(e.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn too_many_writes_is_503() {
        let e = FlapjackError::TooManyConcurrentWrites {
            current: 41,
            max: 40,
        };
        assert_eq!(e.status_code(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn document_too_large_is_400() {
        let e = FlapjackError::DocumentTooLarge {
            size: 4_000_000,
            max: 3_145_728,
        };
        assert_eq!(e.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn queue_full_is_429() {
        assert_eq!(
            FlapjackError::QueueFull.status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[test]
    fn io_error_is_500() {
        let e = FlapjackError::Io("disk full".into());
        assert_eq!(e.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn tantivy_error_is_500() {
        let e = FlapjackError::Tantivy("corrupt index".into());
        assert_eq!(e.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn query_parse_is_400() {
        let e = FlapjackError::QueryParse("unexpected token".into());
        assert_eq!(e.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn json_error_is_400() {
        let e = FlapjackError::Json("invalid json".into());
        assert_eq!(e.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn s3_error_is_500() {
        let e = FlapjackError::S3("access denied".into());
        assert_eq!(e.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn ssl_error_is_500() {
        let e = FlapjackError::Ssl("cert expired".into());
        assert_eq!(e.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn memory_pressure_is_503() {
        let e = FlapjackError::MemoryPressure {
            allocated_mb: 900,
            limit_mb: 1000,
            level: "warning".into(),
        };
        assert_eq!(e.status_code(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn task_not_found_is_404() {
        let e = FlapjackError::TaskNotFound("abc123".into());
        assert_eq!(e.status_code(), StatusCode::NOT_FOUND);
    }

    // ── Display / Error trait ───────────────────────────────────────────

    #[test]
    fn error_display_includes_message() {
        let e = FlapjackError::TenantNotFound("my_index".into());
        let msg = format!("{}", e);
        assert!(msg.contains("my_index"));
    }

    #[test]
    fn error_display_type_mismatch() {
        let e = FlapjackError::TypeMismatch {
            field: "price".into(),
            expected: "integer".into(),
            actual: "string".into(),
        };
        let msg = format!("{}", e);
        assert!(msg.contains("price"));
        assert!(msg.contains("integer"));
        assert!(msg.contains("string"));
    }

    // ── From conversions ────────────────────────────────────────────────

    #[test]
    fn from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let fj_err: FlapjackError = io_err.into();
        assert!(matches!(fj_err, FlapjackError::Io(_)));
        assert!(fj_err.to_string().contains("file not found"));
    }

    #[test]
    fn from_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let fj_err: FlapjackError = json_err.into();
        assert!(matches!(fj_err, FlapjackError::Json(_)));
    }

    // ── IndexPaused ─────────────────────────────────────────────────────

    #[test]
    fn test_index_paused_is_503() {
        let e = FlapjackError::IndexPaused("foo".into());
        assert_eq!(e.status_code(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_index_paused_display_message() {
        let e = FlapjackError::IndexPaused("foo".into());
        let msg = e.to_string();
        assert!(
            msg.contains("paused"),
            "message should contain 'paused': {}",
            msg
        );
        assert!(
            msg.contains("foo"),
            "message should contain index name 'foo': {}",
            msg
        );
    }

    // ── into_response() HTTP status correctness ──────────────────────────
    // These tests verify the ACTUAL HTTP response status code, not just status_code().
    // Both must agree — divergence means clients see different codes than logging/metrics.

    #[cfg(feature = "axum-support")]
    mod into_response_tests {
        use super::*;
        use axum::response::IntoResponse;

        fn status_from_response(e: FlapjackError) -> http::StatusCode {
            e.into_response().status()
        }

        #[test]
        fn too_many_concurrent_writes_http_response_is_503() {
            let e = FlapjackError::TooManyConcurrentWrites {
                current: 41,
                max: 40,
            };
            assert_eq!(
                status_from_response(e),
                StatusCode::SERVICE_UNAVAILABLE,
                "TooManyConcurrentWrites HTTP response must be 503 (matches status_code())"
            );
        }

        #[test]
        fn queue_full_http_response_is_429() {
            assert_eq!(
                status_from_response(FlapjackError::QueueFull),
                StatusCode::TOO_MANY_REQUESTS,
                "QueueFull HTTP response must be 429 (matches status_code())"
            );
        }

        #[test]
        fn index_paused_http_response_is_503_with_retry_after() {
            let response = FlapjackError::IndexPaused("my_index".into()).into_response();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(
                response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok()),
                Some("1"),
                "IndexPaused response must include Retry-After: 1"
            );
        }

        #[test]
        fn into_response_status_matches_status_code_for_all_variants() {
            // Exhaustive check: every variant's HTTP response status equals status_code()
            let errors: Vec<FlapjackError> = vec![
                FlapjackError::TenantNotFound("t".into()),
                FlapjackError::IndexAlreadyExists("t".into()),
                FlapjackError::InvalidQuery("q".into()),
                FlapjackError::QueryTooComplex("q".into()),
                FlapjackError::InvalidSchema("s".into()),
                FlapjackError::InvalidDocument("d".into()),
                FlapjackError::MissingField("f".into()),
                FlapjackError::TypeMismatch {
                    field: "f".into(),
                    expected: "int".into(),
                    actual: "str".into(),
                },
                FlapjackError::FieldNotFound("f".into()),
                FlapjackError::TooManyConcurrentWrites { current: 5, max: 4 },
                FlapjackError::BufferSizeExceeded {
                    requested: 100,
                    max: 50,
                },
                FlapjackError::DocumentTooLarge { size: 100, max: 50 },
                FlapjackError::BatchTooLarge { size: 100, max: 50 },
                FlapjackError::TaskNotFound("id".into()),
                FlapjackError::QueueFull,
                FlapjackError::Io("err".into()),
                FlapjackError::Tantivy("err".into()),
                FlapjackError::QueryParse("err".into()),
                FlapjackError::Json("err".into()),
                FlapjackError::S3("err".into()),
                FlapjackError::Ssl("err".into()),
                FlapjackError::Acme("err".into()),
                FlapjackError::Config("err".into()),
                FlapjackError::MemoryPressure {
                    allocated_mb: 900,
                    limit_mb: 1000,
                    level: "warn".into(),
                },
                FlapjackError::IndexPaused("idx".into()),
            ];
            for e in errors {
                let expected = e.status_code();
                let actual = status_from_response(e.clone());
                assert_eq!(
                    actual, expected,
                    "into_response() status ({}) != status_code() ({}) for {:?}",
                    actual, expected, e
                );
            }
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
                StatusCode::SERVICE_UNAVAILABLE,
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
                StatusCode::TOO_MANY_REQUESTS,
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
            FlapjackError::IndexPaused(ref index) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "index_paused",
                format!("Index is paused for migration: {}", index),
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
        if matches!(&self, FlapjackError::IndexPaused(_)) {
            response
                .headers_mut()
                .insert("Retry-After", "1".parse().unwrap());
        }
        response
    }
}
