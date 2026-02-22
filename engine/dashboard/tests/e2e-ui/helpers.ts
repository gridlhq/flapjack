/**
 * Shared helpers for e2e-ui tests.
 * NO MOCKS. These tests run against a real Flapjack server.
 */
import { API_BASE, API_HEADERS } from '../fixtures/local-instance';

// Backend connection
export { API_BASE, API_HEADERS };

// Test index — seeded in seed.setup.ts, cleaned in cleanup.setup.ts
export const TEST_INDEX = 'e2e-products';

// Re-export auth fixture so test files only need one import
export { test, expect } from '../fixtures/auth.fixture';
