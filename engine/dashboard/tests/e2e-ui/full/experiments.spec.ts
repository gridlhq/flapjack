/**
 * Browser-unmocked Full Suite — Experiments Page (Real Server)
 *
 * Arrange uses fixture helpers for API seeding/cleanup.
 * Act + Assert uses only visible UI interactions.
 */
import { test, expect } from '../../fixtures/auth.fixture';
import {
  createExperiment,
  deleteExperiment,
  startExperiment,
  stopExperiment,
  getExperimentByName,
} from '../../fixtures/api-helpers';

function makeExperimentPayload(name: string) {
  return {
    name,
    indexName: 'e2e-products',
    trafficSplit: 0.5,
    control: { name: 'control' },
    variant: {
      name: 'variant',
      queryOverrides: {
        filters: 'brand:Apple',
      },
    },
    primaryMetric: 'ctr',
    minimumDays: 14,
  };
}

test.describe('Experiments Page', () => {
  test('load-and-verify: seeded experiment renders in experiments table', async ({ page, request }) => {
    const experiment = await createExperiment(
      request,
      makeExperimentPayload(`e2e-exp-load-${Date.now()}`),
    );

    try {
      await page.goto('/experiments');

      await expect(page.getByTestId('experiments-heading')).toBeVisible({ timeout: 10_000 });
      await expect(page.getByTestId('experiments-table')).toBeVisible();

      const row = page.getByTestId(`experiment-row-${experiment.id}`);
      await expect(row).toBeVisible({ timeout: 10_000 });
      await expect(row.getByText(experiment.name)).toBeVisible();
      await expect(row.getByText('e2e-products')).toBeVisible();
      await expect(row.getByText('draft')).toBeVisible();
      await expect(row.getByText('CTR')).toBeVisible();
    } finally {
      await deleteExperiment(request, experiment.id);
    }
  });

  test('running experiment can be stopped from the list UI', async ({ page, request }) => {
    const experiment = await createExperiment(
      request,
      makeExperimentPayload(`e2e-exp-stop-${Date.now()}`),
    );
    await startExperiment(request, experiment.id);

    try {
      await page.goto('/experiments');
      await expect(page.getByTestId('experiments-heading')).toBeVisible({ timeout: 10_000 });

      await page.getByTestId(`stop-experiment-${experiment.id}`).click();
      const dialog = page.getByRole('dialog');
      await expect(dialog).toBeVisible();
      await dialog.getByRole('button', { name: /^Stop$/i }).click();

      await expect(page.getByTestId(`experiment-status-${experiment.id}`)).toContainText('stopped', {
        timeout: 10_000,
      });
      await expect(page.getByTestId(`stop-experiment-${experiment.id}`)).toHaveCount(0);
    } finally {
      await deleteExperiment(request, experiment.id);
    }
  });

  test('stopped experiment can be deleted from the list UI', async ({ page, request }) => {
    const experiment = await createExperiment(
      request,
      makeExperimentPayload(`e2e-exp-delete-${Date.now()}`),
    );
    await startExperiment(request, experiment.id);
    await stopExperiment(request, experiment.id);

    try {
      await page.goto('/experiments');
      await expect(page.getByTestId('experiments-heading')).toBeVisible({ timeout: 10_000 });

      await page.getByTestId(`delete-experiment-${experiment.id}`).click();
      const dialog = page.getByRole('dialog');
      await expect(dialog).toBeVisible();
      await dialog.getByRole('button', { name: /^Delete$/i }).click();

      await expect(page.getByTestId(`experiment-row-${experiment.id}`)).toHaveCount(0, {
        timeout: 10_000,
      });
    } finally {
      await deleteExperiment(request, experiment.id);
    }
  });
});

