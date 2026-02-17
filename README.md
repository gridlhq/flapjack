# 🥞 Flapjack ![Beta](https://img.shields.io/badge/status-beta-orange)

[![CI](https://github.com/gridlhq/flapjack/actions/workflows/ci.yml/badge.svg)](https://github.com/gridlhq/flapjack/actions/workflows/ci.yml)
[![Release](https://github.com/gridlhq/flapjack/actions/workflows/release.yml/badge.svg)](https://github.com/gridlhq/flapjack/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Drop-in replacement for [Algolia](https://algolia.com) — works with [InstantSearch.js](https://github.com/algolia/instantsearch) and the [algoliasearch](https://github.com/algolia/algoliasearch-client-javascript) client. Typo-tolerant full-text search with faceting, geo search, and custom ranking. Single static binary, runs anywhere, data stays on disk.

**[Live Demo](https://flapjack-demo.pages.dev)** · **[Geo Demo](https://flapjack-demo.pages.dev/geo)** · **[API Docs](https://flapjack-demo.pages.dev/api-docs)**

---

## Quickstart

```bash
curl -fsSL https://install.flapjack.foo | sh    #install

flapjack                                        #run the server

open http://127.0.0.1:7700/dashboard            #load the dashboard


# Add documents
curl -X POST http://localhost:7700/indexes/movies/documents \
  -d '[
    {"objectID":"1","title":"The Matrix","year":1999},
    {"objectID":"2","title":"Inception","year":2010}
  ]'

# Search
curl "http://localhost:7700/indexes/movies/search?q=matrxi"

flapjack uninstall
```


<details>
<summary>Quickstart API</summary>

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/indexes` | List all indexes |
| `GET` | `/indexes/:name/search?q=...` | Search (query params) |
| `POST` | `/indexes/:name/search` | Search (JSON body) |
| `POST` | `/indexes/:name/documents` | Add/update documents |
| `GET` | `/indexes/:name/documents/:id` | Get a document |
| `DELETE` | `/indexes/:name/documents/:id` | Delete a document |
| `POST` | `/indexes/:name/documents/delete` | Bulk delete by ID array |
| `GET` | `/indexes/:name/settings` | Get index settings |
| `PUT` | `/indexes/:name/settings` | Update index settings |
| `DELETE` | `/indexes/:name` | Delete an index |
| `GET` | `/tasks/:taskId` | Check task status |
| `POST` | `/migrate` | Migrate from Algolia |

These are convenience endpoints with no auth required in dev mode. The full Algolia-compatible API lives under `/1/` with support for API keys, secured keys, filters, facets, and everything else.

</details>

<details>
<summary>Note:</summary>

Binaries: [Releases](https://github.com/gridlhq/flapjack/releases/latest).

```bash
# Install specific version
curl -fsSL https://install.flapjack.foo | sh -s -- v0.2.0

# Custom install directory
FLAPJACK_INSTALL=/opt/flapjack curl -fsSL https://install.flapjack.foo | sh

# Skip PATH modification
NO_MODIFY_PATH=1 curl -fsSL https://install.flapjack.foo | sh
```

</details>

---

## Migrate from Algolia

```bash
flapjack

curl -X POST http://localhost:7700/migrate \
  -d '{"appId":"YOUR_ALGOLIA_APP_ID","apiKey":"YOUR_ALGOLIA_ADMIN_KEY","sourceIndex":"products"}'
```

Search:

```bash
curl "http://localhost:7700/indexes/products/search?q=widget"
```

Then point your frontend at Flapjack instead of Algolia:

```javascript
import algoliasearch from 'algoliasearch';

// app-id can be any string, api-key is your FLAPJACK_ADMIN_KEY or a search key
const client = algoliasearch('my-app', 'your-flapjack-api-key');
client.transporter.hosts = [{ url: 'localhost:7700', protocol: 'http' }];

// Everything else stays the same
```

InstantSearch.js widgets work as-is — `SearchBox`, `Hits`, `RefinementList`, `Pagination`, `GeoSearch`, etc.

---

## Features

| Feature | Details |
|---------|---------|
| Full-text search | Prefix matching, typo tolerance (Levenshtein ≤1/≤2) |
| Filters | Numeric, string, boolean, date — `AND`/`OR`/`NOT` |
| Faceting | Hierarchical, searchable, `filterOnly`, wildcard `*` |
| Geo search | `aroundLatLng`, `insideBoundingBox`, `insidePolygon`, auto-radius |
| Highlighting | Typo-aware, supports nested objects and arrays |
| Custom ranking | Multi-field, `asc`/`desc` |
| Synonyms | One-way, multi-way, alternative corrections |
| Query rules | Rewrite queries, pin/hide results |
| Pagination | `page`/`hitsPerPage` and `offset`/`length` |
| Distinct | Deduplication by attribute |
| Stop words & plurals | English built-in |
| Batch operations | Add, update, delete, clear, browse |
| API keys | ACL, index patterns, TTL, secured keys (HMAC) |
| S3 backup/restore | Scheduled snapshots, auto-restore on startup |

Algolia-compatible REST API under `/1/` — works with InstantSearch.js v5, the algoliasearch client, and [Laravel Scout](integrations/laravel-scout/).

---

## Comparison

|  | Flapjack | Algolia | Meilisearch | Typesense | Elasticsearch | OpenSearch |
|--|----------|---------|-------------|-----------|---------------|-----------|
| Self-hosted | ✅ | ❌ | ✅ | ✅ | ✅ | ✅ |
| License | MIT | Proprietary | MIT | GPL-3 | ELv2 / SSPL / AGPL | Apache 2.0 |
| Algolia-compatible API | ✅ | — | ❌ | ❌ | ❌ | ❌ |
| InstantSearch.js | Native | Native | Adapter | Adapter | Community | Community |
| One-click Algolia migration | ✅ | — | ❌ | ❌ | ❌ | ❌ |
| Typo tolerance | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Faceting (hierarchical) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Filters (numeric, string, bool, date) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Geo search | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Synonyms | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Query rules (pin/hide) | Basic | ✅ | ❌ | ✅ | ✅ | ✅ |
| Custom ranking | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Analytics | Basic | ✅ | Cloud only | ✅ | ✅ | ✅ |
| Scoped API keys (HMAC) | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| S3 backup/restore | ✅ | N/A | ❌ | ❌ | Snapshots | Snapshots |
| Dashboard UI | ✅ | ✅ | ✅ | Cloud only | Kibana | Dashboards |
| Embeddable as library | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| HA / clustering | ❌ | ✅ | Cloud only | ✅ | ✅ | ✅ |
| Multi-language | English only | 60+ | Many | Many | Many | Many |
| Vector / semantic search | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AI search (RAG) | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Hybrid search (keyword + vector) | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A/B testing | ❌ | ✅ | ❌ | ❌ | ❌ | Partial |

---

## Deployment

```bash
cargo build --release
./target/release/flapjack
```

Requires Rust 1.70+. Pre-built binaries for Linux x86_64 (static musl), Linux ARM64, macOS Intel, and macOS Apple Silicon on the [releases page](https://github.com/gridlhq/flapjack/releases/latest).

<details>
<summary>Docker</summary>

```yaml
# docker-compose.yml
services:
  flapjack:
    image: flapjack/flapjack:latest
    ports:
      - "7700:7700"
    volumes:
      - ./data:/data
    environment:
      FLAPJACK_DATA_DIR: /data
      FLAPJACK_ADMIN_KEY: ${ADMIN_KEY}
    restart: unless-stopped
```

</details>

<details>
<summary>Systemd</summary>

```ini
# /etc/systemd/system/flapjack.service
[Unit]
Description=Flapjack Search Server
After=network.target

[Service]
Type=simple
User=flapjack
WorkingDirectory=/var/lib/flapjack
Environment=FLAPJACK_DATA_DIR=/var/lib/flapjack/data
Environment=FLAPJACK_ADMIN_KEY=your-key
ExecStart=/usr/local/bin/flapjack
Restart=always

[Install]
WantedBy=multi-user.target
```

</details>

---

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `FLAPJACK_DATA_DIR` | `./data` | Index storage directory |
| `FLAPJACK_BIND_ADDR` | `127.0.0.1:7700` | Listen address |
| `FLAPJACK_ADMIN_KEY` | — | Admin API key (enables auth) |
| `FLAPJACK_ENV` | `development` | `production` requires auth on all endpoints |
| `FLAPJACK_S3_BUCKET` | — | S3 bucket for snapshots |
| `FLAPJACK_S3_REGION` | `us-west-1` | S3 region |
| `FLAPJACK_SNAPSHOT_INTERVAL` | — | Auto-snapshot interval (e.g. `6h`) |
| `FLAPJACK_SNAPSHOT_RETENTION` | — | Retention period (e.g. `30d`) |

Data stored in `FLAPJACK_DATA_DIR`. Mount as a volume in Docker.

---

## API Documentation

- [Online API docs](https://flapjack-demo.pages.dev/api-docs)
- [Swagger UI](http://localhost:7700/swagger-ui/) (local)

---

## Use as a Library

Flapjack's core can be embedded directly:

```toml
[dependencies]
flapjack = { version = "0.1", default-features = false }
```

See [LIB.md](LIB.md) for the embedding guide.

---

## Architecture

Built on [Tantivy](https://github.com/stuartcrobinson/tantivy) (forked for edge-ngram prefix search). Axum + Tokio HTTP server. Supports 600+ indexes per 4GB node.

```
flapjack/              # Core library (search, indexing, query execution)
flapjack-http/         # HTTP server (Axum handlers, routing)
flapjack-replication/  # Cluster coordination
flapjack-ssl/          # TLS (Let's Encrypt, ACME)
flapjack/       # Binary entrypoint
```

---

## Development

```bash
cargo install cargo-nextest
cargo nextest run
```

---

## Roadmap

**Security**
- [x] Hash API keys at rest (salted SHA-256) ✅
- [x] Admin key on first boot only ✅
- [x] Key type prefixes (`fj_admin_`, `fj_search_`) ✅

**Infrastructure**
- [ ] High availability — multi-node replication
- [ ] Managed cloud (SaaS)

**Search**
- [ ] Vector / hybrid search
- [ ] AI search (RAG)
- [ ] A/B testing
- [ ] Multi-language support

**Platform**
- [ ] Webhooks
- [ ] Role-based access control

---

## Dashboard Screenshots

Flapjack includes a built-in dashboard UI served at `http://localhost:7700` when the server is running.

<img src="engine/dashboard/img/dash_overview.png" width="600" />

<img src="engine/dashboard/img/dash_search.png" width="600" />

<img src="engine/dashboard/img/dash_migrate_alg.png" width="600" />

---

## License

 [MIT](LICENSE)
