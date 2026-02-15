# Flapjack Dashboard

never run cargo clean! never run the entire test suite ntil the very last step! until you've run all relevant targeted tests and done "cargo smoke" etc

Web UI for managing Flapjack search indices. React + TypeScript + Playwright.

**This is a 100% AI-written codebase with single maintainer and zero manual QA.** Documentation is structured for AI consumption.





never use the "/commit" task to commit to github. use normal cmd tools and never mentoin claude or anthropic in the messages




always refer to e2e-ui as e2e-ui tests so we dont get confused and think they're just e2e api tests. 


## ALWAYS Read These Files First (In Order)

**Before starting ANY task:**

1. `_dev/highest_priority_DO_NOT_EDIT.md` — Core context, critical constraints, current phase
2. `_dev/synopsis_DO_NOT_EDIT.md` — Current project state, what works, what's in progress
3. `_dev/FEATURES.md` — Feature status and priorities

**For implementing features:**

4. `docs/BDD_SPECIFICATIONS.md` — User story (Tier 1)
5. `tests/specs/{feature}.md` — Detailed test spec with fixtures (Tier 2)
6. `_dev/testing/TESTING.md` — Dashboard-specific testing guidance

**For session context:**

- Latest handoff: `_dev/session/handoffs/handoff-session-NNN.md`
- Latest checklist: `_dev/session/checklists/session-NNN-checklist.md`
- Latest test audit: `_dev/session/test_audits/session-NNN-audit.md`

## Core Workflows

### Testing (CRITICAL)

**Read `_dev/testing/AI_TESTING_METHODOLOGY.md` BEFORE writing any test.**

1. Read BDD spec (`docs/BDD_SPECIFICATIONS.md`)
2. Read test spec (`tests/specs/{feature}.md`)
3. Load fixtures (`tests/fixtures/`) with expected values metadata
4. Implement tests (following spec exactly)
5. Implement feature to pass tests

**Test Types — CRITICAL DISTINCTION:**
- **E2E-UI tests**: `npm test` — **Real Chromium browser**, real server, simulated human. NO mocks. `tests/e2e-ui/`
- **E2E-API tests**: `npm run test:e2e-api` — Pure HTTP API tests. **No browser.** No `page.goto()`. `tests/e2e-api/`
- **Unit tests**: `npm run test:unit` — Vitest + React Testing Library. Fast, isolated.

If a test opens a browser → it goes in `e2e-ui/`. Period.
If a test only calls REST APIs → it goes in `e2e-api/`. Period.

**Coverage targets:**
- E2E-UI: 100% of user-facing features (real browser, simulated human)
- E2E-API: API shape/data integrity verification
- Unit: 85%+ (components, hooks, utilities)

### Development

```bash
# Install dependencies
npm install

# Start dev server (requires backend running)
npm run dev

# Start backend server
npm run server

# Run tests
npm run test:unit        # Unit tests (watch mode)
npm test                 # E2E-UI tests (real browser, no mocks)
npm run test:smoke       # Smoke tests only
npm run test:e2e-ui      # Full E2E-UI suite
npm run test:e2e-api     # API-level tests (no browser rendering)
npm run test:ui          # Playwright UI mode (debugging)

# Build
npm run build
npm run preview          # Preview production build
```

### Quality Gates

- All tests must pass before marking work complete
- Unit tests run in < 5s (watch mode)
- Smoke tests run in ~2 min
- Full E2E tests run in ~10-15 min
- No commits without passing tests

## Directory Structure

