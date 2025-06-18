# Golem Web Search API

WebAssembly Components providing a unified API for various web search providers.

## Versions

There are 5 published WASM files for each release:

| Name                                 | Description                                                                          |
|--------------------------------------|--------------------------------------------------------------------------------------|
| `websearch-google.wasm`              | Web search implementation for Google Custom Search                                  |
| `websearch-bing.wasm`                | Web search implementation for Microsoft Bing                                       |
| `websearch-brave.wasm`               | Web search implementation for Brave Search                                         |
| `websearch-tavily.wasm`              | Web search implementation for Tavily                                               |
| `websearch-serper.wasm`              | Web search implementation for Serper                                               |

Every component **exports** the same `golem:web-search` interface, [defined here](wit-websearch/golem-web-search.wit).

All versions depend on `wasi:io`, `wasi:http` and `wasi:logging` and are compatible with WASI 0.2.

## Usage

Each provider has to be configured with an API key passed as an environment variable:

| Provider   | Environment Variable    |
|------------|------------------------|
| Google     | `GOOGLE_API_KEY`       |
| Bing       | `BING_API_KEY`         |
| Brave      | `BRAVE_API_KEY`        |
| Tavily     | `TAVILY_API_KEY`       |
| Serper     | `SERPER_API_KEY`       |

Additionally, setting the `GOLEM_WEBSEARCH_LOG=trace` environment variable enables trace logging for all communication with the underlying search providers.

### Using with Golem

#### Using a template

The easiest way to get started is to use one of the predefined **templates** Golem provides.

**Available soon**

#### Using a component dependency

To existing Golem applications the `golem-web-search` WASM components can be added as a **binary dependency**.

#### Integrating the composing step to the build

The `wac` tool can be used to compose with the selected web search implementation. 

Example steps for integrating with an existing component:

1. Copy the appropriate web search WASM file to your project
2. Add the `golem-web-search.wit` file to your component's `wit/deps/golem:web-search` directory
3. Import in your component's WIT file: `import golem:web-search/web-search@1.0.0;`
4. Use `wac` to compose your component with the selected search implementation

Example composition command:
```bash
wac plug --plug websearch-google.wasm my-component.wasm -o my-component-with-search.wasm
```

### Using without Golem

To use the web search provider components in a WebAssembly project independent of Golem:

1. Download one of the provider WASM files
2. Download the `golem-web-search.wit` WIT package and import it
3. Use [`wac`](https://github.com/bytecodealliance/wac) to compose your component with the selected search implementation

## Building from Source

### Prerequisites

- Rust toolchain with `wasm32-wasip1` target
- `cargo-component` for building WASM components

```bash
rustup target add wasm32-wasip1
cargo install cargo-component
```

### Build Commands

```bash
# Build individual providers
cargo component build -p websearch-google
cargo component build -p websearch-bing
cargo component build -p websearch-brave
cargo component build -p websearch-tavily
cargo component build -p websearch-serper

# Built components will be in target/wasm32-wasip1/debug/
```

## Architecture

The implementation follows a modular architecture where each search provider is implemented as a separate WASM component that exports the unified `golem:web-search` interface. This allows for:

- **Provider Independence**: Each search provider can be updated independently
- **Runtime Selection**: Choose the appropriate provider for your use case
- **Easy Extension**: New search providers can be added following the same pattern
- **WASM Compatibility**: All components are built for WASM with WASI 0.2 support

## API Reference

See the [WIT interface definition](wit-websearch/golem-web-search.wit) for the complete API specification.

Key features:
- Unified search parameters across all providers
- Consistent result format
- Error handling with provider-specific error codes
- Optional metadata including pagination and rate limiting information

## Examples

Take the [test application](test/components-rust/test-llm/src/lib.rs) as an example of using `golem-llm` from Rust. The
implemented test functions are demonstrating the following:

| Function Name | Description                                                                                |
|---------------|--------------------------------------------------------------------------------------------|
| `test1`       | Simple text question and answer, no streaming                                              | 
| `test2`       | Demonstrates using **tools** without streaming                                             |
| `test3`       | Simple text question and answer with streaming                                             |
| `test4`       | Tool usage with streaming                                                                  |
| `test5`       | Using an image in the prompt                                                               |
| `test6`       | Demonstrates that the streaming response is continued in case of a crash (with Golem only) |
| `test7`       | Using a source image by passing byte array as base64 in the prompt                         |

### Running the examples

To run the examples first you need a running Golem instance. This can be Golem Cloud or the single-executable `golem`
binary
started with `golem server run`.

**NOTE**: `golem-llm` requires the latest (unstable) version of Golem currently. It's going to work with the next public
stable release 1.2.2.

Then build and deploy the _test application_. Select one of the following profiles to choose which provider to use:
| Profile Name | Description |
|--------------|-----------------------------------------------------------------------------------------------|
| `anthropic-debug` | Uses the Anthropic LLM implementation and compiles the code in debug profile |
| `anthropic-release` | Uses the Anthropic LLM implementation and compiles the code in release profile |
| `ollama-debug` | Uses the Ollama LLM implementation and compiles the code in debug profile |
| `ollama-release` | Uses the Ollama LLM implementation and compiles the code in release profile |
| `grok-debug` | Uses the Grok LLM implementation and compiles the code in debug profile |
| `grok-release` | Uses the Grok LLM implementation and compiles the code in release profile |
| `openai-debug` | Uses the OpenAI LLM implementation and compiles the code in debug profile |
| `openai-release` | Uses the OpenAI LLM implementation and compiles the code in release profile |
| `openrouter-debug` | Uses the OpenRouter LLM implementation and compiles the code in debug profile |
| `openrouter-release` | Uses the OpenRouter LLM implementation and compiles the code in release profile |

```bash
cd test
golem app build -b openai-debug
golem app deploy -b openai-debug
```

Depending on the provider selected, an environment variable has to be set for the worker to be started, containing the API key for the given provider:

```bash
golem worker new test:llm/debug --env OPENAI_API_KEY=xxx --env GOLEM_LLM_LOG=trace
```

Then you can invoke the test functions on this worker:

```bash
golem worker invoke test:llm/debug test1 --stream 
```

## Development

This repository uses [cargo-make](https://github.com/sagiegurari/cargo-make) to automate build tasks.
Some of the important tasks are:

| Command                             | Description                                                                                            |
|-------------------------------------|--------------------------------------------------------------------------------------------------------|
| `cargo make build`                  | Build all components with Golem bindings in Debug                                                      |
| `cargo make release-build`          | Build all components with Golem bindings in Release                                                    |
| `cargo make build-portable`         | Build all components with no Golem bindings in Debug                                                   |
| `cargo make release-build-portable` | Build all components with no Golem bindings in Release                                                 |
| `cargo make unit-tests`             | Run all unit tests                                                                                     |
| `cargo make check`                  | Checks formatting and Clippy rules                                                                     |
| `cargo make fix`                    | Fixes formatting and Clippy rules                                                                      |
| `cargo make wit`                    | To be used after editing the `wit/golem-llm.wit` file - distributes the changes to all wit directories |

The `test` directory contains a **Golem application** for testing various features of the LLM components.
Check [the Golem documentation](https://learn.golem.cloud/quickstart) to learn how to install Golem and `golem-cli` to
run these tests.

