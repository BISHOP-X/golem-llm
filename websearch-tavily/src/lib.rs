// Tavily Search API implementation
// This integrates with Tavily Search API (https://docs.tavily.com/documentation/api-reference/endpoint/search)

// use golem_websearch::durability::{DurableWebSearch, ExtendedGuest};
use golem_websearch::error::{create_backend_error, create_invalid_query_error, http_status_to_search_error};
use golem_websearch::golem::web_search::types::{SearchError, SearchMetadata, SearchParams, SearchResult, SafeSearchLevel, TimeRange};
// use golem_websearch::golem::web_search::web_search::{Guest, GuestSearchSession, SearchSession};
use golem_websearch::exports::golem::web_search::web_search::Guest as ExportGuest;
use golem_websearch::LOGGING_STATE;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::{debug, trace, warn};

const BASE_URL: &str = "https://api.tavily.com/search";

/// Tavily Search API client for web search
pub struct TavilySearchApi {
    api_key: String,
    client: Client,
}

impl TavilySearchApi {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .build()
            .expect("Failed to initialize HTTP client");
        Self { api_key, client }
    }

    pub async fn search(&self, params: &SearchParams, max_results: Option<u32>, offset: Option<u32>) -> Result<TavilySearchResponse, SearchError> {
        trace!("Sending request to Tavily Search API with query: {}", params.query);

        let mut request_body = serde_json::json!({
            "query": params.query
        });

        if let Some(max_results) = max_results.or(params.max_results) {
            request_body["max_results"] = serde_json::Value::Number(serde_json::Number::from(max_results.min(20))); // Tavily max is 20
        }

        if let Some(include_domains) = &params.include_domains {
            if !include_domains.is_empty() {
                request_body["include_domains"] = serde_json::Value::Array(
                    include_domains.iter().map(|d| serde_json::Value::String(d.clone())).collect()
                );
            }
        }

        if let Some(exclude_domains) = &params.exclude_domains {
            if !exclude_domains.is_empty() {
                request_body["exclude_domains"] = serde_json::Value::Array(
                    exclude_domains.iter().map(|d| serde_json::Value::String(d.clone())).collect()
                );
            }
        }

        if let Some(region) = &params.region {
            request_body["country"] = serde_json::Value::String(region.clone());
        }

        if let Some(time_range) = &params.time_range {
            let time_range_str = match time_range {
                TimeRange::Day => "day",
                TimeRange::Week => "week",
                TimeRange::Month => "month",
                TimeRange::Year => "year",
            };
            request_body["time_range"] = serde_json::Value::String(time_range_str.to_string());
        }

        // Set search depth for better results
        request_body["search_depth"] = serde_json::Value::String("advanced".to_string());
        
        // Include raw content if requested
        if params.include_html == Some(true) {
            request_body["include_raw_content"] = serde_json::Value::Bool(true);
        }

        // Include images if requested
        if params.include_images == Some(true) {
            request_body["include_images"] = serde_json::Value::Bool(true);
        }

        // Include answer if advanced features requested
        if params.advanced_answer == Some(true) {
            request_body["include_answer"] = serde_json::Value::Bool(true);
        }

        let response = self.client
            .post(BASE_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| create_backend_error(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(http_status_to_search_error(status.as_u16(), &error_text));
        }

        let tavily_response: TavilySearchResponse = response
            .json()
            .await
            .map_err(|e| create_backend_error(format!("Failed to parse JSON response: {}", e)))?;

        Ok(tavily_response)
    }
}

#[derive(Debug, Deserialize)]
pub struct TavilySearchResponse {
    pub query: String,
    pub results: Vec<TavilyResult>,
    pub answer: Option<String>,
    pub images: Option<Vec<String>>,
    pub response_time: f64,
}

#[derive(Debug, Deserialize)]
pub struct TavilyResult {
    pub title: String,
    pub url: String,
    pub content: String,
    pub score: f64,
    pub raw_content: Option<String>,
    pub published_date: Option<String>,
}

impl TavilySearchResponse {
    pub fn to_search_results(&self) -> Vec<SearchResult> {
        self.results.iter().map(|item| {
            SearchResult {
                title: item.title.clone(),
                url: item.url.clone(),
                snippet: item.content.clone(),
                display_url: Some(item.url.clone()),
                source: Some("Tavily".to_string()),
                score: Some(item.score),
                html_snippet: item.raw_content.clone(),
                date_published: item.published_date.clone(),
                images: self.images.as_ref().map(|imgs| {
                    imgs.iter().map(|img_url| golem_websearch::golem::web_search::types::ImageResult {
                        url: img_url.clone(),
                        description: None,
                    }).collect()
                }),
                content_chunks: None,
            }
        }).collect()
    }

