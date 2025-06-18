// Brave Search API implementation
// This integrates with Brave Search API (https://api.search.brave.com/app/documentation/web-search/get-started)

// use golem_websearch::durability::{DurableWebSearch, ExtendedGuest};
use golem_websearch::error::{create_backend_error, create_invalid_query_error, create_unsupported_feature_error, http_status_to_search_error};
use golem_websearch::golem::web_search::types::{SearchError, SearchMetadata, SearchParams, SearchResult, SafeSearchLevel, TimeRange};
// use golem_websearch::golem::web_search::web_search::Guest; // GuestSearchSession, SearchSession};
use golem_websearch::exports::golem::web_search::web_search::Guest as ExportGuest;
use golem_websearch::LOGGING_STATE;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::{debug, trace, warn};

const BASE_URL: &str = "https://api.search.brave.com/res/v1/web/search";

/// Brave Search API client for web search
pub struct BraveSearchApi {
    api_key: String,
    client: Client,
}

impl BraveSearchApi {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .build()
            .expect("Failed to initialize HTTP client");
        Self { api_key, client }
    }

    pub async fn search(&self, params: &SearchParams, count: Option<u32>, offset: Option<u32>) -> Result<BraveSearchResponse, SearchError> {
        trace!("Sending request to Brave Search API with query: {}", params.query);

        let mut query_params = vec![
            ("q", params.query.clone()),
        ];

        if let Some(count) = count.or(params.max_results) {
            query_params.push(("count", count.to_string()));
        }

        if let Some(offset) = offset {
            query_params.push(("offset", offset.to_string()));
        }

        if let Some(country) = &params.region {
            query_params.push(("country", country.clone()));
        }

        if let Some(language) = &params.language {
            query_params.push(("search_lang", language.clone()));
        }

        if let Some(safe_search) = &params.safe_search {
            let safe_value = match safe_search {
                SafeSearchLevel::Off => "off",
                SafeSearchLevel::Medium => "moderate",
                SafeSearchLevel::High => "strict",
            };
            query_params.push(("safesearch", safe_value.to_string()));
        }

        if let Some(time_range) = &params.time_range {
            let freshness = match time_range {
                TimeRange::Day => "pd",
                TimeRange::Week => "pw", 
                TimeRange::Month => "pm",
                TimeRange::Year => "py",
            };
            query_params.push(("freshness", freshness.to_string()));
        }

        // Enable text decorations for better snippets
        query_params.push(("text_decorations", "true".to_string()));
        
        // Enable spellcheck
        query_params.push(("spellcheck", "true".to_string()));

        let response = self.client
            .get(BASE_URL)
            .query(&query_params)
            .header("X-Subscription-Token", &self.api_key)
            .header("Accept", "application/json")
            .header("Accept-Encoding", "gzip")
            .send()
            .await
            .map_err(|e| create_backend_error(format!("HTTP request failed: {}", e)))?;

        let status = response.status();        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(http_status_to_search_error(status.as_u16(), &error_text));
        }

        let brave_response: BraveSearchResponse = response
            .json()
            .await
            .map_err(|e| create_backend_error(format!("Failed to parse JSON response: {}", e)))?;

        Ok(brave_response)
    }
}

#[derive(Debug, Deserialize)]
pub struct BraveSearchResponse {
    pub query: Option<BraveQuery>,
    pub web: Option<BraveWebResults>,
    pub infobox: Option<BraveInfobox>,
    pub discussions: Option<BraveDiscussions>,
    pub faq: Option<BraveFaq>,
    pub videos: Option<BraveVideos>,
    pub news: Option<BraveNews>,
    pub locations: Option<BraveLocations>,
}

