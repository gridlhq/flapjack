# Analytics Management — Tier 1: BDD Behavior Specifications

## B-ANA-008: Clear Analytics with Confirmation Dialog

**As a** dashboard administrator
**I want to** clear analytics data with a proper confirmation dialog
**So that** I don't accidentally delete data and I see the UI update immediately

**Acceptance Criteria:**
- Clicking "Clear Analytics" opens a styled confirmation dialog (not browser confirm)
- Dialog shows the index name being cleared
- Dialog has "Cancel" and "Clear" buttons with destructive styling
- Confirming the dialog deletes analytics and immediately shows 0 in all KPIs
- Canceling the dialog leaves data intact
- After clearing, the cache is fully reset (not just invalidated)

---

## B-ANA-009: Update (Flush) Analytics

**As a** dashboard user
**I want to** flush buffered analytics and see fresh data
**So that** I can view the most recent analytics without waiting for auto-flush

**Acceptance Criteria:**
- Clicking "Update" triggers an analytics flush to disk
- Button shows loading state during flush
- After flush completes, all analytics data refreshes
- KPIs and charts update with latest data

---

## B-ANA-010: Analytics BETA Label

**As a** dashboard user
**I want to** see a prominent BETA label on the analytics page
**So that** I understand the feature is in beta and data may have quirks

**Acceptance Criteria:**
- A BETA badge is visible at the top of the Analytics page
- Badge uses orange or red styling for prominence
- Badge is positioned near the page heading
