#!/bin/sh
# tests/test_install.sh — Unit and integration tests for install.sh
#
# Usage:
#   ./tests/test_install.sh           # Run all tests
#   GITHUB_TOKEN=xxx ./tests/test_install.sh  # Include private-repo download tests
#
# Tests are split into:
#   1. Unit tests (no network) — validate platform detection, PATH logic, etc.
#   2. Integration tests (network) — validate actual downloads (requires GITHUB_TOKEN for private repos)

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
INSTALL_SCRIPT="${REPO_DIR}/install.sh"

# ── Test Helpers ─────────────────────────────────────────────────────────────

TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

pass() {
  TESTS_PASSED=$((TESTS_PASSED + 1))
  TESTS_RUN=$((TESTS_RUN + 1))
  printf "  \033[0;32m✓\033[0m %s\n" "$1"
}

fail() {
  TESTS_FAILED=$((TESTS_FAILED + 1))
  TESTS_RUN=$((TESTS_RUN + 1))
  printf "  \033[0;31m✗\033[0m %s\n" "$1"
  if [ -n "${2:-}" ]; then
    printf "    %s\n" "$2"
  fi
}

section() {
  printf "\n\033[1m%s\033[0m\n" "$1"
}

# ── Unit Tests ───────────────────────────────────────────────────────────────

section "Install Script Syntax & Structure"

# Test 1: Script is valid POSIX shell
if sh -n "$INSTALL_SCRIPT" 2>/dev/null; then
  pass "install.sh passes POSIX shell syntax check"
else
  fail "install.sh has shell syntax errors"
fi

# Test 2: Script starts with proper shebang
first_line=$(head -1 "$INSTALL_SCRIPT")
if [ "$first_line" = "#!/bin/sh" ]; then
  pass "Shebang is #!/bin/sh (POSIX compatible)"
else
  fail "Shebang should be #!/bin/sh, got: $first_line"
fi

# Test 3: set -eu is present (fail-fast)
if grep -q '^set -eu' "$INSTALL_SCRIPT"; then
  pass "set -eu present (fail-fast mode)"
else
  fail "set -eu not found — script won't fail on errors"
fi

# Test 4: Script is executable
if [ -x "$INSTALL_SCRIPT" ]; then
  pass "install.sh is executable"
else
  fail "install.sh is not executable"
fi

# ── Configuration Defaults ───────────────────────────────────────────────────

section "Configuration Defaults"

# Test 5: REPO default is set
if grep -q 'REPO=.*stuartcrobinson/flapjack202511' "$INSTALL_SCRIPT"; then
  pass "Default REPO is stuartcrobinson/flapjack202511 (staging)"
elif grep -q 'REPO=.*flapjackhq/flapjack' "$INSTALL_SCRIPT"; then
  pass "Default REPO is flapjackhq/flapjack (prod)"
else
  fail "No recognized default REPO found"
fi

# Test 6: BINARY_NAME is flapjack
if grep -q 'BINARY_NAME="flapjack"' "$INSTALL_SCRIPT"; then
  pass "BINARY_NAME is flapjack"
else
  fail "BINARY_NAME is not flapjack"
fi

# Test 7: Install dir defaults to ~/.flapjack/bin
if grep -q 'INSTALL_DIR=.*HOME/.flapjack.*/bin' "$INSTALL_SCRIPT"; then
  pass "Default install dir is ~/.flapjack/bin"
else
  fail "Default install dir not found"
fi

# ── Platform Detection ───────────────────────────────────────────────────────

section "Platform Detection"

# Test 8: All four Rust target triples are present
for target in "x86_64-unknown-linux-musl" "aarch64-unknown-linux-musl" "x86_64-apple-darwin" "aarch64-apple-darwin"; do
  if grep -q "$target" "$INSTALL_SCRIPT"; then
    pass "Target triple present: $target"
  else
    fail "Target triple missing: $target"
  fi
done

# Test 9: Rosetta 2 detection exists
if grep -q "sysctl.proc_translated" "$INSTALL_SCRIPT"; then
  pass "Rosetta 2 detection present"
