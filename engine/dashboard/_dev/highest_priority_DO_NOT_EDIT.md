# Highest Priority Context

**Project:** Flapjack Dashboard (React Admin UI for search engine)
**Status:** Beta - Active development
**Last Updated:** 2026-02-13

## Critical Constraints

- **100% AI-written code** - Single human maintainer (Stuart), zero manual QA
- **Zero manual testing** - All validation must be automated
- **Tests are specification AND validation** - Higher coverage than industry standard
- **Never commit without passing tests** - No exceptions

## Current Phase

**Phase:** Testing Infrastructure Overhaul
**Focus:** Establishing 3-tier BDD approach with comprehensive test coverage

## What This Project Is

A web dashboard for managing Flapjack search indices:
- Create/delete/browse indexes
- Search and filter documents
- Configure index settings (searchable attributes, facets, ranking)
- Manage API keys
- View analytics

Built with React + TypeScript + Vite, fully typed, using TailwindCSS + Radix UI.

## Tech Stack

- **Frontend:** React 18 + TypeScript + Vite 5
- **State:** Zustand (global) + React Query (server state)
- **UI:** TailwindCSS + Radix UI components
- **Forms:** React Hook Form + Zod validation
- **Testing:** Vitest (unit) + React Testing Library + Playwright (E2E)
- **Backend:** Rust Flapjack server on port 7700

## Critical Architecture Notes

1. **State Management:**
   - Zustand for global UI state (selected index, theme)
   - React Query for all server data (indexes, documents, settings, keys, analytics)
   - No Redux, keep it simple

2. **API Communication:**
   - All API calls go through React Query hooks
   - Base URL: `http://localhost:7700`
   - Uses Algolia-compatible API endpoints under `/1/`

3. **Testing Strategy:**
   - **Unit tests** (65%): Components, hooks, utilities - Vitest + RTL
   - **Integration** (25%): Multi-component flows - Playwright with real backend
   - **E2E smoke** (5%): Critical paths only - Playwright (~2 min)
   - **E2E full** (5%): All features - Playwright (~10-15 min)

## Known Pain Points

1. **Facets panel bug:** Makes redundant search queries when filtering
2. **Analytics performance:** Fetches all 14 hooks on mount instead of lazy loading
3. **No unit tests yet:** Only E2E tests exist (21/39 passing)
4. **Test organization:** Tests scattered, need smoke/full separation

## Current Priorities (in order)

1. **Set up unit testing infrastructure** (Vitest + RTL)
2. **Write BDD specifications** (Tier 1 + Tier 2)
3. **Create comprehensive unit tests** (85%+ coverage)
4. **Reorganize E2E tests** (smoke vs full suites)
5. **Fix facets panel bug** (lift state to parent)

## What NOT to Do

- ❌ Don't add features without tests
- ❌ Don't commit failing tests
- ❌ Don't use class components (functional only)
- ❌ Don't bypass React Query for API calls
- ❌ Don't add emojis unless explicitly requested
- ❌ Don't mention Claude/Anthropic in commit messages

## Development Workflow

1. Read BDD spec → Read test spec → Load fixtures
2. Write unit tests first
3. Write E2E tests from spec
4. Implement feature to pass tests
5. Update session handoff

## Quick Commands

```bash
npm run dev              # Start dev server
npm run server           # Start Flapjack backend
npm run test:unit        # Unit tests (watch mode)
npm run test:smoke       # Smoke tests (~2 min)
npm run test:e2e         # Full E2E (~10-15 min)
npm run test:ui          # Playwright UI mode
```

## File Structure Pattern

```
src/components/MyComponent.tsx     # Component
src/components/MyComponent.test.tsx # Unit tests
tests/specs/my-feature.md          # Tier 2 test spec
tests/e2e/smoke/my-feature.spec.ts # E2E smoke test
docs/BDD_SPECIFICATIONS.md         # Tier 1 user story
```

## Remember

**Every line of code is written by AI. Every bug must be caught by tests. There is no safety net.**

Testing is not optional. It's the only way this project works.
