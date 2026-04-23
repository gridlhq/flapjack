<!-- assembled by scrai — do not edit directly -->

_This file is auto-generated from `.scrai/` sources. Do not edit directly._

Use bash for filesystem operations. ALWAYS read multiple files in a single
bash call — `cat file1.py file2.py` or `head -n 50 file.py && tail -n +80 file.py | head -n 30`.
NEVER issue separate Read/cat calls for files you could read together.

# SANDBOX RESTRICTIONS

You may be running in a sandboxed environment that blocks port binding, .git writes, and DNS.

**Hard stops — STOP IMMEDIATELY, do NOT retry:**
- `PermissionError: [Errno 1] Operation not permitted` on port binding
- `fatal: Unable to create '.git/index.lock': Operation not permitted`
- `Could not resolve host: github.com`

**When blocked:** skip the operation, document in handoff as "SANDBOX BLOCKER: [what] — deferred", pivot to sandbox-safe work.

## Tool Efficiency — MANDATORY

- **Grouped reads**: ALWAYS `cat file1.py file2.py` in one call. NEVER read files one at a time when you need multiple. This is the single biggest efficiency win.
- **Bash edits**: `sed -i`, `awk`, heredoc writes for straightforward changes.
- **Parallel searches**: ALWAYS combine in one bash call (`grep -rn 'pattern1' src/; grep -rn 'pattern2' src/`). NEVER run sequential single-pattern searches.
- **Avoid `find`**: use `grep -rn` with `--include='*.py'` or `ls`/`cat` with globs instead.

### Codebase Navigation

- **Code Map** (below, under `## Code Map`): function index with file paths and line ranges. Check here FIRST before searching.
- **DIRMAP.md**: per-directory summaries. Check `DIRMAP.md` in any directory you're working in.
- **`matt scrai context <target_dir> <target>`**: ranked cross-file context (callers, callees, deps). Example: `matt scrai context engine engine/src/index/manager.rs::search`.

## Highest-Level Priorities

### Current Priorities

- Maintain Algolia API compatibility — new features must not break existing client integrations
- Keep search latency low and memory usage bounded
- Extend analytics and vector search capabilities

## Global Context

This project is single-maintainer, 100% AI-written code. All development is driven by AI coding agents orchestrated by matt. Dev repos sync to public staging and prod repos via **debbie** (`.debbie.toml`) for CI/CD. New files may sync unless excluded — check `.debbie.toml` when adding files that shouldn't be public. Manual QA-only validation is not allowed; verification must run through automated, reproducible CLI commands.

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
- Filenames must use only alphanumeric characters and underscores — no spaces, dashes, or hyphens
- Single source of truth: one canonical place per fact, function, config, or constant; reference it from elsewhere instead of copying. Applies to code AND docs.
- Clean separation of concerns: don't reach across layers without an explicit contract. Applies to code AND docs.
- No unnecessary complexity: prefer the simpler design that meets the requirements. Don't add abstractions for hypothetical future needs.
- No code smell: name things clearly, keep functions focused, delete dead code, don't paper over bugs with try/except.
- Reuse before adding: check the current owner first, prefer extending it or extracting a shared seam, and avoid parallel implementations or spaghetti code.
- Don't make assumptions about external behavior — verify against the code or against authoritative online sources before encoding it. "I think the API returns X" is not a contract.

### Test Quality
- No false positives — every passing test must also be capable of failing for a real defect.
- No redundant tests — if two tests cover the same path with the same assertions, delete one.
- No flaky tests — quarantine and fix; do not paper over with retries.
- Aim for meaningful coverage that asserts correct values with tight tolerances. Smoke tests that only check shapes/types/absence-of-exceptions are not validation.

### Checklist Authoring
Applies to operators and agents writing matt session input artifacts (the `.md` files that drive autonomous AI dev sessions). Includes daemon-generated planning artifacts.