else
  fail "Rosetta 2 detection missing"
fi

# Test 10: Windows detection with helpful error
if grep -q "MINGW\|MSYS\|CYGWIN" "$INSTALL_SCRIPT"; then
  pass "Windows detection present (with error message)"
else
  fail "Windows detection missing"
fi

# ── Download Tool Detection ──────────────────────────────────────────────────

section "Download Tool Detection"

# Test 11: curl support
if grep -q 'command -v curl' "$INSTALL_SCRIPT"; then
  pass "curl detection present"
else
  fail "curl detection missing"
fi

# Test 12: wget fallback
if grep -q 'command -v wget' "$INSTALL_SCRIPT"; then
  pass "wget fallback present"
else
  fail "wget fallback missing"
fi

# ── Version Resolution ───────────────────────────────────────────────────────

section "Version Resolution"

# Test 13: FLAPJACK_VERSION env var support
if grep -q 'FLAPJACK_VERSION' "$INSTALL_SCRIPT"; then
  pass "FLAPJACK_VERSION env var support"
else
  fail "FLAPJACK_VERSION env var not supported"
fi

# Test 14: CLI argument version pinning
if grep -q '${1:-}' "$INSTALL_SCRIPT" || grep -q '"$1"' "$INSTALL_SCRIPT"; then
  pass "CLI argument version pinning supported"
else
  fail "CLI argument version pinning not found"
fi

# Test 15: GitHub API latest release detection
if grep -q 'api.github.com/repos.*releases/latest' "$INSTALL_SCRIPT"; then
  pass "GitHub API latest release detection"
else
  fail "GitHub API latest release detection missing"
fi

# ── Security Features ────────────────────────────────────────────────────────

section "Security Features"

# Test 16: SHA256 checksum verification (with -c flag for actual file verification)
if grep -q 'shasum -a 256 -c' "$INSTALL_SCRIPT" && grep -q 'sha256sum -c' "$INSTALL_SCRIPT"; then
  pass "SHA256 checksum verification (shasum -c + sha256sum -c)"
else
  fail "SHA256 checksum verification incomplete (missing -c flag)"
fi

# Test 17: Checksum failure exits with error code
if grep -A 3 'Checksum verification FAILED' "$INSTALL_SCRIPT" | grep -q 'exit'; then
  pass "Checksum failure causes exit"
else
  fail "Checksum failure message found but no exit statement"
fi

# Test 18: GITHUB_TOKEN support for private repos
if grep -q 'GITHUB_TOKEN' "$INSTALL_SCRIPT"; then
  pass "GITHUB_TOKEN support for private repos"
else
  fail "GITHUB_TOKEN support missing"
fi

# Test 19: GitHub API asset download (for private repos)
if grep -q 'application/octet-stream' "$INSTALL_SCRIPT"; then
  pass "GitHub API asset download (Accept: application/octet-stream)"
else
  fail "GitHub API asset download not implemented"
fi

# ── PATH Management ──────────────────────────────────────────────────────────

section "PATH Management"

# Test 20: Bash profile update
if grep -q '.bashrc' "$INSTALL_SCRIPT" && grep -q '.bash_profile' "$INSTALL_SCRIPT"; then
  pass "Bash profile update (.bashrc/.bash_profile)"
else
  fail "Bash profile update incomplete"
fi

# Test 21: Zsh profile update
if grep -q '.zshrc' "$INSTALL_SCRIPT"; then
  pass "Zsh profile update (.zshrc)"
else
  fail "Zsh profile update missing"
fi

# Test 22: Fish config update
if grep -q 'config.fish' "$INSTALL_SCRIPT"; then
  pass "Fish config update"
else
  fail "Fish config update missing"
fi

# Test 23: NO_MODIFY_PATH support
if grep -q 'NO_MODIFY_PATH' "$INSTALL_SCRIPT"; then
  pass "NO_MODIFY_PATH env var supported"
else
  fail "NO_MODIFY_PATH not supported"
fi

