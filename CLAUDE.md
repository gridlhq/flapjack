<!-- assembled by scrai — do not edit directly -->

_This file is auto-generated from `.scrai/` sources. Do not edit directly._

Use bash for filesystem operations. ALWAYS read multiple files in a single
bash call — `cat file1.py file2.py` or `head -n 50 file.py && tail -n +80 file.py | head -n 30`.
NEVER issue separate Read/cat calls for files you could read together.

Never mention any LLM provider name in commit messages.

## Tool Efficiency — MANDATORY

- **Grouped reads**: ALWAYS `cat file1.py file2.py` in one call. NEVER read files one at a time when you need multiple. This is the single biggest efficiency win.
- **Bash edits**: `sed -i`, `awk`, heredoc writes for straightforward changes.
- **Parallel searches**: ALWAYS combine in one bash call (`grep -rn 'pattern1' src/; grep -rn 'pattern2' src/`). NEVER run sequential single-pattern searches.
- **Avoid `find`**: use `grep -rn` with `--include='*.py'` or `ls`/`cat` with globs instead.

### Codebase Navigation

- **Code Map** (below, under `## Code Map`): function index with file paths and line ranges. Check here FIRST before searching.
- **DIRMAP.md**: per-directory summaries. Check `DIRMAP.md` in any directory you're working in.
- **`matt scrai context <target_dir> <target>`**: ranked cross-file context (callers, callees, deps). Example: `matt scrai context engine engine/src/index/manager.rs::search`.

## Global Context

This project is single-maintainer, 100% AI-written code. All development is driven by AI coding agents orchestrated by matt.

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

## Global Rules

### Code Quality
- Write inline comments liberally for anything not self-evident — AI coders make mistakes frequently, so comments capture intent for future devs (human or AI) to distinguish bugs from design decisions
- Run validation commands after every code change, even for seemingly simple edits
- TDD: write failing tests before implementation (red → green → refactor)
- Follow existing patterns — check neighboring code before inventing new conventions

### Never
- Mention any LLM provider name (claude, anthropic, openai, etc.) in commit messages
- Add `Co-Authored-By` or similar AI attribution lines in commits
- Skip validation checks to save time
- Commit secrets, API keys, or credentials

### Ask First
- `pip install` / `go get` / `cargo add` — adding new dependencies
- `git push` — pushing to remote
- Deleting files
- Running the full test suite (use single-file runs for routine checks)

## Rules

### Validation Commands

Run the relevant checks below after every code change:

```bash
# Check compilation
cd engine && cargo check

# Run tests (single crate)
cd engine && cargo test -p flapjack --lib

# Run tests (specific test)
cd engine && cargo test -p flapjack --lib test_name

# Run clippy
cd engine && cargo clippy --workspace -- -D warnings

# Format check
cd engine && cargo fmt --check
```

### Permissions
- **Allowed without asking**: read files, cargo check, cargo clippy, cargo fmt --check, run single test files
- **Ask first**: cargo add (new dependencies), git push, deleting files, full test suite (`cargo test --workspace`)

### Never
- Run `cargo clean` — rebuilds take too long
- Break Algolia API compatibility without discussion
- Add `unsafe` blocks without justification in comments

## Global Testing Rules

- TDD mandatory: write failing tests before implementation (red → green → refactor)
- Fast feedback: run the smallest relevant test after every code change
- Tests use isolated temp directories — never touch real project state
- Prefer focused single-file test runs for routine checks; ask before running the full suite

## Testing

### Structure
- `engine/tests/` — integration tests
- `engine/src/integ_tests/` — in-crate integration tests
- Unit tests live alongside source in `#[cfg(test)]` modules

### Quick-Reference Commands
```bash
# Run all lib tests (fast)
cd engine && cargo test -p flapjack --lib

# Run a specific test
cd engine && cargo test -p flapjack --lib test_name

# Run integration tests
cd engine && cargo test -p flapjack --test '*'

# Run tests for a subcrate
cd engine && cargo test -p flapjack-server

# Full workspace (ask first)
cd engine && cargo test --workspace
```
