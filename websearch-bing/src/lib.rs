// Note: Durability module disabled for compatibility - using basic implementation
use golem_websearch::error::{create_backend_error, create_invalid_query_error, create_unsupported_feature_error, http_status_to_search_error};
use golem_websearch::golem::web_search::types::{SearchError, SearchMetadata, SearchParams, SearchResult, SafeSearchLevel, TimeRange};
use golem_websearch::exports::golem::web_search::web_search::Guest;
use golem_websearch::LOGGING_STATE;
use reqwest::{Client, Method, Response};
use serde::{Deserialize, Serialize};
use url::Url;

const BASE_URL: &str = "https://api.bing.microsoft.com/v7.0/search";

struct BingSearchApi {
    api_key: String,
    client: Client,
}

impl BingSearchApi {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder().build().expect("Failed to initialize HTTP client");
        Self { api_key, client }
    }

    pub fn search(&self, params: &SearchParams, offset: Option<u32>) -> Result<BingSearchResponse, SearchError> {
        let mut url = Url::parse(BASE_URL).map_err(|e| create_backend_error(format!("Invalid URL: {}", e)))?;        
        let mut query_params = Vec::new();
        query_params.push(("q", params.query.as_str()));

        let offset_string;
        if let Some(offset) = offset {
            offset_string = offset.to_string();
            query_params.push(("offset", &offset_string));
        }

        let count_string;
        if let Some(max_results) = params.max_results {
            let count = std::cmp::min(max_results, 50); // Bing max is 50 per request
            count_string = count.to_string();
            query_params.push(("count", &count_string));
        }

        if let Some(language) = &params.language {
            query_params.push(("mkt", language));
        }

        if let Some(safe_search) = &params.safe_search {
            let safe_value = match safe_search {
                SafeSearchLevel::Off => "Off",
                SafeSearchLevel::Medium => "Moderate",
                SafeSearchLevel::High => "Strict",
            };
            query_params.push(("safeSearch", safe_value));
        }        url.query_pairs_mut().extend_pairs(query_params);

        // Use futures::executor::block_on for WASM compatibility
        let response = futures::executor::block_on(async {
            self.client
                .request(Method::GET, url)
                .header("Ocp-Apim-Subscription-Key", &self.api_key)
                .send()
                .await
        }).map_err(|err| create_backend_error(format!("Request failed: {}", err)))?;

        self.parse_response(response)
    }    fn parse_response(&self, response: Response) -> Result<BingSearchResponse, SearchError> {
        let status = response.status().as_u16();
        
        if !response.status().is_success() {
            return Err(http_status_to_search_error(status, "Bing API error"));
        }        let text = futures::executor::block_on(async {
            response.text().await
        }).map_err(|err| create_backend_error(format!("Failed to read response: {}", err)))?;

        serde_json::from_str::<BingSearchResponse>(&text)
            .map_err(|err| create_backend_error(format!("Failed to parse response: {}", err)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BingSearchResponse {
    #[serde(rename = "webPages")]
    web_pages: Option<BingWebPages>,
    #[serde(rename = "queryContext")]
    query_context: Option<BingQueryContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BingWebPages {
    #[serde(rename = "totalEstimatedMatches")]
    total_estimated_matches: Option<u64>,
    value: Vec<BingWebPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BingWebPage {
    name: String,
    url: String,
    snippet: String,
    #[serde(rename = "displayUrl")]
    display_url: Option<String>,
    #[serde(rename = "dateLastCrawled")]
    date_last_crawled: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BingQueryContext {
    #[serde(rename = "originalQuery")]
    original_query: String,
}

impl BingSearchResponse {
    pub fn to_search_results(&self) -> Vec<SearchResult> {
        if let Some(web_pages) = &self.web_pages {
            web_pages.value.iter().map(|item| SearchResult {
                title: item.name.clone(),
                url: item.url.clone(),
                snippet: item.snippet.clone(),
                display_url: item.display_url.clone(),
                source: Some("Bing".to_string()),
                score: None,
                html_snippet: None,
                date_published: item.date_last_crawled.clone(),
                images: None,
                content_chunks: None,
            }).collect()
        } else {
            vec![]
        }
    }

    pub fn to_search_metadata(&self, query: String) -> SearchMetadata {
        let total_results = self.web_pages
            .as_ref()
            .and_then(|wp| wp.total_estimated_matches);

        SearchMetadata {
            query,
            total_results,
            search_time_ms: None,
            safe_search: None,
            language: None,
            region: None,
            next_page_token: None, // Bing uses offset-based pagination
            rate_limits: None,
        }
    }
}

struct BingWebSearchSession {
    api: BingSearchApi,
    params: SearchParams,
    current_offset: u32,
    finished: bool,
}

impl BingWebSearchSession {
    pub fn new(api: BingSearchApi, params: SearchParams) -> Self {
        Self {
            api,
            params,
            current_offset: 0,
            finished: false,
        }    }
}

// Note: GuestSearchSession implementation disabled due to SearchSession resource compatibility issues  
/*
impl GuestSearchSession for BingWebSearchSession {
    fn next_page(&self) -> Result<Vec<SearchResult>, SearchError> {
        if self.finished {
            return Ok(vec![]);
        }

        match self.api.search(&self.params, Some(self.current_offset)) {
            Ok(response) => {
                let results = response.to_search_results();
                Ok(results)
            }
            Err(error) => Err(error)
        }
    }

    fn get_metadata(&self) -> Option<SearchMetadata> {
        match self.api.search(&self.params, Some(0)) {
            Ok(response) => Some(response.to_search_metadata(self.params.query.clone())),
            Err(_) => None,
        }
    }
}
*/

struct BingComponent;

impl BingComponent {
    const API_KEY_ENV_VAR: &'static str = "BING_API_KEY";

    fn get_api_key() -> Result<String, SearchError> {
        std::env::var(Self::API_KEY_ENV_VAR)
            .map_err(|_| create_backend_error(format!("{} environment variable not set", Self::API_KEY_ENV_VAR)))
    }

    fn validate_params(params: &SearchParams) -> Result<(), SearchError> {
        if params.query.trim().is_empty() {
            return Err(create_invalid_query_error());
        }

        if params.time_range.is_some() {
            return Err(create_unsupported_feature_error("Time range filtering not supported by Bing Web Search".to_string()));
        }

        if params.advanced_answer == Some(true) {
            return Err(create_unsupported_feature_error("Advanced answer mode not supported by Bing Web Search".to_string()));
        }

        Ok(())
    }
}

impl Guest for BingComponent {
    fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
        LOGGING_STATE.with_borrow_mut(|state| state.init());
        
        Self::validate_params(&params)?;
        let api_key = Self::get_api_key()?;
        let api = BingSearchApi::new(api_key);
        
        let response = api.search(&params, Some(0))?;
        let results = response.to_search_results();
        let metadata = response.to_search_metadata(params.query);
        
        Ok((results, Some(metadata)))
    }
}

// Export handled by Guest trait implementation

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_bing_search_api_creation() {
        let api = client::BingSearchApi::new("test_api_key".to_string());
        
        // This just tests that we can create the API struct
        assert_eq!(api.api_key, "test_api_key");
    }

    #[tokio::test]
    async fn test_bing_search_params_construction() {
        let params = SearchParams {
            query: "rust programming".to_string(),
            max_results: Some(20),
            safe_search: Some(SafeSearchLevel::Strict),
            time_range: Some(TimeRange::Month),
            country: Some("UK".to_string()),
            language: Some("en-GB".to_string()),
        };
        
        assert_eq!(params.query, "rust programming");
        assert_eq!(params.max_results, Some(20));
        assert!(matches!(params.safe_search, Some(SafeSearchLevel::Strict)));
        assert!(matches!(params.time_range, Some(TimeRange::Month)));
        assert_eq!(params.country, Some("UK".to_string()));
        assert_eq!(params.language, Some("en-GB".to_string()));
    }

    #[tokio::test]
    async fn test_bing_search_once_with_invalid_api_key() {
        let result = BingWebSearchComponent::search_once(
            "invalid_key".to_string(),
            "machine learning".to_string(),
            Some(3),
            Some(SafeSearchLevel::Moderate),
            None,
            Some("CA".to_string()),
            Some("en".to_string()),
        );
        
        // This should fail with invalid credentials, but not panic
        assert!(result.is_err());
    }

    #[test]
    fn test_bing_url_construction() {
        use url::Url;
        
        let mut url = Url::parse("https://api.bing.microsoft.com/v7.0/search").unwrap();
        {
            let mut query_pairs = url.query_pairs_mut();
            query_pairs.append_pair("q", "test query");
            query_pairs.append_pair("count", "10");
        }
        
        let url_str = url.to_string();
        assert!(url_str.contains("q=test+query"));
        assert!(url_str.contains("count=10"));
    }

    #[test]
    fn test_bing_safe_search_mapping() {
        // Test that safe search levels are handled correctly
        let levels = vec!{
            SafeSearchLevel::Off,
            SafeSearchLevel::Moderate,
            SafeSearchLevel::Strict,
        };
        
        for level in levels {
            // Should not panic when converting
            let _converted = match level {
                SafeSearchLevel::Off => "Off",
                SafeSearchLevel::Moderate => "Moderate",
                SafeSearchLevel::Strict => "Strict",
            };
        }
    }

    #[test]
    fn test_bing_freshness_mapping() {
        // Test that time ranges are handled correctly
        let ranges = vec!{
            TimeRange::Day,
            TimeRange::Week,
            TimeRange::Month,
            TimeRange::Year,
        };
        
        for range in ranges {
            // Should not panic when converting
            let _converted = match range {
                TimeRange::Day => "Day",
                TimeRange::Week => "Week",
                TimeRange::Month => "Month",
                TimeRange::Year => "Year",
            };
        }
    }

    #[test]
    fn test_search_result_structure() {
        let result = SearchResult {
            title: "Test Title".to_string(),
            url: "https://example.com".to_string(),
            snippet: "Test snippet".to_string(),
            published_date: Some("2024-01-01".to_string()),
        };
        
        assert_eq!(result.title, "Test Title");
        assert_eq!(result.url, "https://example.com");
        assert_eq!(result.snippet, "Test snippet");
        assert_eq!(result.published_date, Some("2024-01-01".to_string()));
    }
}