# Test 24: Idempotent PATH update (won't add duplicate)
if grep -q 'grep -qF.*INSTALL_DIR' "$INSTALL_SCRIPT"; then
  pass "Idempotent PATH update (checks for existing entry)"
else
  fail "PATH update may not be idempotent"
fi

# Test 25: Permission-denied handling for shell profiles
if grep -q 'permission denied' "$INSTALL_SCRIPT"; then
  pass "Permission-denied handling for shell profiles"
else
  fail "No permission-denied handling for shell profiles"
fi

# ── Environment Variable Overrides ───────────────────────────────────────────

section "Environment Variable Overrides"

for var in FLAPJACK_INSTALL FLAPJACK_REPO FLAPJACK_VERSION GITHUB_TOKEN NO_MODIFY_PATH; do
  if grep -q "$var" "$INSTALL_SCRIPT"; then
    pass "Env var override: $var"
  else
    fail "Env var override missing: $var"
  fi
done

# ── Functional PATH Setup Tests (no network, sandboxed HOME) ────────────────

section "PATH Setup (functional)"

# Helper: extract setup_path and dependencies from install.sh, run in a sandboxed HOME
run_setup_path() {
  _fake_home="$1"
  _install_dir="$2"

  # Run a minimal version of the installer's setup_path in a sandboxed env.
  # We source the relevant functions then call setup_path().
  HOME="$_fake_home" INSTALL_DIR="$_install_dir" NO_MODIFY_PATH=0 \
    sh -c '
    HOME='"'$_fake_home'"'
    INSTALL_DIR='"'$_install_dir'"'
    setup_colors() { RED="" GREEN="" YELLOW="" BLUE="" BOLD="" NC=""; }
    info()  { printf "info  %s\n" "$1"; }
    warn()  { printf "warn  %s\n" "$1"; }

    '"$(sed -n '/^setup_path()/,/^}/p' "$INSTALL_SCRIPT")"'

    setup_colors
    setup_path
  ' 2>&1
}

# Test: Updates .bashrc when it exists
_td=$(mktemp -d)
mkdir -p "$_td"
touch "$_td/.bashrc"
run_setup_path "$_td" "/fake/bin" > /dev/null
if grep -qF "/fake/bin" "$_td/.bashrc" 2>/dev/null; then
  pass "PATH setup: updates .bashrc when it exists"
else
  fail "PATH setup: did not update .bashrc"
fi
rm -rf "$_td"

# Test: Updates .zshrc when it exists
_td=$(mktemp -d)
touch "$_td/.zshrc"
run_setup_path "$_td" "/fake/bin" > /dev/null
if grep -qF "/fake/bin" "$_td/.zshrc" 2>/dev/null; then
  pass "PATH setup: updates .zshrc when it exists"
else
  fail "PATH setup: did not update .zshrc"
fi
rm -rf "$_td"

# Test: Updates fish config.fish when dir exists
_td=$(mktemp -d)
mkdir -p "$_td/.config/fish"
touch "$_td/.config/fish/config.fish"
run_setup_path "$_td" "/fake/bin" > /dev/null
if grep -qF "/fake/bin" "$_td/.config/fish/config.fish" 2>/dev/null; then
  pass "PATH setup: updates fish config.fish when dir exists"
else
  fail "PATH setup: did not update fish config.fish"
fi
rm -rf "$_td"

# Test: Updates ALL profiles when multiple exist (the key bug fix)
_td=$(mktemp -d)
touch "$_td/.bashrc"
touch "$_td/.zshrc"
mkdir -p "$_td/.config/fish"
touch "$_td/.config/fish/config.fish"
run_setup_path "$_td" "/fake/bin" > /dev/null
_all_updated=true
grep -qF "/fake/bin" "$_td/.bashrc" 2>/dev/null || _all_updated=false
grep -qF "/fake/bin" "$_td/.zshrc" 2>/dev/null || _all_updated=false
grep -qF "/fake/bin" "$_td/.config/fish/config.fish" 2>/dev/null || _all_updated=false
if [ "$_all_updated" = "true" ]; then
  pass "PATH setup: updates ALL shell profiles (bash + zsh + fish)"
