/**
 * ESLint config for E2E-UI spec files.
 *
 * Enforces BROWSER_TESTING_STANDARDS_2.md rules:
 * - No page.evaluate / page.$eval / page.$$eval / page.$()
 * - No CSS class / XPath / attribute selectors in spec files
 * - No { force: true } on actions
 * - No page.pause() (debugging leftover)
 * - No API calls (request.*), waitForTimeout, dispatchEvent in spec files
 *
 * Fixture and setup files are exempt — only *.spec.ts files are linted.
 *
 * Note: Tag-based locators (table, tr, td, th, svg, etc.) are allowed for
 * row-scoping per BROWSER_TESTING_STANDARDS_2.md. Only CSS class selectors,
 * XPath, and attribute selectors are banned.
 */
import playwright from 'eslint-plugin-playwright';
import tseslint from 'typescript-eslint';

export default tseslint.config(
  {
    // Only lint spec files — fixtures/setup files are exempt
    files: ['**/*.spec.ts'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
      },
    },
    plugins: {
      playwright,
    },
    rules: {
      // --- Layer 1: Playwright ESLint rules ---

      // Ban page.evaluate(), page.$eval(), page.$$eval()
      'playwright/no-eval': 'error',

      // Ban deprecated page.$() / page.$$() handle API
      'playwright/no-element-handle': 'error',

      // Ban { force: true } which bypasses actionability checks
      'playwright/no-force-option': 'error',

      // Ban page.pause() (debugging leftover)
      'playwright/no-page-pause': 'error',

      // --- Layer 1: Custom banned patterns ---
      'no-restricted-syntax': ['error',
        {
          selector: "CallExpression[callee.object.name='request'][callee.property.name='get']",
          message: 'API calls (request.get) are banned in spec files. Move to fixtures.ts.',
        },
        {
          selector: "CallExpression[callee.object.name='request'][callee.property.name='post']",
          message: 'API calls (request.post) are banned in spec files. Move to fixtures.ts.',
        },
        {
          selector: "CallExpression[callee.object.name='request'][callee.property.name='delete']",
          message: 'API calls (request.delete) are banned in spec files. Move to fixtures.ts.',
        },
        {
          selector: "CallExpression[callee.object.name='request'][callee.property.name='put']",
          message: 'API calls (request.put) are banned in spec files. Move to fixtures.ts.',
        },
        {
          selector: "CallExpression[callee.object.name='request'][callee.property.name='patch']",
          message: 'API calls (request.patch) are banned in spec files. Move to fixtures.ts.',
        },
        {
          selector: "CallExpression[callee.property.name='waitForTimeout']",
          message: 'waitForTimeout is banned. Use Playwright auto-waiting or assertion timeouts instead.',
        },
        {
          selector: "CallExpression[callee.property.name='dispatchEvent']",
          message: 'dispatchEvent is banned. Use real user interactions (click, fill, etc.).',
        },
        {
          selector: "CallExpression[callee.property.name='setExtraHTTPHeaders']",
          message: 'setExtraHTTPHeaders is banned in spec files. Move to fixtures.ts.',
        },
        // Ban CSS class selectors: .locator('.someClass') or .locator('.some-class')
        {
          selector: "CallExpression[callee.property.name='locator'] > Literal[value=/^\\./]",
          message: 'CSS class selectors (.className) are banned in spec files. Use data-testid or getByRole/getByText instead.',
        },
        // Ban XPath selectors: .locator('//...')
        {
          selector: "CallExpression[callee.property.name='locator'] > Literal[value=/^\\/\\//]",
          message: 'XPath selectors are banned in spec files. Use data-testid or getByRole/getByText instead.',
        },
        // Ban attribute selectors: .locator('[attr=value]') or .locator('input[name=...]')
        {
          selector: "CallExpression[callee.property.name='locator'] > Literal[value=/\\[.*=/]",
          message: 'Attribute selectors are banned in spec files. Use data-testid or getByRole/getByText instead.',
        },
      ],
    },
  },
);
