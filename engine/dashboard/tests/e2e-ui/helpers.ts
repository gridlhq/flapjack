/**
 * Shared helpers for e2e-ui tests.
 * NO MOCKS. These tests run against a real Flapjack server.
 */

// Backend connection
export const API_BASE = 'http://localhost:7700';
export const API_HEADERS = {
  'x-algolia-application-id': 'flapjack',
  'x-algolia-api-key': 'fj_devtestadminkey000000',
  'Content-Type': 'application/json',
};

// Test index — seeded in seed.setup.ts, cleaned in cleanup.setup.ts
export const TEST_INDEX = 'e2e-products';

// Re-export auth fixture so test files only need one import
export { test, expect } from '../fixtures/auth.fixture';