else
  fail "PATH setup: did not update all profiles" \
    "bash=$(grep -c '/fake/bin' "$_td/.bashrc" 2>/dev/null) zsh=$(grep -c '/fake/bin' "$_td/.zshrc" 2>/dev/null) fish=$(grep -c '/fake/bin' "$_td/.config/fish/config.fish" 2>/dev/null)"
fi
rm -rf "$_td"

# Test: Fish gets fish-specific syntax (set -gx), not export
_td=$(mktemp -d)
mkdir -p "$_td/.config/fish"
touch "$_td/.config/fish/config.fish"
run_setup_path "$_td" "/fake/bin" > /dev/null
if grep -q "set -gx PATH" "$_td/.config/fish/config.fish" 2>/dev/null; then
  pass "PATH setup: fish config uses 'set -gx PATH' syntax"
else
  fail "PATH setup: fish config should use 'set -gx PATH', not 'export'"
fi
rm -rf "$_td"

# Test: Idempotent — running twice doesn't duplicate
_td=$(mktemp -d)
touch "$_td/.bashrc"
touch "$_td/.zshrc"
run_setup_path "$_td" "/fake/bin" > /dev/null
run_setup_path "$_td" "/fake/bin" > /dev/null
_bash_count=$(grep -c '/fake/bin' "$_td/.bashrc" 2>/dev/null || echo 0)
_zsh_count=$(grep -c '/fake/bin' "$_td/.zshrc" 2>/dev/null || echo 0)
if [ "$_bash_count" -eq 1 ] && [ "$_zsh_count" -eq 1 ]; then
  pass "PATH setup: idempotent (no duplicate entries on second run)"
else
  fail "PATH setup: duplicate entries on second run (bash=$_bash_count zsh=$_zsh_count)"
fi
rm -rf "$_td"

# Test: Permission-denied on one profile doesn't block others
_td=$(mktemp -d)
touch "$_td/.zshrc"
chmod 444 "$_td/.zshrc"  # read-only → permission denied
mkdir -p "$_td/.config/fish"
touch "$_td/.config/fish/config.fish"
run_setup_path "$_td" "/fake/bin" > /dev/null
if grep -qF "/fake/bin" "$_td/.config/fish/config.fish" 2>/dev/null; then
  pass "PATH setup: permission-denied on .zshrc still updates fish config"
else
  fail "PATH setup: permission-denied on .zshrc blocked fish config update"
fi
chmod 644 "$_td/.zshrc"  # restore for cleanup
rm -rf "$_td"

# Test: NO_MODIFY_PATH=1 skips all profile updates
_td=$(mktemp -d)
touch "$_td/.bashrc"
touch "$_td/.zshrc"
HOME="$_td" INSTALL_DIR="/fake/bin" NO_MODIFY_PATH=1 \
  sh -c '
  HOME='"'$_td'"'
  INSTALL_DIR="/fake/bin"
  NO_MODIFY_PATH=1
  setup_colors() { RED="" GREEN="" YELLOW="" BLUE="" BOLD="" NC=""; }
  info()  { :; }
  warn()  { :; }
  '"$(sed -n '/^setup_path()/,/^}/p' "$INSTALL_SCRIPT")"'
  setup_colors
  setup_path
' 2>/dev/null
if ! grep -qF "/fake/bin" "$_td/.bashrc" 2>/dev/null && ! grep -qF "/fake/bin" "$_td/.zshrc" 2>/dev/null; then
  pass "PATH setup: NO_MODIFY_PATH=1 skips all profile updates"
else
  fail "PATH setup: NO_MODIFY_PATH=1 did not prevent profile modification"
fi
rm -rf "$_td"

# ── Integration Tests (requires network + GITHUB_TOKEN for private repos) ───

section "Integration Tests"

if [ -z "${GITHUB_TOKEN:-}" ]; then
  # Try to get token from gh CLI
  if command -v gh >/dev/null 2>&1; then
    GITHUB_TOKEN=$(gh auth token 2>/dev/null || true)
  fi
fi

