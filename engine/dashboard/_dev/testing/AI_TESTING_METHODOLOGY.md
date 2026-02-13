# AI Testing Methodology

**Context:** 100% AI-written code, single human maintainer, **zero manual QA**
**For:** React Native (mobile) + Node.js (backend) + React/Next.js (web)
**Stack:** Jest, Supertest/Playwright, Maestro

---

## Core Principle

**No manual testing means tests must catch everything.** Comprehensive automated coverage is non-negotiable.

Tests are specification AND validation. Higher coverage than industry standard (85%+ unit, 100% API, complete UI feature coverage).

---

## Test Distribution

- **65-70% Unit Tests** — Jest + React Testing Library / RNTL (isolated functions/components)
- **20% Integration Tests** — Supertest/Playwright (API + business logic + database)
- **10% UI E2E Tests** — Maestro (mobile), Playwright (web) — full user flows (smoke + full)
- **<1% Contract Tests** — Validate external API contracts (Pact/Postman) — minimal, just to keep door open

**Terminology:**
- **Unit** = Single function/component in isolation
- **Integration** = Multiple components together (API endpoint → service → database → response)
- **UI E2E** = Full user journey through actual UI (register → login → home → ...)
- **Contract** = Validate API contracts with external services (OAuth, Stripe, imports)

---

## Four Test Types Explained

**Unit Tests:**
- Test: Individual functions, components, business logic
- Example: `calculatePace(5000, 1500) === 300`
- Mock: External dependencies (API calls, database)
- Catches: Logic bugs, edge cases, data transformations

**Integration Tests:**
- Test: API endpoint + business logic + database working together
- Example: `POST /activities` creates DB record and returns 201
- Mock: External APIs (Stripe, S3, email services) — **but use real test database**
- Catches: Backend bugs, validation, permissions, DB queries, API contract violations

**UI E2E Tests:**
- Test: Full user flows from UI through backend to database
- Example: User registers → logs in → creates activity → sees it in feed
- Mock: Nothing (uses actual UI, backend, test database)
- Catches: Navigation bugs, layout issues, integration failures
- full suite vs smoke tests

**Contract Tests:**
- Test: External API contracts match our integration layer expectations
- Example: OAuth provider returns expected token format, Stripe API structure unchanged
- Mock: Nothing (hit real APIs in test/sandbox mode)
- Catches: Breaking changes in external APIs, SDK version mismatches

**All four required.** Each catches different bug types.

---

## 3-Tier BDD Approach

### Tier 1: Business Behaviors
User stories in `/docs/BDD_SPECIFICATIONS.md`

```markdown
### B-AUTH-001: User Registration
**As a** new user **I want to** register **So that** I can create an account
**Acceptance Criteria:** [list expected behaviors and edge cases]
```

### Tier 2: Test Specifications
Detailed specs in `/tests/specs/{feature}.md` — **Read these before implementing tests**

```markdown
## TEST: Registration with valid inputs
**Fixtures:** users/validUser.json
**Execute:** Navigate RegisterScreen → Enter email/password → Tap [data-testid="register-button"]
**Verify UI:** [data-testid="home-screen"] visible
**Verify API:** GET /auth/me returns user
**Cleanup:** Delete test user
```

**Required elements:**
- Fixture files with known values
- testID selectors (not "find the button")
- Expected values (verify correctness, not just existence)
- Cleanup steps

### Tier 3: Test Implementation
**You generate this** from Tier 2 specs

**Workflow:** Read Tier 2 spec → Generate tests → Add testIDs to components → Implement feature to pass tests

---

## Documentation Structure

```
/docs/AI_TESTING_METHODOLOGY.md   # this document
/docs/BDD_SPECIFICATIONS.md       # Tier 1: User stories
/tests/specs/{feature}.md         # Tier 2: Detailed test specs (READ THESE FIRST)
/tests/fixtures/                  # Test data with known values
/tests/unit/                      # Unit tests (Jest, needed for react native)
/tests/integration/               # Integration tests (Supertest/Playwright)
/tests/e2e-ui/                    # UI E2E tests (Maestro/Playwright)
  /smoke/                         # Fast smoke tests (~2 min)
  /full/                          # Comprehensive suite (~10-15 min)
/tests/contract/                  # Contract tests (Pact/Postman)
  /oauth/                         # OAuth provider contracts
  /stripe/                        # Stripe API contracts (post-MVP)
  /imports/                       # Strava/Garmin import contracts (post-MVP)
```

