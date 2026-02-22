/**
 * ESLint flat config for the Flapjack Dashboard.
 *
 * Uses typescript-eslint for TypeScript parsing and recommended rules.
 * E2E spec files have their own stricter config at tests/e2e-ui/eslint.config.mjs.
 */
import tseslint from 'typescript-eslint';

export default tseslint.config(
  {
    ignores: [
      'dist/**',
      'node_modules/**',
      'tests/e2e-ui/**',
      'tests/e2e-api/**',
      'tests/fixtures/**',
      '*.config.*',
    ],
  },
  ...tseslint.configs.recommended,
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        ecmaVersion: 'latest',
        sourceType: 'module',
      },
    },
    linterOptions: {
      reportUnusedDisableDirectives: 'error',
    },
    rules: {
      // Allow unused vars prefixed with underscore
      '@typescript-eslint/no-unused-vars': ['warn', {
        argsIgnorePattern: '^_',
        varsIgnorePattern: '^_',
      }],
      // Codebase uses `any` extensively — not fixing 140+ instances
      '@typescript-eslint/no-explicit-any': 'off',
      // Radix UI component patterns use empty interfaces extending HTML props
      '@typescript-eslint/no-empty-object-type': 'off',
    },
  },
);