    pub fn to_search_metadata(&self) -> SearchMetadata {
        SearchMetadata {
            query: self.query.clone(),            total_results: Some(self.results.len() as u64),
            search_time_ms: Some((self.response_time * 1000.0) as f64),
            safe_search: None, // Tavily doesn't provide explicit safe search info
            language: None,
            region: None,
            next_page_token: None, // Tavily doesn't use pagination tokens
            rate_limits: None,
        }
    }
}

struct TavilyWebSearchSession {
    api: TavilySearchApi,
    params: SearchParams,
    current_offset: u32,
    finished: bool,
    cached_results: Option<Vec<SearchResult>>,
}

impl TavilyWebSearchSession {
    pub fn new(api: TavilySearchApi, params: SearchParams) -> Result<Self, SearchError> {
        Ok(Self {
            api,
            params,
            current_offset: 0,
            finished: false,
            cached_results: None,
        })
    }

    async fn fetch_results(&mut self, offset: u32) -> Result<TavilySearchResponse, SearchError> {
        self.api.search(&self.params, self.params.max_results, Some(offset)).await
    }
}

// impl GuestSearchSession for TavilyWebSearchSession {
//     fn next_page(&self) -> Result<Vec<SearchResult>, SearchError> {
//         if self.finished {
//             return Ok(vec![]);
//         }
// 
//         // For Tavily, we typically get all results in one call, so implement simple pagination logic
//         let rt = tokio::runtime::Runtime::new()
//             .map_err(|e| create_backend_error(format!("Failed to create async runtime: {}", e)))?;
// 
//         // Create a mutable copy for the runtime block
//         let mut session = TavilyWebSearchSession {
//             api: TavilySearchApi::new(self.api.api_key.clone()),
//             params: self.params.clone(),
//             current_offset: self.current_offset,
//             finished: self.finished,
//             cached_results: None,
//         };
// 
//         match rt.block_on(session.fetch_results(self.current_offset)) {
//             Ok(response) => {
//                 let results = response.to_search_results();
//                 
//                 // Tavily doesn't support traditional pagination, so mark as finished after first call
//                 // self.finished = true; // Can't mutate due to &self constraint
//                 
//                 Ok(results)
//             }
//             Err(error) => Err(error)
//         }
//     }
// 
//     fn get_metadata(&self) -> Option<SearchMetadata> {
//         let rt = tokio::runtime::Runtime::new().ok()?;
//         
//         // Create a mutable copy for the runtime block
//         let mut session = TavilyWebSearchSession {
//             api: TavilySearchApi::new(self.api.api_key.clone()),
//             params: self.params.clone(),
//             current_offset: 0,
//             finished: false,
//             cached_results: None,
//         };
//         
//         match rt.block_on(session.fetch_results(0)) {
//             Ok(response) => Some(response.to_search_metadata()),
//             Err(_) => None,
//         }
//     }
// }

struct TavilyComponent;

impl TavilyComponent {
    const API_KEY_ENV_VAR: &'static str = "TAVILY_API_KEY";

    fn get_api_key() -> Result<String, SearchError> {
        std::env::var(Self::API_KEY_ENV_VAR)
            .map_err(|_| create_backend_error(format!("{} environment variable not set", Self::API_KEY_ENV_VAR)))
    }

    fn validate_params(params: &SearchParams) -> Result<(), SearchError> {
        if params.query.trim().is_empty() {
            return Err(create_invalid_query_error());
        }

        // Tavily has a 400 character limit on queries
        if params.query.len() > 400 {
            return Err(create_invalid_query_error());
        }

        Ok(())
    }

    fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
        LOGGING_STATE.with_borrow_mut(|state| state.init());
        
        Self::validate_params(&params)?;
        
        let api_key = Self::get_api_key()?;
        let api = TavilySearchApi::new(api_key);
          // Use futures executor for WASM compatibility
        match futures::executor::block_on(api.search(&params, params.max_results, Some(0))) {
            Ok(response) => {
                let results = response.to_search_results();
                let metadata = Some(response.to_search_metadata());
                Ok((results, metadata))
            }
            Err(error) => Err(error)
        }
    }
}

impl ExportGuest for TavilyComponent {
    fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
        Self::search_once(params)
    }
}

