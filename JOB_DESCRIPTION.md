Implement Durable Web Search golem:web-search API Across Different Providers in Rust #34

Open

#39 Open

Implement Durable Web Search golem:web-search API Across Different Providers in Rust
#34

Labels
$2.5K
💎 Bounty

Jump to bottom

Description
@jdegoes
jdegoes
opened 2 days ago

This ticket involves implementing the golem:web-search WIT interface for multiple popular web search providers. The golem:web-search interface provides a provider-agnostic and WIT-idiomatic abstraction over a wide range of web search APIs, designed to support optional parameters, provider-specific emulation, and well-typed errors for unimplemented or unsupported features.

The purpose of these implementations is to enable WASM components, running on platforms like Golem Cloud, Spin, or wasmCloud, to query real-time web data in a portable, durable, and provider-neutral way. These components will be used by durable AI agents, LLM pipelines, and serverless applications needing reliable access to web search.

Providers to Implement
The following providers must be implemented:

- Google Custom Search: Offers powerful search backed by Google, with configurable Custom Search Engines.
- Microsoft Bing Web Search: A comprehensive web search API with result decorations, HTML formatting, and localization.
- Brave Search: Privacy-centric search with region/language filtering and scoring metadata.
- Tavily: Offers deep document-level indexing and question answering capabilities.
- Serper: Simple, fast web search API with optional location/language targeting.

These implementations should:

- Be written in Rust and compiled as WASM Components using WASI 0.2 only (Golem does not yet support WASI 0.3).
- Use the cargo component toolchain.
- Fully implement the WIT interface for each provider, either:
  - Using the native features directly,
  - Emulating missing functionality where feasible,
  - Or returning unsupported-feature variant errors for unimplementable features.

Durability Requirements
Each implementation must integrate with the Golem Durability API to ensure that all search sessions and one-shot search calls are logged as durable operations:

- Durable operations must cover start-search, next-page, and search-once.
- Use custom Golem host API wrappers to provide high-level operation granularity in the log.
- Take inspiration from golem:llm and golem:embed, which model durable, query-level logging.

Deliverables
The expected deliverables are:

- websearch-google.wasm — WASM Component for Google Custom Search
- websearch-bing.wasm — WASM Component for Bing Web Search
- websearch-brave.wasm — WASM Component for Brave Search
- websearch-tavily.wasm — WASM Component for Tavily
- websearch-serper.wasm — WASM Component for Serper

Each deliverable should include:

- A complete implementation of the golem:web-search interface
- Durable logging using Golem APIs for all I/O
- Comprehensive unit test suite
- Provider-specific configuration using environment variables (e.g. API keys, region defaults)

Note: In the future, components will adopt wasi-runtime-config for structured configuration, but Golem currently supports environment variables only.

Testing & Compatibility
All WASM components must be tested:

- In the Golem CLI and Golem Cloud 1.2.x environment
- Against the provider's real API (with mock mode optional but not required)
- For failure cases including rate limits, invalid input, and unsupported features

Extensibility
This WIT abstraction was designed based on the API capabilities of each provider. If a provider’s API surface requires deviation from the current design, that is acceptable — but only if:

- The change is justified with a concrete example or constraint
- It is approved by Golem core contributors

If you wish to recommend adding or swapping in another provider (e.g. You.com, DuckDuckGo, or Neeva), that’s acceptable with approval, but the default five must be completed.

package golem:web-search@1.0.0;

interface types {
  /// Core structure for a single search result
  record search-result {
    title: string,
    url: string,
    snippet: string,
    display-url: option<string>,
    source: option<string>,
    score: option<f64>,
    html-snippet: option<string>,
    date-published: option<string>,
    images: option<list<image-result>>,
    content-chunks: option<list<string>>,
  }

  /// Optional image-related result data
  record image-result {
    url: string,
    description: option<string>,
  }

  /// Optional metadata for a search session
  record search-metadata {
    query: string,
    total-results: option<u64>,
    search-time-ms: option<f64>,
    safe-search: option<safe-search-level>,
    language: option<string>,
    region: option<string>,
    next-page-token: option<string>,
    rate-limits: option<rate-limit-info>,
  }

  /// Safe search settings
  enum safe-search-level {
    off,
    medium,
    high,
  }

  /// Rate limiting metadata
  record rate-limit-info {
    limit: u32,
    remaining: u32,
    reset-timestamp: u64,
  }

  /// Query parameters accepted by the unified search API
  record search-params {
    query: string,
    safe-search: option<safe-search-level>,
    language: option<string>,
    region: option<string>,
    max-results: option<u32>,
    time-range: option<time-range>,
    include-domains: option<list<string>>,
    exclude-domains: option<list<string>>,
    include-images: option<bool>,
    include-html: option<bool>,
    advanced-answer: option<bool>,
  }

  /// Supported time range filtering
  enum time-range {
    day,
    week,
    month,
    year,
  }

  /// Structured search error
  variant search-error {
    invalid-query,
    rate-limited(u32),
    unsupported-feature(string),
    backend-error(string),
  }
}

interface web-search {
  use types.{search-params, search-result, search-metadata, search-error};

  /// Start a search session, returning a search context
  start-search: func(params: search-params) -> result<search-session, search-error>;

  /// One-shot search that returns results immediately (limited result count)
  search-once: func(params: search-params) -> result<tuple<list<search-result>, option<search-metadata>>, search-error>;
}

/// Represents an ongoing search session for pagination or streaming
resource search-session {
  /// Get the next page of results
  next-page: func() -> result<list<types.search-result>, types.search-error>;

  /// Retrieve session metadata (after any query)
  get-metadata: func() -> option<types.search-metadata>;
}

Activity
jdegoes commented 2 days ago
@jdegoes
jdegoes
2 days ago
Author
/bounty $2500

algora-pbc added 
💎 Bounty 
$2.5K 
2 days ago

algora-pbc commented 2 days ago
@algora-pbc
algora-pbc
bot
2 days ago · edited by algora-pbc
💎 $2,500 bounty • Golem Cloud
Steps to solve:
1. Start working: Comment `/attempt #34` with your implementation plan
2. Submit work: Create a pull request including `/claim #34` in the PR body to claim the bounty
3. Receive payment: 100% of the bounty is received 2–5 days post-reward. Make sure you are eligible for payouts

❗ Important guidelines:
- To claim a bounty, you need to provide a short demo video of your changes in your pull request
- If anything is unclear, ask for clarification before starting as this will help avoid potential rework
- Low quality AI PRs will not receive review and will be closed
- Do not ask to be assigned unless you've contributed before

Thank you for contributing to golemcloud/golem-llm!