```
dashboard/
├── CLAUDE.md                      # This file
├── _dev/                          # Private, AI-optimized docs
│   ├── highest_priority_DO_NOT_EDIT.md
│   ├── synopsis_DO_NOT_EDIT.md
│   ├── FEATURES.md
│   ├── testing/
│   │   ├── AI_TESTING_METHODOLOGY.md   # Generic methodology
│   │   └── TESTING.md                  # Dashboard-specific testing
│   └── session/
│       ├── handoffs/
│       ├── checklists/
│       └── test_audits/
├── docs/                          # Minimal public-facing docs
│   └── BDD_SPECIFICATIONS.md      # Tier 1: User stories
├── src/
│   ├── components/                # React components + unit tests
│   │   └── *.test.tsx
│   ├── hooks/                     # Custom hooks + unit tests
│   │   └── *.test.ts
│   ├── lib/                       # Utilities + unit tests
│   │   └── *.test.ts
│   ├── pages/                     # Route pages
│   └── App.tsx
├── tests/
│   ├── specs/                     # Tier 2: Detailed test specs
│   │   ├── index-management.md
│   │   ├── search-browse.md
│   │   ├── settings-form.md
│   │   ├── api-keys.md
│   │   └── analytics.md
│   ├── fixtures/                  # Test data with expected values
│   │   └── test-data.ts
│   ├── e2e-ui/                     # NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS
│   │   ├── smoke/                 # Fast critical paths (2 min)
│   │   └── full/                  # Comprehensive suite (10-15 min)
│   └── e2e-api/                   # API-level tests (no browser rendering)
├── package.json
├── playwright.config.ts
├── vitest.config.ts
└── vite.config.ts
```

## Stack

- **Framework:** React 18 + TypeScript
- **Build:** Vite 5
- **Styling:** TailwindCSS + Radix UI
- **State:** Zustand (global) + React Query (server state)
- **Forms:** React Hook Form + Zod
- **Testing:** Vitest + React Testing Library + Playwright
- **Charts:** Recharts

## Testing Approach (3-Tier BDD)

### Tier 1: Business Behaviors
User stories in `docs/BDD_SPECIFICATIONS.md`

```markdown
### B-IDX-001: Create Index
**As a** user **I want to** create a new index **So that** I can add documents
**Acceptance Criteria:** [list expected behaviors and edge cases]
```

### Tier 2: Test Specifications
Detailed specs in `tests/specs/{feature}.md` — **Read these before implementing tests**

```markdown
## TEST: Create index with valid name
**Fixtures:** fixtures/test-data.ts
**Execute:** Click [data-testid="create-index-btn"] → Enter "my-index" → Submit
**Verify UI:** [data-testid="index-list"] contains "my-index"
**Verify API:** GET /indexes returns array including "my-index"
**Cleanup:** DELETE /indexes/my-index
```

### Tier 3: Test Implementation
**You generate this** from Tier 2 specs

**Workflow:** Read Tier 2 spec → Generate tests → Add testIDs to components → Implement feature to pass tests

## Quick Reference

- **Start dev:** `npm run dev` (requires backend on port 7700)
- **Test unit:** `npm run test:unit` (< 5s, watch mode)
- **Test smoke:** `npm run test:smoke` (~2 min, critical paths)
- **Test all:** `npm run test:e2e` (~10-15 min, comprehensive)
- **Debug tests:** `npm run test:ui` (Playwright UI mode)

## Known Issues

See `TESTING_SUMMARY.md` for current test status and known issues.

**Current focus areas:**
- Facets panel bug (makes redundant search queries)
- Clear analytics UX (needs ConfirmDialog component)
- Import index button (missing from Overview page)

## Next Steps

**New to the project?**
1. Read `_dev/highest_priority_DO_NOT_EDIT.md`
2. Read `_dev/synopsis_DO_NOT_EDIT.md`
3. Check `_dev/FEATURES.md` for what to work on
4. Read the relevant BDD spec and test spec
5. Run tests to understand current state
6. Implement with tests-first approach

**Starting a new feature?**
1. Write BDD spec (Tier 1) if missing
2. Write detailed test spec (Tier 2)
3. Write unit tests for components/hooks
4. Write E2E tests from spec
5. Implement feature to pass tests
6. Update session handoff when done

---

**Important:** Never commit code that references Claude or Anthropic in git messages. Use normal git tools and write professional commit messages.
