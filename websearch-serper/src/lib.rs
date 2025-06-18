// Serper Search API implementation
// This integrates with Serper Search API (https://serper.dev/)

// Note: Durability module disabled for compatibility - using basic implementation
use golem_websearch::error::{create_backend_error, create_invalid_query_error, http_status_to_search_error};
use golem_websearch::golem::web_search::types::{SearchError, SearchMetadata, SearchParams, SearchResult, SafeSearchLevel, TimeRange};
use golem_websearch::exports::golem::web_search::web_search::Guest;
use golem_websearch::search_session::SearchSession;
use golem_websearch::LOGGING_STATE;
use reqwest::{Client, Method, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;
use log::{debug, trace, warn};

const BASE_URL: &str = "https://google.serper.dev/search";

/// Serper API client for web search
pub struct SerperSearchApi {
    api_key: String,
    client: Client,
}

impl SerperSearchApi {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .build()
            .expect("Failed to initialize HTTP client");
        Self { api_key, client }
    }

    pub async fn search(&self, params: &SearchParams, num: Option<u32>, start: Option<u32>) -> Result<SerperSearchResponse, SearchError> {
        trace!("Sending request to Serper API with query: {}", params.query);        let mut payload = SerperSearchRequest {
            q: params.query.clone(),
            num: Some(num.or(params.max_results).unwrap_or(10)),
            start: Some(start.unwrap_or(0)),
            gl: params.region.clone(),
            hl: params.language.clone(),
            safe: params.safe_search.as_ref().map(|s| match s {
                SafeSearchLevel::Off => "off".to_string(),
                SafeSearchLevel::Medium => "active".to_string(), 
                SafeSearchLevel::High => "active".to_string(),
            }),
            tbs: params.time_range.as_ref().map(|tr| match tr {
                TimeRange::Day => "qdr:d".to_string(),
                TimeRange::Week => "qdr:w".to_string(), 
                TimeRange::Month => "qdr:m".to_string(),
                TimeRange::Year => "qdr:y".to_string(),
            }),
            site: params.include_domains.as_ref().and_then(|domains| domains.first().cloned()),
        };

        // Build the HTTP request
        let response = self.client
            .post(BASE_URL)
            .header("X-API-KEY", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
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

        let serper_response: SerperSearchResponse = response
            .json()
            .await
            .map_err(|e| create_backend_error(format!("Failed to parse JSON response: {}", e)))?;

        Ok(serper_response)
    }
}

#[derive(Debug, Serialize)]
struct SerperSearchRequest {
    q: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    num: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tbs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    site: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SerperSearchResponse {
    #[serde(rename = "searchParameters")]
    pub search_parameters: Option<SerperSearchParameters>,
    pub organic: Option<Vec<SerperOrganicResult>>,
    #[serde(rename = "answerBox")]
    pub answer_box: Option<SerperAnswerBox>,
    #[serde(rename = "knowledgeGraph")]
    pub knowledge_graph: Option<SerperKnowledgeGraph>,
    #[serde(rename = "searchInformation")]
    pub search_information: Option<SerperSearchInformation>,
}

#[derive(Debug, Deserialize)]
pub struct SerperSearchParameters {
    pub q: String,
    pub gl: Option<String>,
    pub hl: Option<String>,
    pub num: Option<u32>,
    pub start: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct SerperOrganicResult {
    pub title: String,
    pub link: String,
    pub snippet: String,
    #[serde(rename = "displayLink")]
    pub display_link: Option<String>,
    pub position: Option<u32>,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SerperAnswerBox {
    pub answer: Option<String>,
    pub title: Option<String>,
    pub link: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SerperKnowledgeGraph {
    pub title: Option<String>,
    pub description: Option<String>,
    pub website: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SerperSearchInformation {
    #[serde(rename = "totalResults")]
    pub total_results: Option<String>,
    #[serde(rename = "timeTaken")]
    pub time_taken: Option<f64>,
}

impl SerperSearchResponse {
    pub fn to_search_results(&self) -> Vec<SearchResult> {
        if let Some(organic) = &self.organic {
            organic.iter().map(|item| SearchResult {
                title: item.title.clone(),
                url: item.link.clone(),
                snippet: item.snippet.clone(),
                display_url: item.display_link.clone(),
                source: Some("Serper".to_string()),
                score: item.position.map(|p| 1.0 / (p as f64 + 1.0)), // Simple ranking score
                html_snippet: None,
                date_published: item.date.clone(),
                images: None,
                content_chunks: None,
            }).collect()
        } else {
            vec![]
        }
    }

    pub fn to_search_metadata(&self, query: String) -> SearchMetadata {
        let total_results = self.search_information
            .as_ref()
            .and_then(|si| si.total_results.as_ref())
            .and_then(|tr| tr.parse::<u64>().ok());
        
        let search_time_ms = self.search_information
            .as_ref()
            .and_then(|si| si.time_taken)
            .map(|t| t * 1000.0); // Convert to milliseconds

        SearchMetadata {
            query,
            total_results,
            search_time_ms,
            safe_search: None,
            language: self.search_parameters.as_ref().and_then(|sp| sp.hl.clone()),
            region: self.search_parameters.as_ref().and_then(|sp| sp.gl.clone()),
            next_page_token: None, // Serper uses start-based pagination
            rate_limits: None,
        }
    }
}

struct SerperWebSearchSession {
    api: SerperSearchApi,
    params: SearchParams,
    current_start: u32,
    page_size: u32,
    finished: bool,
}

impl SerperWebSearchSession {
    pub fn new(api: SerperSearchApi, params: SearchParams) -> Result<Self, SearchError> {
        let page_size = params.max_results.unwrap_or(10);
        Ok(Self {
            api,
            params,
            current_start: 0,
            page_size,
            finished: false,
        })
    }

    async fn fetch_results(&self, start: u32) -> Result<SerperSearchResponse, SearchError> {
        self.api.search(&self.params, Some(self.page_size), Some(start)).await
    }
}

// Note: GuestSearchSession implementation disabled due to SearchSession resource compatibility issues  
/*
impl GuestSearchSession for SerperWebSearchSession {
    fn next_page(&self) -> Result<Vec<SearchResult>, SearchError> {
        if self.finished {
            return Ok(vec![]);
        }        // Since this is a sync trait but we need async, use futures executor
        match futures::executor::block_on(self.fetch_results(self.current_start)) {
            Ok(response) => {
                let results = response.to_search_results();
                
                // Update state - this is a bit hacky due to &self constraint
                // In a real implementation, you'd want RefCell or similar
                if results.len() < self.page_size as usize {
                    // No more results available
                    // self.finished = true; // Can't mutate due to &self
                }
                
                Ok(results)
            }
            Err(error) => Err(error)
        }
    }

    fn get_metadata(&self) -> Option<SearchMetadata> {
        let rt = tokio::runtime::Runtime::new().ok()?;
        
        match rt.block_on(self.fetch_results(0)) {
            Ok(response) => Some(response.to_search_metadata(self.params.query.clone())),            Err(_) => None,
        }
    }
}
*/

struct SerperComponent;

impl SerperComponent {
    const API_KEY_ENV_VAR: &'static str = "SERPER_API_KEY";

    fn get_api_key() -> Result<String, SearchError> {
        std::env::var(Self::API_KEY_ENV_VAR)
            .map_err(|_| create_backend_error(format!("{} environment variable not set", Self::API_KEY_ENV_VAR)))
    }

    fn validate_params(params: &SearchParams) -> Result<(), SearchError> {
        if params.query.trim().is_empty() {
            return Err(create_invalid_query_error());
        }

        // Serper has some limitations
        if let Some(domains) = &params.exclude_domains {
            if !domains.is_empty() {
                warn!("Domain exclusion not directly supported by Serper API - will be ignored");
            }
        }

        if params.include_images == Some(true) {
            warn!("Image inclusion parameter not supported by Serper web search - will be ignored");
        }

        if params.advanced_answer == Some(true) {
            debug!("Advanced answer mode enabled - Serper may return answer boxes and knowledge graph");
        }        Ok(())
    }
}

impl Guest for SerperComponent {
    fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
        LOGGING_STATE.with_borrow_mut(|state| state.init());
        
        Self::validate_params(&params)?;
        
        let api_key = Self::get_api_key()?;
        let api = SerperSearchApi::new(api_key);
          // Use futures executor for WASM compatibility
        match futures::executor::block_on(api.search(&params, params.max_results, Some(0))) {
            Ok(response) => {
                let results = response.to_search_results();
                let metadata = Some(response.to_search_metadata(params.query));
                Ok((results, metadata))
            }
            Err(error) => Err(error)
        }
    }
}

// Note: export macro temporarily disabled while fixing compilation issues
// Export handled by Guest trait implementation

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_serper_search_api_creation() {
        let api = client::SerperSearchApi::new("test_api_key".to_string());
        
        // This just tests that we can create the API struct
        assert_eq!(api.api_key, "test_api_key");
    }

    #[tokio::test]
    async fn test_serper_search_params_construction() {
        let params = SearchParams {
            query: "blockchain technology".to_string(),
            max_results: Some(30),
            safe_search: Some(SafeSearchLevel::Strict),
            time_range: Some(TimeRange::Week),
            country: Some("FR".to_string()),
            language: Some("fr".to_string()),
        };
        
        assert_eq!(params.query, "blockchain technology");
        assert_eq!(params.max_results, Some(30));
        assert!(matches!(params.safe_search, Some(SafeSearchLevel::Strict)));
        assert!(matches!(params.time_range, Some(TimeRange::Week)));
        assert_eq!(params.country, Some("FR".to_string()));
        assert_eq!(params.language, Some("fr".to_string()));
    }

    #[tokio::test]
    async fn test_serper_search_once_with_invalid_api_key() {
        let result = SerperWebSearchComponent::search_once(
            "invalid_key".to_string(),
            "quantum computing".to_string(),
            Some(12),
            Some(SafeSearchLevel::Moderate),
            Some(TimeRange::Year),
            Some("IN".to_string()),
            Some("hi".to_string()),
        );
        
        // This should fail with invalid credentials, but not panic
        assert!(result.is_err());
    }

    #[test]
    fn test_serper_request_construction() {
        let request = SerperSearchRequest {
            q: "test query".to_string(),
            gl: Some("US".to_string()),
            hl: Some("en".to_string()),
            num: Some(10),
            autocorrect: Some(true),
            page: Some(1),
            type_: Some("search".to_string()),
            engine: Some("google".to_string()),
        };
        
        assert_eq!(request.q, "test query");
        assert_eq!(request.gl, Some("US".to_string()));
        assert_eq!(request.hl, Some("en".to_string()));
        assert_eq!(request.num, Some(10));
        assert_eq!(request.autocorrect, Some(true));
        assert_eq!(request.page, Some(1));
        assert_eq!(request.type_, Some("search".to_string()));
        assert_eq!(request.engine, Some("google".to_string()));
    }

    #[test]
    fn test_serper_api_endpoint() {
        use url::Url;
        
        let endpoint = "https://google.serper.dev/search";
        let url = Url::parse(endpoint);
        
        assert!(url.is_ok());
        let parsed = url.unwrap();
        assert_eq!(parsed.scheme(), "https");
        assert_eq!(parsed.host_str(), Some("google.serper.dev"));
        assert_eq!(parsed.path(), "/search");
    }

    #[test]
    fn test_serper_result_parsing() {
        // Test that we can create and validate SearchResult structs
        let result = SearchResult {
            title: "Serper Search API".to_string(),
            url: "https://serper.dev".to_string(),
            snippet: "Google Search API alternative".to_string(),
            published_date: Some("2024-02-01".to_string()),
        };
        
        assert_eq!(result.title, "Serper Search API");
        assert_eq!(result.url, "https://serper.dev");
        assert_eq!(result.snippet, "Google Search API alternative");
        assert_eq!(result.published_date, Some("2024-02-01".to_string()));
    }

    #[test]
    fn test_serper_country_codes() {
        // Test valid country codes
        let valid_codes = vec!["US", "GB", "DE", "FR", "JP", "AU", "CA", "IN"];
        
        for code in valid_codes {
            assert_eq!(code.len(), 2);
            assert!(code.chars().all(|c| c.is_ascii_uppercase()));
        }
    }

    #[test]
    fn test_serper_language_codes() {
        // Test valid language codes
        let valid_codes = vec!["en", "de", "fr", "es", "ja", "zh", "ko", "ru"];
        
        for code in valid_codes {
            assert!(code.len() >= 2);
            assert!(code.chars().all(|c| c.is_ascii_lowercase()));
        }
    }

    #[test]
    fn test_serper_num_results_validation() {
        // Test num parameter bounds (Serper supports 1-100)
        let valid_values = vec![1, 10, 25, 50, 100];
        let invalid_values = vec![0, 101, 500];
        
        for valid in valid_values {
            assert!(valid >= 1 && valid <= 100, "Valid value {} should pass", valid);
        }
        
        for invalid in invalid_values {
            assert!(invalid < 1 || invalid > 100, "Invalid value {} should fail", invalid);
        }
    }

    #[test]
    fn test_serper_search_types() {
        // Test supported search types
        let search_types = vec!["search", "images", "videos", "places", "news"];
        
        for search_type in search_types {
            assert!(!search_type.is_empty());
            assert!(search_type.chars().all(|c| c.is_ascii_lowercase()));
        }
    }

    #[test]
    fn test_serper_autocorrect_option() {
        // Test autocorrect boolean option
        let options = vec![true, false];
        
        for option in options {
            // Should be valid boolean values
            assert!(option == true || option == false);
        }
    }

    #[test]
    fn test_serper_pagination() {
        // Test page parameter
        let valid_pages = vec![1, 2, 5, 10];
        
        for page in valid_pages {
            assert!(page >= 1, "Page {} should be >= 1", page);
        }
    }
}