test.describe('Experiment Detail Page', () => {
  test('clicking experiment name in list navigates to detail page', async ({ page, request }) => {
    const experiment = await createExperiment(
      request,
      makeExperimentPayload(`e2e-exp-detail-nav-${Date.now()}`),
    );
    await startExperiment(request, experiment.id);

    try {
      await page.goto('/experiments');
      await expect(page.getByTestId('experiments-heading')).toBeVisible({ timeout: 10_000 });

      // Click the experiment name link to navigate to detail
      const row = page.getByTestId(`experiment-row-${experiment.id}`);
      await row.getByRole('link', { name: experiment.name }).click();

      // Verify detail page loaded with correct name and status
      await expect(page.getByTestId('experiment-detail-name')).toHaveText(experiment.name, {
        timeout: 10_000,
      });
      await expect(page.getByTestId('experiment-detail-status')).toContainText('running');
    } finally {
      await deleteExperiment(request, experiment.id);
    }
  });

  test('detail page shows experiment name, status, index, and metric', async ({ page, request }) => {
    const experiment = await createExperiment(
      request,
      makeExperimentPayload(`e2e-exp-detail-info-${Date.now()}`),
    );
    await startExperiment(request, experiment.id);

    try {
      await page.goto(`/experiments/${experiment.id}`);

      await expect(page.getByTestId('experiment-detail-name')).toHaveText(experiment.name, {
        timeout: 10_000,
      });
      await expect(page.getByTestId('experiment-detail-status')).toContainText('running');
      await expect(page.getByTestId('experiment-detail-index')).toHaveText('e2e-products');
      await expect(page.getByTestId('experiment-detail-primary-metric')).toHaveText('CTR');
    } finally {
      await deleteExperiment(request, experiment.id);
    }
  });

  test('detail page shows progress bar for running experiment collecting data', async ({ page, request }) => {
    const experiment = await createExperiment(
      request,
      makeExperimentPayload(`e2e-exp-detail-progress-${Date.now()}`),
    );
    await startExperiment(request, experiment.id);

    try {
      await page.goto(`/experiments/${experiment.id}`);

      await expect(page.getByTestId('experiment-detail-name')).toBeVisible({ timeout: 10_000 });
      // Progress bar should be visible for a fresh experiment with no data
      await expect(page.getByTestId('progress-bar')).toBeVisible({ timeout: 10_000 });
      await expect(page.getByText('Data collection progress')).toBeVisible();
    } finally {
      await deleteExperiment(request, experiment.id);
    }
  });

  test('detail page shows control and variant metric cards', async ({ page, request }) => {
    const experiment = await createExperiment(
      request,
      makeExperimentPayload(`e2e-exp-detail-metrics-${Date.now()}`),
    );
    await startExperiment(request, experiment.id);

    try {
      await page.goto(`/experiments/${experiment.id}`);

      await expect(page.getByTestId('experiment-detail-name')).toBeVisible({ timeout: 10_000 });
      // Both arm metric cards should be visible
      await expect(page.getByTestId('metric-card-control')).toBeVisible({ timeout: 10_000 });
      await expect(page.getByTestId('metric-card-variant')).toBeVisible();

      // Verify metric labels are rendered in each card
      const controlCard = page.getByTestId('metric-card-control');
      await expect(controlCard.getByText('Control')).toBeVisible();
      await expect(controlCard.getByText('Searches')).toBeVisible();
      await expect(controlCard.getByText('Users')).toBeVisible();
      await expect(controlCard.getByText('Clicks')).toBeVisible();

      const variantCard = page.getByTestId('metric-card-variant');
      await expect(variantCard.getByText('Variant')).toBeVisible();
      await expect(variantCard.getByText('Searches')).toBeVisible();
    } finally {
      await deleteExperiment(request, experiment.id);
    }
  });

  test('back link navigates from detail to experiments list', async ({ page, request }) => {
    const experiment = await createExperiment(
      request,
      makeExperimentPayload(`e2e-exp-detail-back-${Date.now()}`),
    );
    await startExperiment(request, experiment.id);

    try {
      await page.goto(`/experiments/${experiment.id}`);
      await expect(page.getByTestId('experiment-detail-name')).toBeVisible({ timeout: 10_000 });

      // Click the back link
      await page.getByRole('link', { name: /Experiments/i }).click();

      // Should be back on the list page
      await expect(page.getByTestId('experiments-heading')).toBeVisible({ timeout: 10_000 });
    } finally {
      await deleteExperiment(request, experiment.id);
    }
  });

  test('seeded running experiment detail page renders from seed data', async ({ page, request }) => {
    // Find the seeded experiment created in seed.setup.ts
    const seeded = await getExperimentByName(request, 'e2e-seeded-experiment');

    await page.goto(`/experiments/${seeded.id}`);

    await expect(page.getByTestId('experiment-detail-name')).toHaveText('e2e-seeded-experiment', {
      timeout: 10_000,
    });
    await expect(page.getByTestId('experiment-detail-status')).toContainText('running');
    await expect(page.getByTestId('experiment-detail-index')).toHaveText('e2e-products');
    await expect(page.getByTestId('experiment-detail-primary-metric')).toHaveText('CTR');
    await expect(page.getByTestId('metric-card-control')).toBeVisible();
    await expect(page.getByTestId('metric-card-variant')).toBeVisible();
  });

  test('stopped experiment detail shows stopped status and no declare winner', async ({ page, request }) => {
    const experiment = await createExperiment(
      request,
      makeExperimentPayload(`e2e-exp-detail-stopped-${Date.now()}`),
    );
    await startExperiment(request, experiment.id);
    await stopExperiment(request, experiment.id);

    try {
      await page.goto(`/experiments/${experiment.id}`);

      await expect(page.getByTestId('experiment-detail-name')).toBeVisible({ timeout: 10_000 });
      await expect(page.getByTestId('experiment-detail-status')).toContainText('stopped');
      // Declare Winner button should not be present for stopped experiment without sufficient data
      await expect(page.getByTestId('declare-winner-button')).toHaveCount(0);
    } finally {
      await deleteExperiment(request, experiment.id);
    }
  });
});
