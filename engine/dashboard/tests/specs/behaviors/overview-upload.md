# Overview Upload — Tier 1: BDD Behavior Specifications

## B-OVER-001: Upload Index Snapshot from Overview

**As a** dashboard administrator
**I want to** upload an index snapshot from the Overview page
**So that** I can restore or import data without navigating to individual index pages

**Acceptance Criteria:**
- An "Upload" button is visible in the Overview page header alongside "Export All" and "Create Index"
- Clicking "Upload" opens a file picker for .tar.gz files
- After selecting a file, user is prompted to choose a target index from existing indexes
- Upload starts and shows progress/loading state
- Success or error feedback is shown after upload completes