**Before implementing tests, read:**
1. `/docs/BDD_SPECIFICATIONS.md` — User story
2. `/tests/specs/{feature}.md` — Detailed spec with fixtures, testIDs, expected values
3. `/tests/fixtures/` — Load fixture data

---

## Fixture Format

**All fixtures must include metadata with expected values:**

```json
{
  "metadata": {
    "expected_distance_meters": 5000,
    "expected_pace_seconds_per_km": 300
  },
  "route_points": [ /* actual data */ ]
}
```

**Usage:**
```typescript
expect(activity.distance_meters).toBe(fixture.metadata.expected_distance_meters);
```

**Do not** make up test values. Always use fixture metadata.

---

## Test Requirements

**Test Independence:** Each test creates data, runs, cleans up. No shared state. Use try/finally for cleanup.

**Value Verification:** Assert exact expected values from fixture metadata, not just existence.

**UI State Verification:**
- Add testID to all verifiable UI elements
- Assert screen visibility, not mock function calls
- Verify layout (e.g., button is visible, not scrolled off-screen)

**Coverage Targets:**
- Unit: 85%+ line coverage
- Integration: 100% of API endpoints (no exceptions)
- UI E2E: All user-facing features (smoke + full suite)
  - Smoke: 5 critical paths (~2 min)
  - Full: 30-50 tests (~10-15 min)

---

## UI E2E Test Strategy

**No manual QA = comprehensive automated UI coverage required.**

### Smoke Tests (fast, run on every commit)
critical paths (~2 min total):
- Auth flow (register → login → home → logout)
- Core features happy paths
- Critical layout (submit button visible)

### Full UI E2E Suite (complete, run before release)
**All features with edge cases** (~10-15 min):
- Auth: registration, login, logout, session persistence, errors
- Core feature: create, read, update, delete, all sport types, all visibility levels
- Social: like/unlike, high-five, comment CRUD, follow/unfollow
- Privacy: all permission matrix combinations (9+ tests)
- Layouts: forms with long content, empty states, error states
- Navigation: all screens, back button, deep links

**Run smoke tests in CI. Run full suite before every release.**

---

## When to Run Tests

| Test Type | Trigger | Speed | Purpose |
|-----------|---------|-------|---------|
| Unit | On file save (watch mode) | < 1s | Instant feedback |
| Integration | Before commit | < 2 min | API contract validation |
| UI E2E Smoke | On commit (CI) | ~2-5 min | Critical paths protected |
| UI E2E Full | Before release | ~10-15 min | Comprehensive validation |
| Contract | Weekly / before major release | ~1-2 min | External API breakage detection |

---

## Social Permutation Testing

Generate tests from permission matrix to cover all visibility × relationship combinations:

```typescript
const MATRIX = [
  { visibility: 'public', relationship: 'stranger', canView: true, canEdit: false },
  { visibility: 'friends', relationship: 'stranger', canView: false, canEdit: false },
  { visibility: 'private', relationship: 'follower', canView: false, canEdit: false },
  // ... 6 more rows
];

MATRIX.forEach(row => {
  test(`${row.visibility} - ${row.relationship}`, async () => {
    // Create users, set relationship, verify access
  });
});
```

---

## Contract Testing Strategy

**Purpose:** Validate that external APIs we depend on haven't changed in breaking ways.

**When to Use:**
- OAuth providers (Google, Apple) — Verify token format, user info structure
- Post-MVP: Stripe/PayPal — Verify payment response structure
- Post-MVP: Strava/Garmin imports — Verify activity data format

**For Sigil MVP:** Only 2-3 contract tests to keep the door open:
1. OAuth login returns expected user structure
2. OAuth token refresh works as expected

**Implementation:**
- Use Pact or Postman Contract Testing
- Run weekly or before major releases
- Hit real APIs in test/sandbox mode (not mocked)
- Store contract definitions in `/tests/contract/`

**Example:**
```typescript
// tests/contract/oauth/google-oauth.contract.test.ts
test('Google OAuth returns expected user structure', async () => {
  const response = await googleOAuth.getUserInfo(testAccessToken);

  expect(response).toMatchObject({
    id: expect.any(String),
    email: expect.stringMatching(/.+@.+/),
    name: expect.any(String),
    picture: expect.any(String)
  });
});
```

**Why minimal for MVP:**
- OAuth providers rarely break contracts
- Integration tests already mock OAuth (testing our code)
- Contract tests validate the external service itself
- Add more when integrating Stripe, Strava, Garmin (post-MVP)

**When to expand:**
- Post-MVP when adding Stripe (validate payment webhooks)
- Post-MVP when adding Strava/Garmin imports (validate activity format)
- If OAuth provider announces major API version change
