use golem_websearch::error::{create_backend_error, http_status_to_search_error};
use golem_websearch::golem::web_search::types::{SearchError, SearchParams, SearchResult, SearchMetadata, TimeRange, SafeSearchLevel};
use log::trace;
use reqwest::{Client, Method, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

const BASE_URL: &str = "https://www.googleapis.com/customsearch/v1";

/// Google Custom Search API client
pub struct GoogleSearchApi {
    api_key: String,
    cx: String, // Custom Search Engine ID
    client: Client,
}

impl GoogleSearchApi {
    pub fn new(api_key: String, cx: String) -> Self {
        let client = Client::builder()
            .build()
            .expect("Failed to initialize HTTP client");
        Self {
            api_key,
            cx,
            client,
        }
    }

    pub fn search(&self, params: &SearchParams, start: Option<u32>) -> Result<GoogleSearchResponse, SearchError> {
        trace!("Sending request to Google Custom Search API with query: {}", params.query);

        let mut url = Url::parse(BASE_URL).map_err(|e| create_backend_error(format!("Invalid URL: {}", e)))?;        // Build the final query with domain filtering if needed
        let final_query = if let Some(include_domains) = &params.include_domains {
            if !include_domains.is_empty() {
                let site_query = include_domains.iter()
                    .map(|domain| format!("site:{}", domain))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                format!("{} ({})", params.query, site_query)
            } else {
                params.query.clone()
            }
        } else {
            params.query.clone()
        };

        // Build query parameters
        let mut query_params = Vec::new();
        query_params.push(("key", self.api_key.as_str()));
        query_params.push(("cx", self.cx.as_str()));
        query_params.push(("q", final_query.as_str()));

        let start_str;
        if let Some(start) = start {
            start_str = start.to_string();
            query_params.push(("start", &start_str));
        }

        let num_str;
        if let Some(max_results) = params.max_results {
            let num = std::cmp::min(max_results, 10); // Google CSE max is 10 per request
            num_str = num.to_string();
            query_params.push(("num", &num_str));
        }

        let lang_str;
        if let Some(language) = &params.language {
            lang_str = format!("lang_{}", language);
            query_params.push(("lr", &lang_str));
        }

        if let Some(region) = &params.region {
            query_params.push(("gl", region));
        }

        if let Some(safe_search) = &params.safe_search {
            let safe_value = match safe_search {
                SafeSearchLevel::Off => "off",
                SafeSearchLevel::Medium => "medium",
                SafeSearchLevel::High => "high",
            };
            query_params.push(("safe", safe_value));
        }        if let Some(time_range) = &params.time_range {
            let sort_value = match time_range {
                TimeRange::Day => "date:r:86400", // Last 24 hours
                TimeRange::Week => "date:r:604800", // Last week
                TimeRange::Month => "date:r:2592000", // Last 30 days
                TimeRange::Year => "date:r:31536000", // Last year
            };
            query_params.push(("sort", sort_value));
        }

        url.query_pairs_mut().extend_pairs(query_params);        // Use futures executor for WASM compatibility
        let response: Response = futures::executor::block_on(async {
            self.client
                .request(Method::GET, url)
                .send()
                .await
        }).map_err(|err| create_backend_error(format!("Request failed: {}", err)))?;

        self.parse_response(response)
    }    fn parse_response(&self, response: Response) -> Result<GoogleSearchResponse, SearchError> {
        let status = response.status().as_u16();
        
        if !response.status().is_success() {
            return Err(http_status_to_search_error(status, "Google API error"));
        }        let text = futures::executor::block_on(async {
            response.text().await
        }).map_err(|err| create_backend_error(format!("Failed to read response: {}", err)))?;

        serde_json::from_str::<GoogleSearchResponse>(&text)
            .map_err(|err| create_backend_error(format!("Failed to parse response: {}", err)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSearchResponse {
    pub kind: String,
    pub url: Option<GoogleUrl>,
    pub queries: Option<GoogleQueries>,
    pub context: Option<GoogleContext>,
    #[serde(rename = "searchInformation")]
    pub search_information: Option<GoogleSearchInformation>,
    pub items: Option<Vec<GoogleSearchItem>>,
    pub error: Option<GoogleError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleUrl {
    #[serde(rename = "type")]
    pub url_type: String,
    pub template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleQueries {
    pub request: Option<Vec<GoogleQuery>>,
    #[serde(rename = "nextPage")]
    pub next_page: Option<Vec<GoogleQuery>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleQuery {
    pub title: String,
    #[serde(rename = "totalResults")]
    pub total_results: String,
    #[serde(rename = "searchTerms")]
    pub search_terms: String,
    pub count: Option<u32>,
    #[serde(rename = "startIndex")]
    pub start_index: Option<u32>,
    #[serde(rename = "inputEncoding")]
    pub input_encoding: Option<String>,
    #[serde(rename = "outputEncoding")]
    pub output_encoding: Option<String>,
    pub safe: Option<String>,
    pub cx: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleContext {
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSearchInformation {
    #[serde(rename = "searchTime")]
    pub search_time: f64,
    #[serde(rename = "formattedSearchTime")]
    pub formatted_search_time: String,
    #[serde(rename = "totalResults")]
    pub total_results: String,
    #[serde(rename = "formattedTotalResults")]
    pub formatted_total_results: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleSearchItem {
    pub kind: String,
    pub title: String,
    #[serde(rename = "htmlTitle")]
    pub html_title: String,
    pub link: String,
    #[serde(rename = "displayLink")]
    pub display_link: String,
    pub snippet: String,
    #[serde(rename = "htmlSnippet")]
    pub html_snippet: String,
    #[serde(rename = "cacheId")]
    pub cache_id: Option<String>,
    #[serde(rename = "formattedUrl")]
    pub formatted_url: Option<String>,
    #[serde(rename = "htmlFormattedUrl")]
    pub html_formatted_url: Option<String>,
    pub pagemap: Option<HashMap<String, Vec<serde_json::Value>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleError {
    pub code: u32,
    pub message: String,
    pub errors: Option<Vec<GoogleErrorDetail>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleErrorDetail {
    pub message: String,
    pub domain: String,
    pub reason: String,
}

impl GoogleSearchResponse {
    pub fn to_search_results(&self) -> Vec<SearchResult> {
        if let Some(items) = &self.items {
            items.iter().map(|item| SearchResult {
                title: item.title.clone(),
                url: item.link.clone(),
                snippet: item.snippet.clone(),
                display_url: Some(item.display_link.clone()),
                source: Some("Google".to_string()),
                score: None, // Google doesn't provide relevance scores
                html_snippet: Some(item.html_snippet.clone()),
                date_published: None, // Would need to parse from pagemap if available
                images: None, // Would need to extract from pagemap
                content_chunks: None,
            }).collect()
        } else {
            vec![]
        }
    }

    pub fn to_search_metadata(&self, query: String) -> SearchMetadata {
        let (total_results, search_time) = if let Some(info) = &self.search_information {
            (
                info.total_results.parse().ok(),
                Some(info.search_time * 1000.0) // Convert to milliseconds
            )
        } else {
            (None, None)
        };

        let next_page_token = self.queries
            .as_ref()
            .and_then(|q| q.next_page.as_ref())
            .and_then(|np| np.first())
            .and_then(|np| np.start_index)
            .map(|start| start.to_string());

        SearchMetadata {
            query,
            total_results,
            search_time_ms: search_time,
            safe_search: None, // Would need to extract from request
            language: None,    // Would need to extract from request
            region: None,      // Would need to extract from request
            next_page_token,
            rate_limits: None, // Google doesn't provide rate limit info in response
        }
    }
} 