- **Filename convention.** Start every artifact filename with a date+time-of-day prefix: `<3-letter-month><day>_<am|pm>_<n>_<short_description>.md` (e.g. `apr17_pm_3_settings_reorg.md`). The leading prefix groups same-batch artifacts in directory listings. The Code Quality filename rule still applies: alphanumeric and underscores only, no spaces, no dashes, no other punctuation.
- **Front-and-center PURPOSE.** Restate purpose and high-level goals at the top of EVERY stage, not just once at the top of the file. Agents drift; reminders help them make better judgement calls.
- **Explicit out-of-scope list** in every artifact. Scope creep is one of the most common failure modes for autonomous agents — name what NOT to touch.
- **Per-stage TDD.** Each stage must call out its red tests before its green implementation work.
- **Concrete file paths and approximate line numbers** for every code change. Vague checklists produce vague diffs.
- **Existing owner and reuse plan.** Identify the current owner for the behavior being changed. Prefer extending it or extracting a shared seam. If a new boundary is needed, say why.
- **Automated validation commands only** — no manual QA. Every verification step must be a reproducible CLI command the agent can run unattended.
- **Merge information** at the bottom of each artifact: target branch, rebase-or-merge preference, any coordination notes for concurrent work.
- **Agent operating conditions** stated plainly: full permissions / no sandbox / may install dependencies, delete files, run tests, commit and push as needed. Secrets live outside the worktree (`.env.secret`-style paths) — reference by full path, never hardcode values.

### External System Verification
- Before writing code that depends on an external system's behavior (API output format, coordinate convention, class indexing, unit system), write a contract test that verifies the assumption against documentation or captured reference output. This tests your understanding of their system, not your code.
- Before any operation that costs real money or significant time (GPU experiment, cloud deployment, multi-hour compute), a known-answer integration test must exist that feeds hand-calculated inputs through the full pipeline and asserts correct outputs within tight tolerances. If the test doesn't pass, the operation doesn't run.
- When working with an external technology, check `~/.matt/scrai/globals/technologies/` for verified conventions and known gotchas before writing code that depends on that system's behavior. Available: `aws-ec2`, `carla`, `da3`, `runpod`. Read the relevant file before making assumptions about the system's API, coordinate conventions, or output format.
- Before implementation begins, lightweight operator-scoped inspection or discovery steps are allowed only for orientation; this does not authorize encoding uncontracted assumptions about external-system behavior into code, does not authorize expensive or time-consuming external operations, and does not weaken the contract-test and known-answer integration-test requirements once implementation or costly execution is in scope.

### Debugging
- Focus on debugging and diagnostics instead of just trying stuff when you face an error. Understand what's actually happening before attempting fixes.
- Check online frequently — any problem you face has probably already been discussed and resolved on message boards or blogs. Search for the specific error message or symptom before guessing.
- When validation or experiments produce unexpected results, investigate the code path mechanically before attributing the result to external causes (model limitations, data quality, out-of-distribution inputs). Trace what computation produced the specific number.
- If web search is unavailable, check local docs and `--help` output.

### No Manual QA
- Do not add manual-only QA steps to checklists, prompts, or handoffs.
- All testing and verification must be automated and reproducible via CLI commands.

### Direct Mode Intake (`MATT_DIRECT=1`)
- Use direct mode only when the operator already supplies a concrete staged plan and wants mechanical formatting with no intake/vetting review loop.
- Required input shape:
```markdown
## Stages
### <title>
<stage body>
```
- `## Stages` must contain one or more `###` stage headings; each stage needs a non-empty title and body.
- Output is canonical `stages.md` with sequential `## [ ] Stage N: <title>` headers. Headers are written untagged; tag suffixes are assigned by the shared stage-plan review pass. Direct mode does not parse inline work-type tags.
- Enable with `MATT_DIRECT=1`. This path bypasses intake and vet-input LLM steps.

### Scrai-Generated Artifacts — Do Not Manually Clean

`matt scrai scan` generates `TODO: Document` stubs on undocumented functions across all projects. These are **dev-repo scaffolding** — documentation-coverage markers used by the scanning pipeline. For projects using debbie, stubs are stripped automatically by post-sync hooks during sync to staging/prod and never appear in public repos.

**Do NOT** create checklists, sessions, or commits to bulk-remove or bulk-replace these stubs. The cleanup is handled mechanically at the deployment layer (debbie post-sync hooks). If you're already working on a function and want to write a real doc comment, replace the stub inline — but never make stub cleanup a standalone task.

