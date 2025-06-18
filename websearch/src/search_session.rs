use crate::golem::web_search::types::{SearchError, SearchMetadata, SearchResult};
use std::cell::RefCell;

/// State for managing search sessions
pub struct SearchSessionState {
    pub metadata: Option<SearchMetadata>,
    pub next_page_token: Option<String>,
    pub finished: bool,
    pub error: Option<SearchError>,
}

impl SearchSessionState {
    pub fn new() -> Self {
        Self {
            metadata: None,
            next_page_token: None,
            finished: false,
            error: None,
        }
    }

    pub fn with_metadata(metadata: SearchMetadata) -> Self {
        let next_page_token = metadata.next_page_token.clone();
        Self {
            metadata: Some(metadata),
            next_page_token,
            finished: false,
            error: None,
        }
    }

    pub fn set_error(&mut self, error: SearchError) {
        self.error = Some(error);
        self.finished = true;
    }

    pub fn set_finished(&mut self) {
        self.finished = true;
        self.next_page_token = None;
    }

    pub fn update_with_results(&mut self, results: Vec<SearchResult>, metadata: Option<SearchMetadata>) {
        if let Some(metadata) = metadata {
            self.next_page_token = metadata.next_page_token.clone();
            self.metadata = Some(metadata);
        }
        
        if self.next_page_token.is_none() {
            self.finished = true;
        }
    }

    pub fn has_more_pages(&self) -> bool {
        !self.finished && self.next_page_token.is_some()
    }
}

/// Generic search session trait that providers can implement
pub trait SearchSessionProvider {
    fn next_page(&self, state: &mut SearchSessionState) -> Result<Vec<SearchResult>, SearchError>;
}

/// Container for search sessions with state management
pub struct SearchSession<T: SearchSessionProvider> {
    provider: T,
    state: RefCell<SearchSessionState>,
}

impl<T: SearchSessionProvider> SearchSession<T> {
    pub fn new(provider: T, metadata: Option<SearchMetadata>) -> Self {
        let state = if let Some(metadata) = metadata {
            SearchSessionState::with_metadata(metadata)
        } else {
            SearchSessionState::new()
        };

        Self {
            provider,
            state: RefCell::new(state),
        }
    }

    pub fn next_page(&self) -> Result<Vec<SearchResult>, SearchError> {
        let mut state = self.state.borrow_mut();
        
        if state.finished {
            return Ok(vec![]);
        }

        if let Some(error) = &state.error {
            return Err(error.clone());
        }

        self.provider.next_page(&mut state)
    }

    pub fn get_metadata(&self) -> Option<SearchMetadata> {
        self.state.borrow().metadata.clone()
    }

    pub fn is_finished(&self) -> bool {
        self.state.borrow().finished
    }
} 