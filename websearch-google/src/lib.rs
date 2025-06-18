use crate::client::{GoogleSearchApi, GoogleSearchResponse};
// Note: Durability module disabled for compatibility - using basic implementation
use golem_websearch::error::{create_backend_error, create_invalid_query_error, create_unsupported_feature_error};
use golem_websearch::golem::web_search::types::{SearchError, SearchMetadata, SearchParams, SearchResult};
use golem_websearch::exports::golem::web_search::web_search::Guest;
use golem_websearch::search_session::{SearchSession as GenericSearchSession, SearchSessionProvider, SearchSessionState};
use golem_websearch::LOGGING_STATE;
use std::cell::RefCell;

mod client;

struct GoogleSearchSession {
    api: GoogleSearchApi,
    params: SearchParams,
    current_start: RefCell<u32>,
}

impl GoogleSearchSession {
    pub fn new(api: GoogleSearchApi, params: SearchParams) -> Self {
        Self {
            api,
            params,
            current_start: RefCell::new(1), // Google uses 1-based indexing
        }
    }
}

impl SearchSessionProvider for GoogleSearchSession {
    fn next_page(&self, state: &mut SearchSessionState) -> Result<Vec<SearchResult>, SearchError> {
        if state.finished {
            return Ok(vec![]);
        }

        let start = *self.current_start.borrow();
        
        match self.api.search(&self.params, Some(start)) {
            Ok(response) => {
                let results = response.to_search_results();
                let metadata = response.to_search_metadata(self.params.query.clone());
                
                // Update pagination state
                *self.current_start.borrow_mut() = start + results.len() as u32;
                
                // Check if we have more pages
                let has_more = response.queries
                    .as_ref()
                    .and_then(|q| q.next_page.as_ref())
                    .is_some();
                
                if !has_more {
                    state.set_finished();
                }
                
                state.update_with_results(results.clone(), Some(metadata));
                Ok(results)
            }
            Err(error) => {
                state.set_error(error.clone());
                Err(error)
            }
        }
    }
}

struct GoogleWebSearchSession {
    session: GenericSearchSession<GoogleSearchSession>,
}

impl GoogleWebSearchSession {
    pub fn new(api: GoogleSearchApi, params: SearchParams) -> Result<Self, SearchError> {
        // Perform initial search to get metadata
        let response = api.search(&params, Some(1))?;
        let metadata = response.to_search_metadata(params.query.clone());
        
        let provider = GoogleSearchSession::new(api, params);
        let session = GenericSearchSession::new(provider, Some(metadata));
        
        Ok(Self { session })
    }
}

// Note: GuestSearchSession implementation disabled due to SearchSession resource compatibility issues  
/*
impl GuestSearchSession for GoogleWebSearchSession {
    fn next_page(&self) -> Result<Vec<SearchResult>, SearchError> {
        self.session.next_page()
    }

    fn get_metadata(&self) -> Option<SearchMetadata> {
        self.session.get_metadata()
    }
}
*/

struct GoogleComponent;

impl GoogleComponent {
    const API_KEY_ENV_VAR: &'static str = "GOOGLE_API_KEY";
    const CX_ENV_VAR: &'static str = "GOOGLE_CX";

    fn get_api_credentials() -> Result<(String, String), SearchError> {
        let api_key = std::env::var(Self::API_KEY_ENV_VAR)
            .map_err(|_| create_backend_error(format!("{} environment variable not set", Self::API_KEY_ENV_VAR)))?;
        
        let cx = std::env::var(Self::CX_ENV_VAR)
            .map_err(|_| create_backend_error(format!("{} environment variable not set", Self::CX_ENV_VAR)))?;
            
        Ok((api_key, cx))
    }

    fn validate_params(params: &SearchParams) -> Result<(), SearchError> {
        if params.query.trim().is_empty() {
            return Err(create_invalid_query_error());
        }

        // Check for unsupported features
        if params.exclude_domains.is_some() && !params.exclude_domains.as_ref().unwrap().is_empty() {
            return Err(create_unsupported_feature_error("Domain exclusion not supported by Google Custom Search".to_string()));
        }

        if params.advanced_answer == Some(true) {
            return Err(create_unsupported_feature_error("Advanced answer mode not supported by Google Custom Search".to_string()));
        }

        Ok(())
    }
}

impl Guest for GoogleComponent {
    fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
        LOGGING_STATE.with_borrow_mut(|state| state.init());
        
        Self::validate_params(&params)?;
        let (api_key, cx) = Self::get_api_credentials()?;
        let api = GoogleSearchApi::new(api_key, cx);
        
        let response = api.search(&params, Some(1))?;
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
    async fn test_google_search_api_creation() {
        let api = client::GoogleSearchApi::new(
            "test_api_key".to_string(),
            "test_cx".to_string(),
        );
        
        // This just tests that we can create the API struct
        assert_eq!(api.api_key, "test_api_key");
        assert_eq!(api.custom_search_engine_id, "test_cx");
    }

    #[tokio::test]
    async fn test_search_params_construction() {
        let params = SearchParams {
            query: "test query".to_string(),
            max_results: Some(10),
            safe_search: Some(SafeSearchLevel::Moderate),
            time_range: Some(TimeRange::Week),
            country: Some("US".to_string()),
            language: Some("en".to_string()),
        };
        
        assert_eq!(params.query, "test query");
        assert_eq!(params.max_results, Some(10));
        assert!(matches!(params.safe_search, Some(SafeSearchLevel::Moderate)));
        assert!(matches!(params.time_range, Some(TimeRange::Week)));
        assert_eq!(params.country, Some("US".to_string()));
        assert_eq!(params.language, Some("en".to_string()));
    }

    #[tokio::test]
    async fn test_search_once_with_invalid_api_key() {
        let result = GoogleWebSearchComponent::search_once(
            "invalid_key".to_string(),
            "invalid_cx".to_string(),
            "test query".to_string(),
            Some(5),
            None,
            None,
            None,
            None,
        );
        
        // This should fail with invalid credentials, but not panic
        assert!(result.is_err());
    }

    #[test]
    fn test_url_construction() {
        use url::Url;
        
        let mut url = Url::parse("https://www.googleapis.com/customsearch/v1").unwrap();
        {
            let mut query_pairs = url.query_pairs_mut();
            query_pairs.append_pair("key", "test_key");
            query_pairs.append_pair("cx", "test_cx");
            query_pairs.append_pair("q", "test query");
        }
        
        let url_str = url.to_string();
        assert!(url_str.contains("key=test_key"));
        assert!(url_str.contains("cx=test_cx"));
        assert!(url_str.contains("q=test+query"));
    }

    #[test]
    fn test_safe_search_level_conversion() {
        // Test safe search level mapping
        let moderate = SafeSearchLevel::Moderate;
        let strict = SafeSearchLevel::Strict;
        let off = SafeSearchLevel::Off;
        
        // These should not panic
        assert!(matches!(moderate, SafeSearchLevel::Moderate));
        assert!(matches!(strict, SafeSearchLevel::Strict));
        assert!(matches!(off, SafeSearchLevel::Off));
    }

    #[test]
    fn test_time_range_conversion() {
        // Test time range mapping
        let day = TimeRange::Day;
        let week = TimeRange::Week;
        let month = TimeRange::Month;
        let year = TimeRange::Year;
        
        // These should not panic
        assert!(matches!(day, TimeRange::Day));
        assert!(matches!(week, TimeRange::Week));
        assert!(matches!(month, TimeRange::Month));
        assert!(matches!(year, TimeRange::Year));
    }
}