/**
 * E2E-UI Full Suite — Hybrid Search Controls (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests run against a REAL Flapjack server with seeded test data.
 *
 * Uses a dedicated index (e2e-hybrid) to avoid race conditions with
 * vector-settings tests that run in parallel on the shared e2e-products index.
 *
 * Covers:
 * - Hybrid controls hidden when no embedders configured
 * - Hybrid controls visible when embedders configured
 * - Semantic ratio slider label updates
 * - Search results appear when hybrid search is active
 */
import { test, expect } from '../../fixtures/auth.fixture';
import {
  addDocuments,
  deleteIndex,
  configureEmbedder,
  clearEmbedders,
  addDocumentsWithVectors,
} from '../../fixtures/api-helpers';

// Dedicated index — avoids race condition with vector-settings tests
// that modify the shared e2e-products index in parallel.
const HYBRID_INDEX = 'e2e-hybrid';

test.describe('Hybrid Search Controls', () => {
  // Tests modify shared index settings — must run serially (not in parallel)
  test.describe.configure({ mode: 'serial' });

  test.beforeAll(async ({ request }) => {
    // Create a fresh index with docs for search tests
    await deleteIndex(request, HYBRID_INDEX);
    await addDocuments(request, HYBRID_INDEX, [
      { objectID: 'h-1', name: 'Hybrid Doc Alpha', category: 'Test' },
      { objectID: 'h-2', name: 'Hybrid Doc Beta', category: 'Test' },
    ]);
    // Explicitly clear embedders in case the index retained stale config
    await clearEmbedders(request, HYBRID_INDEX);
  });

  test.afterAll(async ({ request }) => {
    await deleteIndex(request, HYBRID_INDEX);
  });

  test('hybrid controls hidden when no embedders configured', async ({
    page,
  }) => {
    // Index has no embedders configured
    await page.goto(`/index/${HYBRID_INDEX}`);
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i)),
    ).toBeVisible({ timeout: 15_000 });

    // Hybrid controls should NOT be visible
    await expect(page.getByTestId('hybrid-controls')).not.toBeVisible();
  });

  test('hybrid controls visible when embedders configured', async ({
    request,
    page,
  }) => {
    // Seed embedder
    await configureEmbedder(request, HYBRID_INDEX, 'default', {
      source: 'userProvided',
      dimensions: 384,
    });

    await page.goto(`/index/${HYBRID_INDEX}`);
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i)),
    ).toBeVisible({ timeout: 15_000 });

    // Hybrid controls should be visible
    await expect(page.getByTestId('hybrid-controls')).toBeVisible({
      timeout: 10_000,
    });
    await expect(page.getByText('Hybrid Search')).toBeVisible();
    await expect(page.getByTestId('semantic-ratio-slider')).toBeVisible();
    await expect(page.getByTestId('semantic-ratio-label')).toBeVisible();
    await expect(page.getByTestId('semantic-ratio-label')).toHaveText(
      'Balanced',
    );
  });

  test('semantic ratio slider updates label', async ({ request, page }) => {
    // Ensure embedder is configured (may already be from previous test)
    await configureEmbedder(request, HYBRID_INDEX, 'default', {
      source: 'userProvided',
      dimensions: 384,
    });

    await page.goto(`/index/${HYBRID_INDEX}`);
    await expect(page.getByTestId('hybrid-controls')).toBeVisible({
      timeout: 15_000,
    });

    // Change slider value to 1.0 (semantic only)
    const slider = page.getByTestId('semantic-ratio-slider');
    await slider.fill('1');

    // Label should update
    await expect(page.getByTestId('semantic-ratio-label')).toHaveText(
      'Semantic only',
    );

    // Change to 0 (keyword only)
    await slider.fill('0');
    await expect(page.getByTestId('semantic-ratio-label')).toHaveText(
      'Keyword only',
    );
  });

  test('search results appear with hybrid search active', async ({
    request,
    page,
  }) => {
    // Seed embedder + docs with vectors
    await configureEmbedder(request, HYBRID_INDEX, 'default', {
      source: 'userProvided',
      dimensions: 384,
    });
    await addDocumentsWithVectors(request, HYBRID_INDEX, [
      {
        objectID: 'vec-1',
        name: 'Vector Laptop',
        category: 'Laptops',
        _vectors: { default: new Array(384).fill(0.1) },
      },
      {
        objectID: 'vec-2',
        name: 'Vector Phone',
        category: 'Phones',
        _vectors: { default: new Array(384).fill(0.2) },
      },
    ]);

    await page.goto(`/index/${HYBRID_INDEX}`);
    await expect(page.getByTestId('hybrid-controls')).toBeVisible({
      timeout: 15_000,
    });

    // Perform a search
    const searchInput = page.getByPlaceholder(/search documents/i);
    await searchInput.fill('laptop');
    await searchInput.press('Enter');

    // Verify actual result content appears (not just that the panel exists)
    await expect(page.getByText('Vector Laptop')).toBeVisible({
      timeout: 10_000,
    });
  });
});
