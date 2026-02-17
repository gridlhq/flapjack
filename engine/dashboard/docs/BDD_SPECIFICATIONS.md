# BDD Specifications — Tier 1: User Stories

**Context:** Flapjack Dashboard is a React admin UI for managing Flapjack search indices.
**Testing Philosophy:** 3-tier BDD approach with zero manual QA.

---

## Analytics

### B-ANA-001: View Analytics Overview
**As a** search administrator
**I want to** view high-level analytics metrics
**So that** I can understand search performance at a glance

**Acceptance Criteria:**
- Dashboard displays total searches, unique users, no-result rate KPIs
- Charts show search volume and no-result rate trends over time
- Top 10 searches table shows most popular queries
- Date range selector (7d, 30d, 90d) filters all data
- Delta badges show period-over-period changes

### B-ANA-002: Analyze Search Queries
**As a** search administrator
**I want to** view detailed search query analytics
**So that** I can understand what users are searching for

**Acceptance Criteria:**
- Table shows all search queries ranked by count (descending)
- Each query shows: query text, count, average hits
- Filter queries by text (client-side)
- Filter queries by country (server-side)
- Filter queries by device/platform (server-side)
- Sortable by count, query text, average hits

### B-ANA-003: Identify No-Result Searches
**As a** search administrator
**I want to** see queries that returned zero results
**So that** I can improve index coverage

**Acceptance Criteria:**
- Banner shows overall no-result rate percentage
- Table lists all zero-result queries with counts
- Queries are sorted by frequency
- Empty state shown when no zero-result queries exist

### B-ANA-004: Understand Device Distribution
**As a** search administrator
**I want to** see device/platform breakdown
**So that** I can optimize for user devices

**Acceptance Criteria:**
- Cards show desktop, mobile, tablet counts and percentages
- Percentages sum to 100%
- Stacked area chart shows device trends over time
- Empty state shown when no device data exists

### B-ANA-005: Analyze Geographic Distribution
**As a** search administrator
**I want to** see where searches originate
**So that** I can understand geographic usage

**Acceptance Criteria:**
- Country table shows all countries sorted by count
- Each country shows: name, code, count, percentage
- Clicking a country drills down to show:
  - Top searches for that country
  - Regional/state breakdown (for supported countries)
- Back button returns to country list
- Empty state shown when no geographic data exists

### B-ANA-006: Analyze Filter Usage
**As a** search administrator
**I want to** see which filters users apply
**So that** I can understand filtering behavior

**Acceptance Criteria:**
- Table shows all filters with counts
- Expandable rows show filter values
- Section shows filters that cause no results
- Empty state shown when no filter data exists

### B-ANA-007: Manage Analytics Data
**As a** search administrator
**I want to** update and clear analytics
**So that** I can manage analytics lifecycle

**Acceptance Criteria:**
- Update button flushes pending analytics to storage
- Clear button prompts for confirmation
- Clear button deletes all analytics for the index
- Canceling confirmation does not delete data

---

## Index Management

### B-IDX-001: View Index Overview
**As a** search administrator
**I want to** see all my search indices
**So that** I can manage them

**Acceptance Criteria:**
- Overview page lists all indices
- Each index shows: name, document count, size
- Search bar filters indices by name
- Click an index to view details

### B-IDX-002: Create New Index
**As a** search administrator
**I want to** create a new search index
**So that** I can add searchable content

**Acceptance Criteria:**
- Create button opens modal
- Modal requires index name
- Name validation (alphanumeric + hyphens)
- Success creates index and navigates to it
- Error shows validation message

### B-IDX-003: Delete Index
**As a** search administrator
**I want to** delete an index
**So that** I can remove unused indices

**Acceptance Criteria:**
- Delete button shows confirmation dialog
- Dialog shows index name
- Confirming deletes index and returns to overview
- Canceling does not delete
- Cannot delete while on index detail page

---

## Search & Browse

### B-SRH-001: Search Index Documents
**As a** search administrator
**I want to** search my index
**So that** I can verify search results

**Acceptance Criteria:**
- Search box performs queries against index
- Results update as I type (debounced)
- Results show: objectID, searchable fields
- Facets filter results
- Pagination works for large result sets

### B-SRH-002: Browse All Documents
**As a** search administrator
**I want to** browse all documents
**So that** I can review index contents

**Acceptance Criteria:**
- Empty query shows all documents
- Results are paginated
- Each document shows all fields
- Click document to view full details

---

## Synonyms

### B-SYN-001: Manage Synonyms
**As a** search administrator
**I want to** create and edit synonyms
**So that** I can improve search relevance

**Acceptance Criteria:**
- Table lists all synonyms
- Create button opens modal
- Modal requires synonym set (comma-separated)
- Save creates synonym and updates table
- Delete removes synonym
- Changes are immediately reflected

---

## Rules & Merchandising

### B-RUL-001: Create Query Rules
**As a** search administrator
**I want to** create rules that modify search results
**So that** I can customize user experience

**Acceptance Criteria:**
- Table lists all rules
- Create button opens modal
- Rule requires: condition pattern, consequence (pin/hide/boost)
- Save creates rule
- Rules are applied to searches
- Delete removes rule

---

## API Keys

### B-KEY-001: Manage API Keys
**As a** search administrator
**I want to** view and create API keys
**So that** I can control access

**Acceptance Criteria:**
- Table shows all API keys
- Each key shows: name, value, created date
- Create button generates new key
- Delete removes key
- Copy button copies key to clipboard

---

## Settings

### B-SET-001: Configure Index Settings
**As a** search administrator
**I want to** configure searchable attributes
**So that** I can control search behavior

**Acceptance Criteria:**
- Settings form shows current configuration
- Searchable attributes are orderable
- Save button updates settings
- Validation prevents invalid configurations

---

## System

### B-SYS-001: View System Health
**As a** search administrator
**I want to** see system health metrics
**So that** I can monitor uptime

**Acceptance Criteria:**
- Health page shows server status
- Metrics: uptime, memory, CPU, indices count
- Error state when server unreachable