if [ -z "${GITHUB_TOKEN:-}" ]; then
  printf "  \033[1;33mSkipped\033[0m (set GITHUB_TOKEN or install gh CLI for integration tests)\n"
else
  # Test 31: Full install with version pinning
  test_dir=$(mktemp -d)
  trap_cleanup() { rm -rf "$test_dir"; }
  trap trap_cleanup EXIT

  if NO_MODIFY_PATH=1 FLAPJACK_INSTALL="$test_dir" GITHUB_TOKEN="$GITHUB_TOKEN" sh "$INSTALL_SCRIPT" v0.0.7 2>&1 | grep -q "installed successfully"; then
    pass "Full install with version pinning (v0.0.7)"
  else
    fail "Full install with version pinning failed"
  fi

  # Test 32: Binary exists and is executable
  if [ -x "$test_dir/bin/flapjack" ]; then
    pass "Binary is executable at expected path"
  else
    fail "Binary not found or not executable at $test_dir/bin/flapjack"
  fi

  # Test 33: Binary runs (check help)
  if "$test_dir/bin/flapjack" --help >/dev/null 2>&1; then
    pass "Binary runs successfully (--help)"
  else
    fail "Binary failed to run --help (may be incompatible with this platform)"
  fi

  # Test 34: Latest version auto-detection
  test_dir2=$(mktemp -d)
  if NO_MODIFY_PATH=1 FLAPJACK_INSTALL="$test_dir2" GITHUB_TOKEN="$GITHUB_TOKEN" sh "$INSTALL_SCRIPT" 2>&1 | grep -q "installed successfully"; then
    pass "Latest version auto-detection works"
  else
    fail "Latest version auto-detection failed"
  fi
  rm -rf "$test_dir2"

  # Test 35: Idempotent reinstall (run twice, check no errors)
  test_dir3=$(mktemp -d)
  output1=$(NO_MODIFY_PATH=1 FLAPJACK_INSTALL="$test_dir3" GITHUB_TOKEN="$GITHUB_TOKEN" sh "$INSTALL_SCRIPT" v0.0.7 2>&1 || true)
  output2=$(NO_MODIFY_PATH=1 FLAPJACK_INSTALL="$test_dir3" GITHUB_TOKEN="$GITHUB_TOKEN" sh "$INSTALL_SCRIPT" v0.0.7 2>&1 || true)
  if echo "$output2" | grep -q "installed successfully"; then
    pass "Idempotent reinstall works"
  else
    fail "Idempotent reinstall failed" "$output2"
  fi
  rm -rf "$test_dir3"

  # Test 36: Custom install directory
  test_dir4=$(mktemp -d)/custom/path
  if NO_MODIFY_PATH=1 FLAPJACK_INSTALL="$(dirname "$test_dir4")" GITHUB_TOKEN="$GITHUB_TOKEN" sh "$INSTALL_SCRIPT" v0.0.7 2>&1 | grep -q "installed successfully"; then
    pass "Custom install directory (FLAPJACK_INSTALL)"
  else
    fail "Custom install directory failed"
  fi
  rm -rf "$(dirname "$(dirname "$test_dir4")")"

  # Test 37: Invalid version fails gracefully
  test_dir5=$(mktemp -d)
  if NO_MODIFY_PATH=1 FLAPJACK_INSTALL="$test_dir5" GITHUB_TOKEN="$GITHUB_TOKEN" sh "$INSTALL_SCRIPT" v999.999.999 2>&1 | grep -qi "error\|fail\|not found\|404"; then
    pass "Invalid version fails gracefully"
  else
    fail "Invalid version did not produce error"
  fi
  rm -rf "$test_dir5"
fi

# ── Quickstart API Tests (requires network + binary) ─────────────────────────

section "Quickstart API Tests"

if [ -z "${GITHUB_TOKEN:-}" ]; then
  printf "  \033[1;33mSkipped\033[0m (requires GITHUB_TOKEN for binary download)\n"
