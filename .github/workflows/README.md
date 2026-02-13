# Flapjack CI/CD Workflows

This directory contains GitHub Actions workflows that are synced to the public `gridlhq/flapjack` repository.

## How It Works

1. **Development (flapjack_dev)**: Tests are run manually via the script
   ```bash
   ./engine/_dev/s/run-all-tests.sh
   ```

2. **Public Repo (gridlhq/flapjack)**: Tests run automatically
   - On every push to `main` (via sync-and-deploy.sh)
   - Nightly at 2 AM UTC (comprehensive test suite)

## Workflows

### ci.yml - Continuous Integration

Runs on every push to `main` in the public repo only.

**Tests included:**
- Rust engine (rustfmt, clippy, fast tests)
- Rust engine (all tests) - main branch only
- Installer tests (Ubuntu + macOS)
- Dashboard (unit tests, build, page tests)
- Dashboard integration tests (main branch only, requires Algolia secrets)
- All SDKs (PHP 8.1-8.3, Python 3.9-3.12, JS, Go 1.21-1.23, Ruby 3.1-3.3, Java, C#)
- Integrations (Laravel Scout, WordPress)

**Repository Check:**
All jobs check `github.repository == 'gridlhq/flapjack'` to ensure they only run in the public repo.

### nightly.yml - Comprehensive Nightly Tests

Runs every night at 2 AM UTC on the public repo only.

**Additional coverage:**
- Extended version matrices (PHP 8.4, Python 3.13, Node 18/20/22, Java 17/21, .NET 7/8)
- All Rust tests (not just fast subset)
- Dashboard integration tests
- Cross-platform installer tests
- Full SDK compatibility matrix

## Sync Process

The `sync-and-deploy.sh` script automatically syncs these workflows to the public repo:

```bash
cd engine/_dev/s
./sync-and-deploy.sh "commit message"
```

This script:
1. Syncs files including `.github/workflows/`
2. Runs tests in the public repo
3. Commits and pushes to `gridlhq/flapjack`
4. Triggers the CI workflow on push to main

## Required GitHub Secrets

Set these in the public repo settings (`gridlhq/flapjack`):

- `ALGOLIA_APP_ID` - For integration tests
- `ALGOLIA_ADMIN_KEY` - For integration tests

## Local Development

To run the full test suite locally in flapjack_dev:

```bash
# Run all tests (equivalent to CI)
./engine/_dev/s/run-all-tests.sh

# With Algolia credentials for integration tests
export ALGOLIA_APP_ID="your-app-id"
export ALGOLIA_ADMIN_KEY="your-admin-key"
./engine/_dev/s/run-all-tests.sh
```

## Workflow Design

The workflows use a tiered approach:

- **Fast tests on every push**: Essential checks that run quickly
- **Comprehensive tests on main**: Full test suite after merge
- **Nightly tests**: Extended compatibility matrix, all versions

This balances speed (fast PR feedback) with coverage (catch edge cases).
