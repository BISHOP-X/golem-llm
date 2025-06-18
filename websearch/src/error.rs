use crate::golem::web_search::types::{SearchError};

/// Helper functions for creating and handling search errors
pub fn create_invalid_query_error() -> SearchError {
    SearchError::InvalidQuery
}

pub fn create_rate_limited_error(retry_after: u32) -> SearchError {
    SearchError::RateLimited(retry_after)
}

pub fn create_unsupported_feature_error(feature: String) -> SearchError {
    SearchError::UnsupportedFeature(feature)
}

pub fn create_backend_error(message: String) -> SearchError {
    SearchError::BackendError(message)
}

/// Convert HTTP status codes to appropriate search errors
pub fn http_status_to_search_error(status: u16, message: &str) -> SearchError {
    match status {
        400 => create_invalid_query_error(),
        401 | 403 => create_backend_error(format!("Authentication failed: {}", message)),
        429 => create_rate_limited_error(60), // Default to 60 seconds retry
        500..=599 => create_backend_error(format!("Server error: {}", message)),
        _ => create_backend_error(format!("HTTP error {}: {}", status, message)),
    }
} 