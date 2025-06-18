# Golem Web Search API

This directory contains the implementation of the Durable Web Search `golem:web-search` API for Golem Cloud. The implementation provides a unified interface for multiple web search providers with built-in durability and error handling.

## Architecture

The web search implementation follows the same pattern as the LLM library:

- **Main Library** (`websearch/`): Core types, durability wrapper, and shared utilities
- **Provider Libraries**: Individual implementations for each search provider
  - `websearch-google/`: Google Custom Search API
  - `websearch-bing/`: Microsoft Bing Web Search API  
  - `websearch-brave/`: Brave Search API
  - `websearch-tavily/`: Tavily Search API
  - `websearch-serper/`: Serper Search API

## WIT Interface

The web search API is defined in `golem-web-search.wit` and includes:

### Core Types

- `search-result`: Contains title, URL, snippet, and metadata for a single result
- `search-metadata`: Session information including total results, timing, and pagination tokens
- `search-params`: Query parameters including filters, language, region, and limits
- `search-error`: Structured error types for different failure modes

### Main Interface

- `search-once`: Performs a one-shot search returning results immediately
- `start-search`: Initiates a search session for pagination support

### Search Session Resource

- `next-page`: Retrieves the next page of results
- `get-metadata`: Returns session metadata

## Provider Implementations

### Google Custom Search

**Environment Variables:**
- `GOOGLE_API_KEY`: Google Cloud API key
- `GOOGLE_CX`: Custom Search Engine ID

**Features:**
- ✅ Basic search with snippets
- ✅ Language and region filtering  
- ✅ Safe search levels
- ✅ Domain inclusion filtering
- ✅ Time range filtering
- ✅ Pagination support
- ❌ Domain exclusion (not supported by Google CSE)
- ❌ Advanced answer mode

### Microsoft Bing Web Search

**Environment Variables:**
- `BING_API_KEY`: Azure Cognitive Services API key

**Features:**
- ✅ Basic search with snippets
- ✅ Language and region filtering
- ✅ Safe search levels
- ✅ Pagination support
- ❌ Time range filtering
- ❌ Domain filtering
- ❌ Advanced answer mode

### Brave Search (Planned)

**Environment Variables:**
- `BRAVE_API_KEY`: Brave Search API key

### Tavily Search (Planned)

**Environment Variables:**
- `TAVILY_API_KEY`: Tavily API key

### Serper Search (Planned)

**Environment Variables:**
- `SERPER_API_KEY`: Serper API key

## Durability

All providers support Golem's durability features:

- **Function-level durability**: Search requests and responses are persisted in the oplog
- **Session durability**: Search sessions can be recovered after interruptions
- **Automatic retry**: Failed requests are automatically retried with backoff
- **State management**: Pagination state is preserved across restarts

## Usage Examples

### One-shot Search

```rust
use golem_websearch::golem::web_search::types::{SearchParams, SafeSearchLevel};
use golem_websearch::golem::web_search::web_search::Guest;

let params = SearchParams {
    query: "rust programming language".to_string(),
    safe_search: Some(SafeSearchLevel::Medium),
    language: Some("en".to_string()),
    region: Some("US".to_string()),
    max_results: Some(10),
    time_range: None,
    include_domains: None,
    exclude_domains: None,
    include_images: Some(false),
    include_html: Some(false),
    advanced_answer: Some(false),
};

let (results, metadata) = Component::search_once(params)?;
```

### Paginated Search

```rust
let session = Component::start_search(params)?;

// Get first page
let page1 = session.next_page()?;

// Get metadata
let metadata = session.get_metadata();

// Get subsequent pages
let page2 = session.next_page()?;
```

## Error Handling

The API uses structured error types:

- `InvalidQuery`: Empty or malformed search query
- `RateLimited(u32)`: Request rate limited, retry after N seconds
- `UnsupportedFeature(String)`: Provider doesn't support requested feature
- `BackendError(String)`: API or network error

## Development

### Building

```bash
# Build all providers
cargo build --release

# Build specific provider
cargo build -p websearch-google --release
```

### Testing

```bash
# Run tests for core library
cargo test -p golem-websearch

# Test specific provider (requires API keys)
GOOGLE_API_KEY=... GOOGLE_CX=... cargo test -p websearch-google
```

### Creating WASM Components

```bash
# Install cargo-component
cargo install cargo-component

# Build WASM component
cargo component build --release -p websearch-google
```

## Configuration

Each provider requires specific API credentials set as environment variables. See the provider-specific sections above for required variables.

### Logging

Set `GOLEM_WEBSEARCH_LOG` to control log levels:
- `error`: Only errors
- `warn`: Warnings and errors  
- `info`: General information (default)
- `debug`: Detailed debugging
- `trace`: Very verbose logging

## Rate Limits

Each provider has different rate limits:

- **Google CSE**: 100 queries/day (free tier), paid tiers available
- **Bing**: Varies by subscription tier
- **Brave**: 2000 queries/month (free tier)
- **Tavily**: Varies by plan
- **Serper**: 2500 queries/month (free tier)

The API automatically handles rate limiting and provides retry-after information when available.

## Contributing

When adding new providers:

1. Create a new crate following the naming pattern `websearch-{provider}`
2. Implement the `Guest` and `ExtendedGuest` traits
3. Add proper error handling and parameter validation
4. Include comprehensive tests
5. Update this README with provider-specific information
6. Add the new crate to the workspace `Cargo.toml`

## License

This implementation is part of the Golem Cloud project and follows the same licensing terms. 