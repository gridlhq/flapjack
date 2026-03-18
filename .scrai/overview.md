## Overview

Flapjack is a drop-in replacement for Algolia — a typo-tolerant full-text search engine with faceting, geo search, custom ranking, vector search, and click analytics. Compatible with InstantSearch.js and the algoliasearch client. Single static binary, data stays on disk.

### Architecture

- **Core library** (`engine/src/`) — search engine built on Tantivy: indexing, query execution, faceting, typo tolerance, geo, vector search, analytics
- **HTTP server** (`engine/flapjack-server/`) — Axum-based REST API with Algolia-compatible endpoints, auth, OpenAPI
- **HTTP client layer** (`engine/flapjack-http/`) — shared HTTP types and routing
- **Replication** (`engine/flapjack-replication/`) — peer-to-peer index replication (circuit breaker, peer management)
- **SSL** (`engine/flapjack-ssl/`) — TLS/SSL support
- **SDKs** (`sdks/`) — client SDKs (outside engine scope)

### Core Library Modules (`engine/src/`)

| Module | Purpose |
|--------|---------|
| `index/` | Index management, document storage, schema, settings, facets, S3 snapshots, write queue, relevance scoring, synonyms |
| `query/` | Query parsing, fuzzy matching, filtering, geo queries, highlighting, word splitting, stopwords, plurals |
| `analytics/` | Click/conversion analytics, HLL aggregation, retention, DataFusion-based query engine |
| `vector/` | Vector search (usearch), embedding, config |
| `query_suggestions/` | Query suggestion generation |
| `tokenizer/` | Custom tokenizer pipeline |
| `types.rs` | Shared types (Document, FieldValue, SearchResults) |
| `error.rs` | Error types |

### Current Priorities

- Maintain Algolia API compatibility — new features must not break existing client integrations
- Keep search latency low and memory usage bounded
- Extend analytics and vector search capabilities
