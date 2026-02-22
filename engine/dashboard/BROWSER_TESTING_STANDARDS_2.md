# Browser Testing Standards

Read this before writing or modifying any UI test.

## important:


  its mandatory that we get to a point where we can tell teh CEO that everything works perfectly without needing manual QA

make sure ui tests actualy verify content of whats on the screen, not just that it exists


we need to be more liberal in using data-testid

we should basically never use xpath or css selectors. maybe just for generated serial content idk.  but then should use data-testid too...



## Three test categories

```
ui/src/components/__tests__/       component tests    (jsdom, mocked API, no browser)
ui/browser-tests-mocked/           browser mocked     (real browser, mocked server)
ui/browser-tests-unmocked/         browser unmocked   (real browser, real server, real DB)
```

Never use "E2E-UI tests." Always use one of these three names.

- Component tests: Vitest + React Testing Library. Fast, co-located with components.
- Browser mocked: Playwright with mocked server responses. For testing UI against hard-to-reproduce server states (500s, timeouts). Directory reserved, not yet populated.
- Browser unmocked: Playwright against a running app server with real database. Zero mocks. Contains smoke/ (critical paths) and full/ (comprehensive).

The rest of this document is about browser-tests-unmocked.


## The rule

Every action in a spec file must be something a human can physically do in a browser. If the element is hidden, disabled, covered, or unreachable through visible UI, the test must not interact with it.


## Arrange vs Act

ARRANGE (setup):
Shortcuts allowed. API calls, data seeding, page.goto, storageState auth. Gets the app to a starting state. Not what the test is testing.

ACT + ASSERT (the test itself):
Zero shortcuts. Every click, keystroke, and navigation through the UI exactly as a human would. No API calls, no page.evaluate, no page.goto to skip steps in the flow under test.

The line: if the action IS what you're testing, it goes through the UI. If it's a precondition, a shortcut is fine.

Test: "user can edit a borrower"
- Arrange: create borrower via API (fine, creation isn't under test)
- Act: click borrower in list, edit name field, click Save (UI only)
- Assert: verify updated name is visible

Test: "user can create a borrower"
- Arrange: page.goto to the borrowers page (fine)
- Act: click New, fill form, click Create (UI only, this IS under test)
- Assert: verify borrower appears in list


## Load-and-verify rule

Every page's first spec must verify that seeded data renders correctly in the default list/table view before testing any CRUD actions. Seed at least one record in Arrange, then assert it appears in the page body. This catches response-shape mismatches (e.g. backend returns `{items: [...]}` but frontend expects `[...]`) that CRUD-only tests miss because they skip the list rendering path.

Example — Webhooks:
- Arrange: create a webhook via API
- Act: navigate to Webhooks page
- Assert: verify the seeded webhook URL appears in the table

Only after this passes should subsequent tests exercise create/edit/delete.


## Assert the right thing

After navigating to a page, assert on content unique to the page body — not text that also appears in the sidebar or nav. A React crash behind a nav-label match is invisible to the test.

Bad:
```
await webhooksButton.click();
await expect(page.getByText("Webhooks").first()).toBeVisible();  // matches sidebar
```

Good:
```
await webhooksButton.click();
await expect(page.getByRole('heading', { name: 'Webhooks' })).toBeVisible();
// or assert on page-specific content like the "Add Webhook" button or table headers
```


## Enforcement

This is not honor-system. Two layers physically prevent cheating in spec files.

Layer 1 -- ESLint (catches violations at lint time):

The config at tests/e2e-ui/eslint.config.mjs applies strict rules to *.spec.ts files only. Fixture and setup files are exempt. Run with `npm run lint:e2e`. Key rules:

```
playwright/no-eval              bans page.evaluate, page.$eval
playwright/no-element-handle    bans deprecated page.$() API
playwright/no-force-option      bans { force: true } on any action
playwright/no-page-pause        bans page.pause() (debugging leftover)
no-restricted-syntax            bans:
  - request.*, waitForTimeout, dispatchEvent, setExtraHTTPHeaders
  - .locator('.className')   CSS class selectors
  - .locator('//xpath')      XPath selectors
  - .locator('[attr=val]')   attribute selectors
```

Tag-based locators (table, tr, td, th, svg, etc.) are allowed for row-scoping. CSS class selectors, XPath, and attribute selectors are banned.

If you put an API call or CSS class selector in a spec file, ESLint will reject it. The fix is to move shortcuts into fixtures.ts and use data-testid attributes instead of CSS classes.

Layer 2 -- Playwright actionability checks (catches violations at runtime):

Every locator.click() automatically verifies the element is visible, stable, not obscured by an overlay, and enabled. Every fill() verifies the element is editable. This is the framework doing "could a human actually do this?" for you. The force:true flag bypasses all of these checks, which is why it is banned.

Together: ESLint prevents bad code from being written. Playwright prevents hidden/disabled elements from being interacted with. An agent cannot cheat in spec files unless it disables both.


## File structure

Shortcuts live in fixture/setup files. Never in spec files.

```
tests/
  fixtures/                   shortcuts: API calls, data seeding, auth
  e2e-ui/
    eslint.config.mjs         bans shortcuts in *.spec.ts (run: npm run lint:e2e)
    smoke/*.spec.ts           human-like interactions only
    full/*.spec.ts            human-like interactions only
  e2e-api/                    pure HTTP tests (no browser)
```

Spec files import helpers from fixtures.ts for the Arrange phase. The spec body only contains Act + Assert code.


## Locators

Use locators that match what the user sees.

Preferred (in order):
```
page.getByRole('button', { name: 'Submit' })
page.getByText('Welcome back')
page.getByLabel('Email address')
page.getByPlaceholder('Enter email')
page.getByTestId('submit-btn')                                        last resort
```

Banned in spec files:
```
page.locator('.btn-primary')                                           CSS class
page.locator('//div[3]/button')                                        XPath
page.locator('input[name="title"]')                                    attribute selector
page.locator('label:has-text("x")').locator('..').locator('input')     DOM traversal
```

If a form input has no visible label, the fix is in the component (add a label element or aria-label attribute). Do not work around missing labels with raw selectors in the test.

Row-scoping is allowed because it mirrors how a human finds things in a table:
```
page.locator('tr').filter({ hasText: 'Laptop' }).getByRole('button', { name: 'Edit' })
```


## Banned patterns in *.spec.ts

```
page.evaluate(...)              direct DOM manipulation
page.$eval(...)                 same
page.$$eval(...)                same
page.$(...)                     deprecated handle API
.click({ force: true })         bypasses visibility/enabled checks
page.dispatchEvent(...)         synthetic events
page.setExtraHTTPHeaders(...)   invisible to users
page.waitForTimeout(N)          use auto-waiting or assertion timeouts instead
request.get(...)                API call in spec (move to fixtures.ts)
request.post(...)               same
request.delete(...)             same
```

All of these are allowed in fixtures.ts and auth.setup.ts.


## Waiting

Never use page.waitForTimeout(). Playwright auto-waits for elements. When the default timeout is too short, use the timeout option on assertions:

```
await expect(page.getByText('Dashboard')).toBeVisible({ timeout: 10000 })
```


## Auth

auth.setup.ts logs in through the real UI,cd saves browser state to .auth/admin.json via storageState. All tests load that state automatically.


## Current violations

All known violations were fixed in the Feb 2026 browser test overhaul (sessions 103-114) and hardened in session 002 (Feb 2026). ESLint config at tests/e2e-ui/eslint.config.mjs. Run `npm run lint:e2e` to verify 0 errors across all spec files.
