// pub mod durability;  // Disabled due to SearchSession resource compatibility issues
pub mod error;
pub mod search_session;

wit_bindgen::generate!({
    path: "./wit",
    world: "websearch-library",
    generate_all,
    generate_unused_types: true,
    additional_derives: [PartialEq, golem_rust::FromValueAndType, golem_rust::IntoValue],    pub_export_macro: true,
});

pub use __export_websearch_library_impl as export_websearch;
use std::cell::RefCell;
use std::str::FromStr;

pub struct LoggingState {
    logging_initialized: bool,
}

impl LoggingState {
    /// Initializes WASI logging based on the `GOLEM_WEBSEARCH_LOG` environment variable.
    pub fn init(&mut self) {
        if !self.logging_initialized {
            let _ = wasi_logger::Logger::install();
            let max_level: log::LevelFilter =
                log::LevelFilter::from_str(&std::env::var("GOLEM_WEBSEARCH_LOG").unwrap_or_default())
                    .unwrap_or(log::LevelFilter::Info);
            log::set_max_level(max_level);
            self.logging_initialized = true;
        }
    }
}

thread_local! {
    /// This holds the state of our application.
    pub static LOGGING_STATE: RefCell<LoggingState> = const { RefCell::new(LoggingState {
        logging_initialized: false,
    }) };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golem::web_search::types::{SearchParams, SearchResult, SearchMetadata, SafeSearchLevel, TimeRange};

    #[test]
    fn test_search_params_creation() {
        let params = SearchParams {
            query: "test query".to_string(),
            safe_search: Some(SafeSearchLevel::Medium),
            language: Some("en".to_string()),
            region: Some("US".to_string()),
            max_results: Some(10),
            time_range: Some(TimeRange::Day),
            include_domains: Some(vec!["example.com".to_string()]),
            exclude_domains: Some(vec!["spam.com".to_string()]),
            include_images: Some(false),
            include_html: Some(true),
            advanced_answer: Some(false),
        };

        assert_eq!(params.query, "test query");
        assert_eq!(params.max_results, Some(10));
    }

    #[test]
    fn test_search_result_creation() {
        let result = SearchResult {
            title: "Test Title".to_string(),
            url: "https://example.com".to_string(),
            snippet: "Test snippet".to_string(),
            display_url: Some("example.com".to_string()),
            source: Some("Google".to_string()),
            score: Some(0.95),
            html_snippet: Some("<b>Test</b> snippet".to_string()),
            date_published: Some("2023-01-01".to_string()),
            images: None,
            content_chunks: None,
        };

        assert_eq!(result.title, "Test Title");
        assert_eq!(result.url, "https://example.com");
    }

    #[test]
    fn test_search_metadata_creation() {
        let metadata = SearchMetadata {
            query: "test query".to_string(),
            total_results: Some(1000),
            search_time_ms: Some(150.0),
            safe_search: Some(SafeSearchLevel::Medium),
            language: Some("en".to_string()),
            region: Some("US".to_string()),
            next_page_token: Some("next_page_123".to_string()),
            rate_limits: None,
        };

        assert_eq!(metadata.query, "test query");
        assert_eq!(metadata.total_results, Some(1000));
        assert_eq!(metadata.search_time_ms, Some(150.0));
    }
} 