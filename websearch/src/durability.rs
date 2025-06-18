use crate::golem::web_search::types::{SearchError, SearchParams};
// Note: SearchSession-related types are disabled due to WIT resource compatibility issues
// use crate::golem::web_search::web_search::{Guest, GuestSearchSession, SearchSession};
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;

/// Wraps a WebSearch implementation with custom durability
pub struct DurableWebSearch<Impl> {
    phantom: PhantomData<Impl>,
}

/// Trait to be implemented in addition to the WebSearch `Guest` trait when wrapping it with `DurableWebSearch`.
pub trait ExtendedGuest: Guest + 'static {
    /// Creates an instance of the provider-specific `SearchSession` without wrapping it in a `Resource`
    fn unwrapped_start_search(params: SearchParams) -> Result<Self::SearchSession, SearchError>;
}

/// When the durability feature flag is off, wrapping with `DurableWebSearch` is just a passthrough
#[cfg(not(feature = "durability"))]
mod passthrough_impl {
    use crate::durability::{DurableWebSearch, ExtendedGuest};
    use crate::golem::web_search::types::{SearchError, SearchMetadata, SearchParams, SearchResult};
    use crate::golem::web_search::web_search::{Guest, SearchSession};

    impl<Impl: ExtendedGuest> Guest for DurableWebSearch<Impl> {
        type SearchSession = Impl::SearchSession;

        fn start_search(params: SearchParams) -> Result<SearchSession, SearchError> {
            Impl::start_search(params)
        }

        fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
            Impl::search_once(params)
        }
    }
}

/// When the durability feature flag is on, wrapping with `DurableWebSearch` adds custom durability
/// on top of the provider-specific WebSearch implementation using Golem's special host functions and
/// the `golem-rust` helper library.
#[cfg(feature = "durability")]
mod durable_impl {
    use crate::durability::{DurableWebSearch, ExtendedGuest};    use crate::golem::web_search::types::{SearchError, SearchParams};
    // Note: SearchSession-related types are disabled due to WIT resource compatibility issues
    // use crate::golem::web_search::web_search::{Guest, GuestSearchSession, SearchSession};
    use golem_rust::bindings::golem::durability::durability::DurableFunctionType;
    use golem_rust::durability::Durability;
    use golem_rust::{with_persistence_level, FromValueAndType, IntoValue, PersistenceLevel};
    use std::cell::RefCell;
    use std::fmt::{Display, Formatter};

    impl<Impl: ExtendedGuest> Guest for DurableWebSearch<Impl> {
        type SearchSession = DurableSearchSession<Impl>;

        fn start_search(params: SearchParams) -> Result<SearchSession, SearchError> {
            let durability = Durability::<SearchSession, SearchError>::new(
                "golem_websearch",
                "start_search",
                DurableFunctionType::WriteRemote,
            );
            if durability.is_live() {
                let result = with_persistence_level(PersistenceLevel::PersistNothing, || {
                    match Impl::unwrapped_start_search(params.clone()) {
                        Ok(session) => Ok(SearchSession::new(DurableSearchSession::<Impl>::live(session))),
                        Err(error) => Err(error),
                    }
                });
                durability.persist(StartSearchInput { params }, result)
            } else {
                durability.replay()
            }
        }

        fn search_once(params: SearchParams) -> Result<(Vec<SearchResult>, Option<SearchMetadata>), SearchError> {
            let durability = Durability::<(Vec<SearchResult>, Option<SearchMetadata>), SearchError>::new(
                "golem_websearch",
                "search_once",
                DurableFunctionType::WriteRemote,
            );
            if durability.is_live() {
                let result = with_persistence_level(PersistenceLevel::PersistNothing, || {
                    Impl::search_once(params.clone())
                });
                durability.persist(SearchOnceInput { params }, result)
            } else {
                durability.replay()
            }
        }
    }

    /// Represents the durable search session's state
    ///
    /// In live mode it directly calls the underlying provider search session.
    /// In replay mode it would handle replaying search operations.
    enum DurableSearchSessionState<Impl: ExtendedGuest> {
        Live {
            session: Impl::SearchSession,
        },
        // Future: Add replay support similar to LLM streaming
    }

    pub struct DurableSearchSession<Impl: ExtendedGuest> {
        state: RefCell<Option<DurableSearchSessionState<Impl>>>,
    }

    impl<Impl: ExtendedGuest> DurableSearchSession<Impl> {
        fn live(session: Impl::SearchSession) -> Self {
            Self {
                state: RefCell::new(Some(DurableSearchSessionState::Live { session })),
            }
        }
    }

    impl<Impl: ExtendedGuest> GuestSearchSession for DurableSearchSession<Impl> {
        fn next_page(&self) -> Result<Vec<SearchResult>, SearchError> {
            let durability = Durability::<Vec<SearchResult>, SearchError>::new(
                "golem_websearch",
                "next_page",
                DurableFunctionType::WriteRemote,
            );
            if durability.is_live() {
                let result = {
                    let mut state_guard = self.state.borrow_mut();
                    if let Some(DurableSearchSessionState::Live { session }) = state_guard.as_mut() {
                        session.next_page()
                    } else {
                        Err(SearchError::BackendError("Invalid session state".to_string()))
                    }
                };
                durability.persist(NoInput, result)
            } else {
                durability.replay()
            }
        }

        fn get_metadata(&self) -> Option<SearchMetadata> {
            let state_guard = self.state.borrow();
            if let Some(DurableSearchSessionState::Live { session }) = state_guard.as_ref() {
                session.get_metadata()
            } else {
                None
            }
        }
    }

    #[derive(Clone, PartialEq)]
    struct StartSearchInput {
        params: SearchParams,
    }

    #[derive(Clone, PartialEq)]
    struct SearchOnceInput {
        params: SearchParams,
    }

    #[derive(Clone, PartialEq)]
    struct NoInput;

    struct UnusedError;

    impl Display for UnusedError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "Unused error")
        }
    }

    // Implement the required traits for durability serialization
    impl IntoValue for StartSearchInput {
        fn into_value(self) -> golem_rust::Value {
            self.params.into_value()
        }
    }

    impl FromValueAndType for StartSearchInput {
        fn from_value_and_type(value: golem_rust::Value, _type: &golem_rust::Type) -> Result<Self, String> {
            Ok(Self {
                params: SearchParams::from_value_and_type(value, _type)?,
            })
        }
    }

    impl IntoValue for SearchOnceInput {
        fn into_value(self) -> golem_rust::Value {
            self.params.into_value()
        }
    }

    impl FromValueAndType for SearchOnceInput {
        fn from_value_and_type(value: golem_rust::Value, _type: &golem_rust::Type) -> Result<Self, String> {
            Ok(Self {
                params: SearchParams::from_value_and_type(value, _type)?,
            })
        }
    }

    impl IntoValue for NoInput {
        fn into_value(self) -> golem_rust::Value {
            golem_rust::Value::Record(vec![])
        }
    }

    impl FromValueAndType for NoInput {
        fn from_value_and_type(_value: golem_rust::Value, _type: &golem_rust::Type) -> Result<Self, String> {
            Ok(NoInput)
        }
    }
} 