else
  qs_dir=$(mktemp -d)
  qs_data=$(mktemp -d)
  QS_PORT=7711
  QS_SERVER_PID=""

  qs_cleanup() {
    if [ -n "$QS_SERVER_PID" ] && kill -0 "$QS_SERVER_PID" 2>/dev/null; then
      kill "$QS_SERVER_PID" 2>/dev/null || true
      wait "$QS_SERVER_PID" 2>/dev/null || true
    fi
    rm -rf "$qs_dir" "$qs_data"
  }
  trap qs_cleanup EXIT

  # Install latest version
  qs_install_output=$(NO_MODIFY_PATH=1 FLAPJACK_INSTALL="$qs_dir" GITHUB_TOKEN="$GITHUB_TOKEN" sh "$INSTALL_SCRIPT" 2>&1)
  if echo "$qs_install_output" | grep -q "installed successfully" && [ -x "$qs_dir/bin/flapjack" ]; then
    pass "Quickstart: installed latest binary"
  else
    fail "Quickstart: failed to install binary" "$qs_install_output"
  fi

  # Start server on non-default port with isolated data dir
  if [ -x "$qs_dir/bin/flapjack" ]; then
    FLAPJACK_BIND_ADDR="127.0.0.1:${QS_PORT}" FLAPJACK_DATA_DIR="$qs_data" "$qs_dir/bin/flapjack" >/dev/null 2>&1 &
    QS_SERVER_PID=$!

    # Wait for server readiness
    qs_ready=false
    for _i in $(seq 1 30); do
      if curl -s "http://localhost:${QS_PORT}/indexes" >/dev/null 2>&1; then
        qs_ready=true
        break
      fi
      sleep 0.5
    done

    if [ "$qs_ready" = "true" ]; then
      pass "Quickstart: server started on port ${QS_PORT}"
    else
      fail "Quickstart: server failed to start within 15s"
    fi

    if [ "$qs_ready" = "true" ]; then
      # POST documents (matching README quickstart)
      post_resp=$(curl -s -X POST "http://localhost:${QS_PORT}/indexes/movies/documents" \
        -d '[
          {"objectID":"1","title":"The Matrix","year":1999},
          {"objectID":"2","title":"Inception","year":2010}
        ]' 2>&1)
      if echo "$post_resp" | grep -q "taskID"; then
        pass "Quickstart: POST documents returns taskID"
      else
        fail "Quickstart: POST documents failed" "$post_resp"
      fi

      # Wait for indexing
      sleep 1

      # Search — exact match
      search_resp=$(curl -s "http://localhost:${QS_PORT}/indexes/movies/search?q=matrix" 2>&1)
      if echo "$search_resp" | grep -q '"The Matrix"'; then
        pass "Quickstart: search for 'matrix' returns The Matrix"
      else
        fail "Quickstart: search for 'matrix' did not return expected hit" "$search_resp"
      fi

      # Search — typo tolerance (matching README: "matrx")
      typo_resp=$(curl -s "http://localhost:${QS_PORT}/indexes/movies/search?q=matrx" 2>&1)
      if echo "$typo_resp" | grep -q '"The Matrix"'; then
        pass "Quickstart: typo-tolerant search 'matrx' returns The Matrix"
      else
        fail "Quickstart: typo-tolerant search 'matrx' did not return expected hit" "$typo_resp"
      fi

      # List indexes — verify movies index exists
      idx_resp=$(curl -s "http://localhost:${QS_PORT}/indexes" 2>&1)
      if echo "$idx_resp" | grep -q "movies"; then
        pass "Quickstart: GET /indexes lists 'movies'"
      else
        fail "Quickstart: GET /indexes missing 'movies'" "$idx_resp"
      fi
    fi

    # Stop server
    if [ -n "$QS_SERVER_PID" ] && kill -0 "$QS_SERVER_PID" 2>/dev/null; then
      kill "$QS_SERVER_PID" 2>/dev/null || true
      wait "$QS_SERVER_PID" 2>/dev/null || true
    fi
    QS_SERVER_PID=""
  fi

  rm -rf "$qs_dir" "$qs_data"
fi

# ── Uninstall Tests (requires network + binary) ─────────────────────────────

section "Uninstall Tests"