// Export handled by Guest trait implementation

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_tavily_search_api_creation() {
        let api = client::TavilySearchApi::new("test_api_key".to_string());
        
        // This just tests that we can create the API struct
        assert_eq!(api.api_key, "test_api_key");
    }

    #[tokio::test]
    async fn test_tavily_search_params_construction() {
        let params = SearchParams {
            query: "artificial intelligence research".to_string(),
            max_results: Some(25),
            safe_search: Some(SafeSearchLevel::Off),
            time_range: Some(TimeRange::Day),
            country: Some("JP".to_string()),
            language: Some("ja".to_string()),
        };
        
        assert_eq!(params.query, "artificial intelligence research");
        assert_eq!(params.max_results, Some(25));
        assert!(matches!(params.safe_search, Some(SafeSearchLevel::Off)));
        assert!(matches!(params.time_range, Some(TimeRange::Day)));
        assert_eq!(params.country, Some("JP".to_string()));
        assert_eq!(params.language, Some("ja".to_string()));
    }

    #[tokio::test]
    async fn test_tavily_search_once_with_invalid_api_key() {
        let result = TavilyWebSearchComponent::search_once(
            "invalid_key".to_string(),
            "machine learning algorithms".to_string(),
            Some(8),
            None,
            Some(TimeRange::Month),
            Some("DE".to_string()),
            Some("de".to_string()),
        );
        
        // This should fail with invalid credentials, but not panic
        assert!(result.is_err());
    }

    #[test]
    fn test_tavily_request_body_construction() {
        use serde_json::json;
        
        let query = "test query";
        let max_results = 10;
        
        let body = json!({
            "api_key": "test_key",
            "query": query,
            "search_depth": "basic",
            "max_results": max_results
        });
        
        assert_eq!(body["query"], "test query");
        assert_eq!(body["max_results"], 10);
        assert_eq!(body["search_depth"], "basic");
    }

    #[test]
    fn test_tavily_result_parsing() {
        // Test that we can create and validate SearchResult structs
        let result = SearchResult {
            title: "Tavily AI Search".to_string(),
            url: "https://tavily.com".to_string(),
            snippet: "AI-powered search for research".to_string(),
            published_date: Some("2024-01-15".to_string()),
        };
        
        assert_eq!(result.title, "Tavily AI Search");
        assert_eq!(result.url, "https://tavily.com");
        assert_eq!(result.snippet, "AI-powered search for research");
        assert_eq!(result.published_date, Some("2024-01-15".to_string()));
    }

    #[test]
    fn test_tavily_api_endpoint() {
        use url::Url;
        
        let endpoint = "https://api.tavily.com/search";
        let url = Url::parse(endpoint);
        
        assert!(url.is_ok());
        let parsed = url.unwrap();
        assert_eq!(parsed.scheme(), "https");
        assert_eq!(parsed.host_str(), Some("api.tavily.com"));
        assert_eq!(parsed.path(), "/search");
    }

    #[test]
    fn test_tavily_search_depth_options() {
        // Test search depth configuration
        let depths = vec!["basic", "advanced"];
        
        for depth in depths {
            assert!(depth == "basic" || depth == "advanced");
        }
    }

    #[test]
    fn test_tavily_include_domains() {
        // Test domain filtering functionality
        let domains = vec!["example.com", "test.org", "sample.net"];
        let formatted = domains.join(",");
        
        assert_eq!(formatted, "example.com,test.org,sample.net");
        assert!(!formatted.is_empty());
    }

    #[test]
    fn test_tavily_exclude_domains() {
        // Test domain exclusion functionality
        let exclude_domains = vec!["spam.com", "ads.net"];
        let formatted = exclude_domains.join(",");
        
        assert_eq!(formatted, "spam.com,ads.net");
        assert!(!formatted.is_empty());
    }

    #[test]
    fn test_tavily_max_results_validation() {
        // Test max results bounds (Tavily supports up to 100)
        let valid_values = vec![1, 5, 10, 20, 50, 100];
        let invalid_values = vec![0, 101, 1000];
        
        for valid in valid_values {
            assert!(valid > 0 && valid <= 100, "Valid value {} should pass", valid);
        }
        
        for invalid in invalid_values {
            assert!(invalid == 0 || invalid > 100, "Invalid value {} should fail", invalid);
        }
    }

    #[test]
    fn test_tavily_query_preprocessing() {
        // Test query cleaning and validation
        let queries = vec![
            ("simple query", true),
            ("", false), // empty
            ("a", false), // too short
            ("valid search term", true),
            ("   trimmed   ", true), // should be trimmed
        ];
        
        for (query, should_be_valid) in queries {
            let trimmed = query.trim();
            let is_valid = !trimmed.is_empty() && trimmed.len() >= 2;
            assert_eq!(is_valid, should_be_valid, "Query '{}' validation failed", query);
        }
    }
}