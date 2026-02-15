# API Logs — Tier 1: BDD Behavior Specifications

## B-LOG-001: View API Request Log

**As a** developer using the dashboard
**I want to** see a log of all API requests with method, URL, status, and duration
**So that** I can monitor and debug API interactions

**Acceptance Criteria:**
- Each log entry shows HTTP method (GET/POST/PUT/DELETE) with color coding
- Each log entry shows the request URL
- Each log entry shows a status icon (spinner=pending, checkmark=success, X=error)
- Each log entry shows the response duration in milliseconds
- Duration is calculated from request start to response received
- Entries are ordered newest-first

---

## B-LOG-002: Expand Log Entry for Details

**As a** developer debugging an API issue
**I want to** click a log entry to see full request/response details
**So that** I can inspect the exact data sent and received

**Acceptance Criteria:**
- Clicking a log row expands an inline detail panel below it
- Detail panel shows the request body as formatted JSON
- Detail panel shows the response body as formatted JSON
- Detail panel shows response status code and headers summary
- Clicking again collapses the panel
- Only one entry can be expanded at a time

---

## B-LOG-003: Toggle Curl Command View

**As a** developer who wants to reproduce requests
**I want to** toggle the log display to show full curl commands
**So that** I can copy and run requests from my terminal

**Acceptance Criteria:**
- A toggle control switches between "Endpoint" and "Curl" view modes
- In Curl mode, each entry shows a complete curl command with method, URL, headers, and body
- Curl commands include all relevant headers (Content-Type, auth headers)
- The curl command can be run directly in a terminal

---

## B-LOG-004: Copy Log Entry

**As a** developer sharing API details with teammates
**I want to** copy an individual log entry to my clipboard
**So that** I can paste it into a chat, ticket, or terminal

**Acceptance Criteria:**
- Each log entry row has a copy button
- In Endpoint mode, copy button copies the method + URL line
- In Curl mode, copy button copies the full curl command
- Visual feedback confirms the copy succeeded (e.g., checkmark icon)

---

## B-LOG-005: Export All Logs as Bash Script

**As a** developer creating reproducible test scripts
**I want to** export all logged API calls as a bash script
**So that** I can replay or share them

**Acceptance Criteria:**
- Export button downloads a .sh file
- File contains commented curl commands for each request
- File has a bash shebang and timestamp header
- Commands are in chronological order

---

## B-LOG-006: Clear Log Entries

**As a** developer starting a fresh debugging session
**I want to** clear all API log entries
**So that** I can focus on new requests without noise

**Acceptance Criteria:**
- Clear button removes all entries from the log
- Log shows empty state message after clearing
- New API calls after clearing appear normally
