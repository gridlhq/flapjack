# API Logs — Tier 2: Test Specifications

**Maps to:** B-LOG-001 through B-LOG-006
**Test Type:** E2E-UI (route mocking, no real backend)
**Prerequisites:** Dashboard dev server running

---

## Test Data Setup

API logs are captured by the Axios interceptor in `src/lib/api.ts` and stored in a Zustand store
with sessionStorage persistence. For e2e-ui tests, logs are generated naturally by navigating
through the dashboard (each page load triggers health/indexes API calls which populate the log).

**Expected log entries after page load:**
- GET /health → 200, duration > 0ms
- GET /1/indexes → 200, duration > 0ms

---

## B-LOG-001: View API Request Log

### TEST: Log entries show method, URL, status, and duration
**Execute:**
1. Mock APIs for search page (generates API calls that populate the log)
2. Navigate to `/index/test-index`
3. Wait for page load (generates API calls)
4. Navigate to `/logs`
5. Wait for [data-testid="logs-list"] visible

**Verify UI:**
- Log entries are visible in the list
- At least one entry shows a green checkmark icon (success status)
- At least one entry shows method text (GET or POST)
- At least one entry shows a URL containing "/health" or "/1/indexes"
- At least one entry shows a non-zero duration (e.g., matches /\d+ms/ or /\d+\.\d+s/)

**Verify API:**
- No specific API calls (logs are client-side)

---

## B-LOG-002: Expand Log Entry for Details

### TEST: Expanding entry shows request body and response JSON
**Execute:**
1. Mock APIs for search page
2. Navigate to `/index/test-index` (generates search API call with body)
3. Navigate to `/logs`
4. Click first log entry row

**Verify UI:**
- An expanded detail card appears below the clicked row
- Detail card contains "Response" heading
- Detail card contains formatted JSON (pre element with JSON content)
- Clicking the same row again collapses the detail

---

## B-LOG-003: Toggle Curl Command View

### TEST: Curl toggle switches display mode
**Execute:**
1. Mock APIs for search page
2. Navigate to `/index/test-index`
3. Navigate to `/logs`
4. Find the view toggle and switch to "Curl" mode

**Verify UI:**
- Log entries display curl commands instead of method+URL lines
- Each curl command starts with "curl"
- Each curl command contains "-X GET" or "-X POST"
- Each curl command contains a URL
- Switching back to "Endpoint" mode restores original display

---

## B-LOG-004: Copy Log Entry

### TEST: Copy button copies entry text
**Execute:**
1. Mock APIs for search page
2. Navigate to `/index/test-index`
3. Navigate to `/logs`
4. Click copy button on first log entry

**Verify UI:**
- Copy button shows visual feedback (checkmark icon replaces copy icon)
- Clipboard contains the entry text (method + URL in Endpoint mode)

---

## B-LOG-005: Export All Logs

### TEST: Export downloads a bash script file
**Execute:**
1. Mock APIs for search page
2. Navigate to `/index/test-index`
3. Navigate to `/logs`
4. Click "Export" button

**Verify UI:**
- Download is triggered (verify via download event listener)

---

## B-LOG-006: Clear Log Entries

### TEST: Clear button empties the log list
**Execute:**
1. Mock APIs for search page
2. Navigate to `/index/test-index`
3. Navigate to `/logs`
4. Verify log entries exist
5. Click "Clear" button

**Verify UI:**
- Log list disappears
- Empty state message "No API logs" is visible
- Badge shows "0 requests"