### Never
- Mention any LLM provider name (claude, anthropic, openai, etc.) in commit messages
- Add `Co-Authored-By` or similar AI attribution lines in commits
- Skip validation checks to save time
- Commit secrets, API keys, or credentials
- Create sessions/checklists to bulk-remove `TODO: Document` stubs (stripped by debbie sync)

### Session Autonomy
Matt sessions run autonomously. Never pause for human approval.
Install dependencies, delete files, and run tests when the task requires it.
Use focused tests by default; run the full suite when the change surface warrants it.
Commit and push freely.

### Shared-Host Process Safety
Concurrent matt/batman workers and auto-lifecycle daemons run as Python processes on shared hosts.
Broad `killall`, `pkill -f`, `ps aux | grep ... | xargs kill`, and grep-derived `kill -9` commands are forbidden.
Only exact PIDs started by the current session may be terminated.
On this macOS host, `Xcode.app` and `Python3.framework` grep patterns match all Python processes and must not be used for kill targeting.

### Commit Discipline
- Commit after every logical unit of work. Do not batch unrelated changes.
- If a session produces multiple independent changes, commit them as separate commits.
- Never end a session with uncommitted work.

### Knowledge Store Discipline
- Grep the knowledge store for existing factual coverage before writing new factual claims.
- Update canonical facts in place instead of restating them in `research/` notes.
- Reference canonical files by path instead of copying their content into notes.
- Delete or rewrite stale `research/` notes when findings are promoted into `decisions/` or `reference/`.
- When creating or updating knowledge files, maintain `created` and `updated` YAML frontmatter date fields (`YYYY-MM-DD`). Set both on creation; update `updated` on every substantive edit.

### Deployment & CI/CD Flow
- Dev repos are **private**. GitHub Actions burns paid minutes on private repos.
- **Disable GitHub Actions** on all dev repos (Settings → Actions → General → Disable actions).
- NEVER debug, fix, or monitor CI failures from a dev repo.
- Repos with `.debbie.toml` use debbie to sync curated files to public staging and prod repos where CI runs for free.
- Flow: dev repo → `debbie sync staging` → staging CI (free) → `debbie sync prod` → prod CI (free).
- NEVER add dev repos to workflow check-repo guards or CI configs.
- GHCR, Docker publication, releases, and installer tests run on staging/prod repos only.
- `.debbie.toml` is the single source of truth for sync scope and identity rewrites.

### Roadmap Maintenance
- Keep shared planning-doc governance wording generic so it remains valid across repo-specific path migrations.
- Repos must maintain `PRIORITIES.md` with a `## Summary` section. `PRIORITIES.md` owns strategic intent and priority ordering — do not duplicate specific work-item IDs, implementation deltas, or source links that belong in `ROADMAP.md`.
- Repos must maintain `ROADMAP.md` with a `## Open / Not Yet Implemented` section, kept under the `ROADMAP_MAX_LINES` cap enforced by scrai (currently 300 lines). `ROADMAP.md` owns the open-work item index — do not restate priority rationale that belongs in `PRIORITIES.md`. Historical implemented items belong in `roadmap/implemented.md`, not in `ROADMAP.md`.
- Keep temporary path exceptions only in the repo-local `.scrai/rules.md` for that repo; do not duplicate shared governance text there.

### Standards References
- `~/.matt/scrai/globals/standards/browser_testing.md` - Browser test reliability and reproducibility standards.
- `~/.matt/scrai/globals/standards/ml_experiment_qa.md` - ML experiment validation and quality-assurance standards.
- `~/.matt/scrai/globals/standards/ui_screen_specs.md` - UI screen specification and acceptance standards.
- `~/.matt/scrai/globals/standards/ux_workflow_design.md` - UX workflow planning and design standards.

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

- Fast feedback: run the smallest relevant test after every code change
- Tests use isolated temp directories — never touch real project state
- Use focused single-file test runs for routine checks; run the full suite autonomously when shared surfaces changed or focused tests show wider risk
- Do not use manual QA as validation; every verification step must be an automated, reproducible CLI command
- Correctness over crash-resistance: tests that only check shapes, types, and absence of exceptions are smoke tests. For functions that transform data, assert output values match hand-calculated expected values with tight tolerances
- Validation gate thresholds must test "is this correct?" not "did something happen?" — set thresholds tight enough to reject values you know are wrong

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