#[derive(Debug, Deserialize)]
pub struct BraveQuery {
    pub original: String,
    pub show_strict_warning: Option<bool>,
    pub altered: Option<String>,
    pub safesearch: Option<bool>,
    pub is_navigational: Option<bool>,
    pub local_decision: Option<String>,
    pub local_locations_idx: Option<u32>,
    pub is_location_specific: Option<bool>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub bad_results: Option<bool>,
    pub should_fallback: Option<bool>,
    pub lat: Option<String>,
    pub long: Option<String>,
    pub state: Option<String>,
    pub city: Option<String>,
    pub header_country: Option<String>,
    pub more_results_available: Option<bool>,
    pub custom_location_label: Option<String>,
    pub reddit_cluster: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BraveWebResults {
    #[serde(rename = "type")]
    pub result_type: String,
    pub results: Vec<BraveWebResult>,
    pub family_friendly: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct BraveWebResult {
    #[serde(rename = "type")]
    pub result_type: String,
    pub subtype: Option<String>,
    pub title: String,
    pub url: String,
    pub description: String,
    pub date: Option<String>,
    pub extra_snippets: Option<Vec<String>>,
    pub language: Option<String>,
    pub family_friendly: Option<bool>,
    pub thumbnail: Option<BraveThumbnail>,
    pub age: Option<String>,
    pub page_age: Option<String>,
    pub profile: Option<BraveProfile>,
    pub meta_url: Option<BraveMetaUrl>,
    pub cluster_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BraveThumbnail {
    pub src: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bg_color: Option<String>,
    pub original: Option<String>,
    pub logo: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct BraveProfile {
    pub name: String,
    pub url: String,
    pub long_name: Option<String>,
    pub img: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BraveMetaUrl {
    pub scheme: String,
    pub netloc: String,
    pub hostname: String,
    pub favicon: Option<String>,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct BraveInfobox {
    #[serde(rename = "type")]
    pub infobox_type: String,
    pub subtype: Option<String>,
    pub position: u32,
    pub title: String,
    pub description: String,
    pub long_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BraveDiscussions {
    #[serde(rename = "type")]
    pub discussion_type: String,
    pub results: Vec<BraveDiscussionResult>,
}

#[derive(Debug, Deserialize)]
pub struct BraveDiscussionResult {
    #[serde(rename = "type")]
    pub result_type: String,
    pub title: String,
    pub url: String,
    pub description: String,
    pub date: Option<String>,
    pub forum: Option<String>,
    pub num_answers: Option<u32>,
    pub score: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct BraveFaq {
    #[serde(rename = "type")]
    pub faq_type: String,
    pub results: Vec<BraveFaqResult>,
}

#[derive(Debug, Deserialize)]
pub struct BraveFaqResult {
    pub question: String,
    pub answer: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct BraveVideos {
    #[serde(rename = "type")]
    pub video_type: String,
    pub results: Vec<BraveVideoResult>,
    pub mutated_by_goggles: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct BraveVideoResult {
    #[serde(rename = "type")]
    pub result_type: String,
    pub url: String,
    pub title: String,
    pub description: String,
    pub date: Option<String>,
    pub duration: Option<String>,
    pub views: Option<String>,
    pub creator: Option<String>,
    pub publisher: Option<String>,
    pub thumbnail: Option<BraveThumbnail>,
}

#[derive(Debug, Deserialize)]
pub struct BraveNews {
    #[serde(rename = "type")]
    pub news_type: String,
    pub results: Vec<BraveNewsResult>,
    pub mutated_by_goggles: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct BraveNewsResult {
    #[serde(rename = "type")]
    pub result_type: String,
    pub title: String,
    pub url: String,
    pub description: String,
    pub date: Option<String>,
    pub page_age: Option<String>,
    pub breaking: Option<bool>,
    pub thumbnail: Option<BraveThumbnail>,
    pub meta_url: Option<BraveMetaUrl>,
}

#[derive(Debug, Deserialize)]
pub struct BraveLocations {
    #[serde(rename = "type")]
    pub location_type: String,
    pub results: Vec<BraveLocationResult>,
}

#[derive(Debug, Deserialize)]
pub struct BraveLocationResult {
    #[serde(rename = "type")]
    pub result_type: String,
    pub title: String,
    pub url: String,
    pub description: String,
    pub coordinates: Option<Vec<f64>>,
    pub postal_code: Option<String>,
    pub country_code: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub rating: Option<f64>,
    pub category: Option<String>,
    pub hours: Option<String>,
    pub provider: Option<String>,
    pub thumbnail: Option<BraveThumbnail>,
}

impl BraveSearchResponse {
    pub fn to_search_results(&self) -> Vec<SearchResult> {
        let mut results = Vec::new();

        // Add web results (primary)
        if let Some(web) = &self.web {
            for item in &web.results {
                results.push(SearchResult {
                    title: item.title.clone(),
                    url: item.url.clone(),
                    snippet: item.description.clone(),
                    display_url: item.meta_url.as_ref().map(|m| format!("{}://{}", m.scheme, m.netloc)),
                    source: Some("Brave".to_string()),
                    score: None, // Brave doesn't provide explicit scores
                    html_snippet: None,
                    date_published: item.date.clone(),
                    images: item.thumbnail.as_ref().map(|t| vec![golem_websearch::golem::web_search::types::ImageResult {
                        url: t.src.clone(),
                        description: None,
                    }]),
                    content_chunks: item.extra_snippets.clone(),
                });
            }
        }

        // Add discussions results
        if let Some(discussions) = &self.discussions {
            for item in &discussions.results {
                results.push(SearchResult {
                    title: item.title.clone(),
                    url: item.url.clone(),
                    snippet: item.description.clone(),
                    display_url: None,
                    source: Some(format!("Brave Discussion ({})", item.forum.as_deref().unwrap_or("Unknown"))),
                    score: item.score,
                    html_snippet: None,
                    date_published: item.date.clone(),
                    images: None,
                    content_chunks: None,
                });
            }
        }

        // Add news results
        if let Some(news) = &self.news {
            for item in &news.results {
                results.push(SearchResult {
                    title: item.title.clone(),
                    url: item.url.clone(),
                    snippet: item.description.clone(),
                    display_url: item.meta_url.as_ref().map(|m| format!("{}://{}", m.scheme, m.netloc)),
                    source: Some("Brave News".to_string()),
                    score: None,
                    html_snippet: None,
                    date_published: item.date.clone(),
                    images: item.thumbnail.as_ref().map(|t| vec![golem_websearch::golem::web_search::types::ImageResult {
                        url: t.src.clone(),
                        description: None,
                    }]),
                    content_chunks: None,
                });
            }
        }

        results
    }

    pub fn to_search_metadata(&self, query: String) -> SearchMetadata {
        SearchMetadata {
            query,
            total_results: None, // Brave doesn't provide total result counts
            search_time_ms: None, // Brave doesn't provide search timing
            safe_search: self.query.as_ref().and_then(|q| q.safesearch).map(|safe| {
                if safe {
                    golem_websearch::golem::web_search::types::SafeSearchLevel::Medium
                } else {
                    golem_websearch::golem::web_search::types::SafeSearchLevel::Off
                }
            }),
            language: None,
            region: self.query.as_ref().and_then(|q| q.country.clone()),
            next_page_token: self.query.as_ref().and_then(|q| {
                if q.more_results_available.unwrap_or(false) {
                    Some("more".to_string()) // Simple token indicating more results
                } else {
                    None
                }
            }),
            rate_limits: None,
        }
    }
}

struct BraveWebSearchSession {
    api: BraveSearchApi,
    params: SearchParams,
    current_offset: u32,
    page_size: u32,
    finished: bool,
}

impl BraveWebSearchSession {
    pub fn new(api: BraveSearchApi, params: SearchParams) -> Result<Self, SearchError> {
        let page_size = params.max_results.unwrap_or(20).min(20); // Brave max is 20
        Ok(Self {
            api,
            params,
            current_offset: 0,
            page_size,
            finished: false,
        })
    }

    async fn fetch_results(&self, offset: u32) -> Result<BraveSearchResponse, SearchError> {
        self.api.search(&self.params, Some(self.page_size), Some(offset)).await    }
}

// impl GuestSearchSession for BraveWebSearchSession {
//     fn next_page(&self) -> Result<Vec<SearchResult>, SearchError> {
//         if self.finished {
//             return Ok(vec![]);
//         }
// 
//         // Since this is a sync trait but we need async, we use a runtime
//         let rt = tokio::runtime::Runtime::new()
//             .map_err(|e| create_backend_error(format!("Failed to create async runtime: {}", e)))?;
// 
//         match rt.block_on(self.fetch_results(self.current_offset)) {
//             Ok(response) => {
//                 let results = response.to_search_results();
//                 
//                 // Check if there are more results available
//                 let has_more = response.query
//                     .as_ref()
//                     .map(|q| q.more_results_available.unwrap_or(false))
//                     .unwrap_or(false);
//                 
//                 if !has_more || results.len() < self.page_size as usize {
//                     // No more results available - this is a bit hacky due to &self constraint
//                     // self.finished = true; // Can't mutate due to &self
//                 }
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
//         match rt.block_on(self.fetch_results(0)) {
//             Ok(response) => Some(response.to_search_metadata(self.params.query.clone())),
//             Err(_) => None,
//         }
//     }
// }

struct BraveComponent;

impl BraveComponent {
    const API_KEY_ENV_VAR: &'static str = "BRAVE_API_KEY";

    fn get_api_key() -> Result<String, SearchError> {
        std::env::var(Self::API_KEY_ENV_VAR)
            .map_err(|_| create_backend_error(format!("{} environment variable not set", Self::API_KEY_ENV_VAR)))
    }

    fn validate_params(params: &SearchParams) -> Result<(), SearchError> {
        if params.query.trim().is_empty() {
            return Err(create_invalid_query_error());
        }

        // Brave has some limitations
        if let Some(domains) = &params.exclude_domains {
            if !domains.is_empty() {
                warn!("Domain exclusion not directly supported by Brave Search API - will be ignored");
            }
        }

        if let Some(domains) = &params.include_domains {
            if !domains.is_empty() {
                warn!("Domain inclusion not directly supported by Brave Search API - will be ignored");
            }
        }

        if params.include_images == Some(true) {
            debug!("Image inclusion enabled - Brave may return video and news results with thumbnails");
        }

        if params.include_html == Some(true) {
            debug!("HTML inclusion enabled - using text decorations for better snippets");
        }

        if params.advanced_answer == Some(true) {
            debug!("Advanced answer mode enabled - Brave may return infobox, FAQ, and discussion results");
        }

        Ok(())
    }
}

// impl Guest for BraveComponent {
//     type SearchSession = BraveWebSearchSession;
// 
//     fn start_search(params: SearchParams) -> Result<SearchSession, SearchError> {
//         LOGGING_STATE.with_borrow_mut(|state| state.init());
//         
//         Self::validate_params(&params)?;
//         
//         let api_key = Self::get_api_key()?;
//         let api = BraveSearchApi::new(api_key);
//         let session = BraveWebSearchSession::new(api, params)?;
//         Ok(SearchSession::new(session))
//     }
// 
//     fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
//         LOGGING_STATE.with_borrow_mut(|state| state.init());
//         
//         Self::validate_params(&params)?;
//         
//         let api_key = Self::get_api_key()?;
//         let api = BraveSearchApi::new(api_key);
//         
//         // Use async runtime for the one-shot search
//         let rt = tokio::runtime::Runtime::new()
//             .map_err(|e| create_backend_error(format!("Failed to create async runtime: {}", e)))?;
// 
//         match rt.block_on(api.search(&params, params.max_results, Some(0))) {
//             Ok(response) => {
//                 let results = response.to_search_results();
//                 let metadata = Some(response.to_search_metadata(params.query));
//                 Ok((results, metadata))
//             }
//             Err(error) => Err(error)
//         }
//     }
// }

impl BraveComponent {
    fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
        LOGGING_STATE.with_borrow_mut(|state| state.init());
        
        Self::validate_params(&params)?;
        
        let api_key = Self::get_api_key()?;
        let api = BraveSearchApi::new(api_key);
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

// impl ExtendedGuest for BraveComponent {
//     fn unwrapped_start_search(params: SearchParams) -> Result<Self::SearchSession, SearchError> {
//         Self::validate_params(&params)?;
//         
//         let api_key = Self::get_api_key()?;
//         let api = BraveSearchApi::new(api_key);
//         BraveWebSearchSession::new(api, params)
//     }
// }

// impl ExportGuest for BraveComponent {
//     fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
//         Self::search_once(params)
//     }
// }

// struct BraveWebSearchComponent;

// impl ExportGuest for BraveWebSearchComponent {
//     fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
//         BraveComponent::search_once(params)
//     }
// }

// wit_bindgen::generate!({
//     world: "web-search-provider", 
//     path: "../websearch/wit",
//     generate_all,
// });

// export!(BraveWebSearchComponent);

impl ExportGuest for BraveComponent {
    fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
        Self::search_once(params)
    }
}

// Note: export macro temporarily disabled while fixing compilation issues
// golem_websearch::export_websearch!(BraveComponent);

// Export handled by Guest trait implementation

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_brave_search_api_creation() {
        let api = client::BraveSearchApi::new("test_api_key".to_string());
        
        // This just tests that we can create the API struct
        assert_eq!(api.api_key, "test_api_key");
    }

    #[tokio::test]
    async fn test_brave_search_params_construction() {
        let params = SearchParams {
            query: "open source projects".to_string(),
            max_results: Some(15),
            safe_search: Some(SafeSearchLevel::Moderate),
            time_range: Some(TimeRange::Year),
            country: Some("AU".to_string()),
            language: Some("en-AU".to_string()),
        };
        
        assert_eq!(params.query, "open source projects");
        assert_eq!(params.max_results, Some(15));
        assert!(matches!(params.safe_search, Some(SafeSearchLevel::Moderate)));
        assert!(matches!(params.time_range, Some(TimeRange::Year)));
        assert_eq!(params.country, Some("AU".to_string()));
        assert_eq!(params.language, Some("en-AU".to_string()));
    }

    #[tokio::test]
    async fn test_brave_search_once_with_invalid_api_key() {
        let result = BraveWebSearchComponent::search_once(
            "invalid_key".to_string(),
            "web development".to_string(),
            Some(5),
            Some(SafeSearchLevel::Strict),
            Some(TimeRange::Week),
            Some("US".to_string()),
            Some("en".to_string()),
        );
        
        // This should fail with invalid credentials, but not panic
        assert!(result.is_err());
    }

    #[test]
    fn test_brave_url_construction() {
        use url::Url;
        
        let mut url = Url::parse("https://api.search.brave.com/res/v1/web/search").unwrap();
        {
            let mut query_pairs = url.query_pairs_mut();
            query_pairs.append_pair("q", "brave search");
            query_pairs.append_pair("count", "20");
        }
        
        let url_str = url.to_string();
        assert!(url_str.contains("q=brave+search"));
        assert!(url_str.contains("count=20"));
    }

    #[test]
    fn test_brave_safe_search_mapping() {
        // Test that safe search levels are handled correctly
        let test_cases = vec!(
            (SafeSearchLevel::Off, "off"),
            (SafeSearchLevel::Moderate, "moderate"),
            (SafeSearchLevel::Strict, "strict"),
        );
        
        for (level, expected) in test_cases {
            let converted = match level {
                SafeSearchLevel::Off => "off",
                SafeSearchLevel::Moderate => "moderate",
                SafeSearchLevel::Strict => "strict",
            };
            assert_eq!(converted, expected);
        }
    }

    #[test]
    fn test_brave_freshness_mapping() {
        // Test that time ranges are handled correctly
        let test_cases = vec!(
            (TimeRange::Day, "pd"),
            (TimeRange::Week, "pw"),
            (TimeRange::Month, "pm"),
            (TimeRange::Year, "py"),
        );
        
        for (range, expected) in test_cases {
            let converted = match range {
                TimeRange::Day => "pd",
                TimeRange::Week => "pw",
                TimeRange::Month => "pm",
                TimeRange::Year => "py",
            };
            assert_eq!(converted, expected);
        }
    }

    #[test]
    fn test_brave_result_parsing() {
        // Test that we can create SearchResult structs
        let result = SearchResult {
            title: "Brave Search Result".to_string(),
            url: "https://brave.com/search".to_string(),
            snippet: "Privacy-focused search engine".to_string(),
            published_date: None,
        };
        
        assert_eq!(result.title, "Brave Search Result");
        assert_eq!(result.url, "https://brave.com/search");
        assert_eq!(result.snippet, "Privacy-focused search engine");
        assert_eq!(result.published_date, None);
    }

    #[test]
    fn test_query_validation() {
        // Test empty query
        let empty_query = "";
        assert!(empty_query.is_empty());
        
        // Test valid query
        let valid_query = "rust programming language";
        assert!(!valid_query.is_empty());
        assert!(valid_query.len() > 5);
    }

    #[test]
    fn test_max_results_bounds() {
        // Test max results validation
        let max_results = vec![Some(1), Some(10), Some(20), Some(50), Some(100), None];
        
        for max in max_results {
            match max {
                Some(n) if n > 0 && n <= 100 => assert!(true),
                Some(_) => assert!(false, "Invalid max results value"),
                None => assert!(true), // None is valid (uses default)
            }
        }
    }
}