if [ -z "${GITHUB_TOKEN:-}" ]; then
  printf "  \033[1;33mSkipped\033[0m (requires GITHUB_TOKEN for binary download)\n"
else
  # Install to sandboxed dir with shell configs
  uninst_home=$(mktemp -d)
  uninst_install="${uninst_home}/.flapjack"

  # Create shell config files with existing content
  printf "# existing bashrc content\n" > "$uninst_home/.bashrc"
  printf "# existing zshrc content\n" > "$uninst_home/.zshrc"
  mkdir -p "$uninst_home/.config/fish"
  printf "# existing fish config\n" > "$uninst_home/.config/fish/config.fish"

  # Install flapjack (this adds PATH entries to the shell configs)
  HOME="$uninst_home" FLAPJACK_INSTALL="$uninst_install" GITHUB_TOKEN="$GITHUB_TOKEN" \
    sh "$INSTALL_SCRIPT" 2>&1 >/dev/null

  # Verify binary was installed
  if [ -x "$uninst_install/bin/flapjack" ]; then
    pass "Uninstall: binary installed at $uninst_install/bin/flapjack"
  else
    fail "Uninstall: binary not found (cannot proceed with uninstall tests)"
  fi

  # Verify PATH entries were added to at least one config
  if grep -qF ".flapjack" "$uninst_home/.bashrc" 2>/dev/null || \
     grep -qF ".flapjack" "$uninst_home/.zshrc" 2>/dev/null || \
     grep -qF ".flapjack" "$uninst_home/.config/fish/config.fish" 2>/dev/null; then
    pass "Uninstall: PATH entries present in shell configs before uninstall"
  else
    fail "Uninstall: no PATH entries found in shell configs (install may not have modified them)"
  fi

  if [ -x "$uninst_install/bin/flapjack" ]; then
    # Run uninstall
    uninst_output=$(HOME="$uninst_home" FLAPJACK_INSTALL="$uninst_install" \
      "$uninst_install/bin/flapjack" uninstall 2>&1)

    # Check success message
    if echo "$uninst_output" | grep -q "uninstalled"; then
      pass "Uninstall: command reports success"
    else
      fail "Uninstall: no success message" "$uninst_output"
    fi

    # Verify install directory removed
    if [ ! -d "$uninst_install" ]; then
      pass "Uninstall: install directory removed"
    else
      fail "Uninstall: install directory still exists at $uninst_install"
    fi

    # Verify PATH entries cleaned from bashrc
    if ! grep -qF ".flapjack" "$uninst_home/.bashrc" 2>/dev/null; then
      pass "Uninstall: PATH entry removed from .bashrc"
    else
      fail "Uninstall: .bashrc still contains .flapjack PATH entry"
    fi

    # Verify PATH entries cleaned from zshrc
    if ! grep -qF ".flapjack" "$uninst_home/.zshrc" 2>/dev/null; then
      pass "Uninstall: PATH entry removed from .zshrc"
    else
      fail "Uninstall: .zshrc still contains .flapjack PATH entry"
    fi

    # Verify PATH entries cleaned from fish config
    if ! grep -qF ".flapjack" "$uninst_home/.config/fish/config.fish" 2>/dev/null; then
      pass "Uninstall: PATH entry removed from fish config"
    else
      fail "Uninstall: fish config still contains .flapjack PATH entry"
    fi

    # Verify existing content preserved
    if grep -q "existing bashrc content" "$uninst_home/.bashrc" 2>/dev/null; then
      pass "Uninstall: existing .bashrc content preserved"
    else
      fail "Uninstall: existing .bashrc content was lost"
    fi
  fi

  rm -rf "$uninst_home"
fi

# ── Summary ──────────────────────────────────────────────────────────────────

section "Summary"
printf "  Total: %d  Passed: \033[0;32m%d\033[0m  Failed: \033[0;31m%d\033[0m\n\n" "$TESTS_RUN" "$TESTS_PASSED" "$TESTS_FAILED"

if [ "$TESTS_FAILED" -gt 0 ]; then
  exit 1
fi
