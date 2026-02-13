#!/bin/sh
# install.sh — Single-command installer for Flapjack search engine.
#
# Usage:
#   curl -fsSL https://install.flapjack.foo | sh            # latest from prod
#   curl -fsSL https://staging.flapjack.foo | sh             # latest from staging
#   curl -fsSL https://install.flapjack.foo | sh -s -- v0.2.0   # pinned version
#
# Environment variables:
#   FLAPJACK_INSTALL   - Install directory (default: ~/.flapjack)
#   FLAPJACK_REPO      - GitHub owner/repo (default: set per distribution)
#   FLAPJACK_VERSION   - Version to install (default: latest)
#   GITHUB_TOKEN       - GitHub token for private repos / rate limits
#   NO_MODIFY_PATH     - Set to 1 to skip PATH modification

set -eu

# ── Configuration ────────────────────────────────────────────────────────────

REPO="${FLAPJACK_REPO:-flapjackhq/flapjack}"
BINARY_NAME="flapjack"
INSTALL_DIR="${FLAPJACK_INSTALL:-$HOME/.flapjack}/bin"

# ── Colors (disabled when piped) ─────────────────────────────────────────────

setup_colors() {
  if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    BOLD='\033[1m'
    NC='\033[0m'
  else
    RED='' GREEN='' YELLOW='' BLUE='' BOLD='' NC=''
  fi
}

info()  { printf "${BLUE}info${NC}  %s\n" "$1"; }
warn()  { printf "${YELLOW}warn${NC}  %s\n" "$1"; }
error() { printf "${RED}error${NC} %s\n" "$1" >&2; }

# ── Platform Detection ───────────────────────────────────────────────────────

detect_platform() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux*)   os="linux" ;;
    Darwin*)  os="darwin" ;;
    MINGW*|MSYS*|CYGWIN*)
      error "Windows is not supported by this installer."
      error "Download the .zip from: https://github.com/${REPO}/releases/latest"
      exit 1
      ;;
    *)
      error "Unsupported operating system: $os"
      exit 1
      ;;
  esac

  case "$arch" in
    x86_64|amd64)   arch="x86_64" ;;
    aarch64|arm64)   arch="aarch64" ;;
    *)
      error "Unsupported architecture: $arch"
      exit 1
      ;;
  esac

  # Detect Rosetta 2 on macOS — if uname reports x86_64 but we're on Apple Silicon,
  # prefer the native ARM64 build
  if [ "$os" = "darwin" ] && [ "$arch" = "x86_64" ]; then
    if sysctl -n sysctl.proc_translated 2>/dev/null | grep -q 1; then
      arch="aarch64"
      info "Detected Rosetta 2 — installing native Apple Silicon build"
    fi
  fi

  # Map to Rust target triples used in our release artifacts
  case "${os}-${arch}" in
    linux-x86_64)   target="x86_64-unknown-linux-musl" ;;
    linux-aarch64)   target="aarch64-unknown-linux-musl" ;;
    darwin-x86_64)   target="x86_64-apple-darwin" ;;
    darwin-aarch64)  target="aarch64-apple-darwin" ;;
    *)
      error "No prebuilt binary for ${os}-${arch}"
      exit 1
      ;;
  esac

  info "Detected platform: ${os}/${arch} → ${target}"
}

# ── Download Tool Detection ──────────────────────────────────────────────────

detect_downloader() {
  if command -v curl > /dev/null 2>&1; then
    downloader="curl"
  elif command -v wget > /dev/null 2>&1; then
    downloader="wget"
  else
    error "Neither curl nor wget found. Please install one and try again."
    exit 1
  fi
}

download() {
  url="$1"
  output="$2"

  auth_header=""
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    auth_header="Authorization: token ${GITHUB_TOKEN}"
  fi

  if [ "$downloader" = "curl" ]; then
    if [ -n "$auth_header" ]; then
      curl -fsSL -H "$auth_header" -o "$output" "$url"
    else
      curl -fsSL -o "$output" "$url"
    fi
  else
    if [ -n "$auth_header" ]; then
      wget -q --header="$auth_header" -O "$output" "$url"
    else
      wget -q -O "$output" "$url"
    fi
  fi
}

# ── Version Resolution ───────────────────────────────────────────────────────

get_version() {
  if [ -n "${FLAPJACK_VERSION:-}" ]; then
    version="$FLAPJACK_VERSION"
    return
  fi

  # Parse version from CLI args (e.g., `| sh -s -- v0.2.0`)
  if [ -n "${1:-}" ]; then
    version="$1"
    return
  fi

  info "Fetching latest release version..."
  api_url="https://api.github.com/repos/${REPO}/releases/latest"

  if [ "$downloader" = "curl" ]; then
    if [ -n "${GITHUB_TOKEN:-}" ]; then
      version=$(curl -fsSL -H "Authorization: token ${GITHUB_TOKEN}" "$api_url" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')
    else
      version=$(curl -fsSL "$api_url" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')
    fi
  else
    if [ -n "${GITHUB_TOKEN:-}" ]; then
      version=$(wget -qO- --header="Authorization: token ${GITHUB_TOKEN}" "$api_url" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')
    else
      version=$(wget -qO- "$api_url" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')
    fi
  fi

  if [ -z "$version" ]; then
    error "Could not determine latest version. Check https://github.com/${REPO}/releases"
    error "You can also specify a version: curl ... | sh -s -- v0.1.0"
    exit 1
  fi

  info "Latest version: ${version}"
}

# ── GitHub API Asset Download (for private repos) ────────────────────────────

# Resolves a release asset's API download URL and downloads it.
# For public repos, falls back to the direct GitHub download URL.
download_release_asset() {
  asset_name="$1"
  output="$2"

  if [ -n "${GITHUB_TOKEN:-}" ]; then
    # Use GitHub API to find the asset ID, then download via API (works for private repos)
    api_url="https://api.github.com/repos/${REPO}/releases/tags/${version}"
    if [ "$downloader" = "curl" ]; then
      asset_url=$(curl -fsSL -H "Authorization: token ${GITHUB_TOKEN}" "$api_url" \
        | grep -B 3 "\"name\": \"${asset_name}\"" \
        | grep '"url"' | head -1 \
        | sed 's/.*"url": *"//;s/".*//')
    else
      asset_url=$(wget -qO- --header="Authorization: token ${GITHUB_TOKEN}" "$api_url" \
        | grep -B 3 "\"name\": \"${asset_name}\"" \
        | grep '"url"' | head -1 \
        | sed 's/.*"url": *"//;s/".*//')
    fi

    if [ -n "$asset_url" ]; then
      # Download via API with octet-stream accept header
      if [ "$downloader" = "curl" ]; then
        curl -fsSL -H "Authorization: token ${GITHUB_TOKEN}" -H "Accept: application/octet-stream" -o "$output" "$asset_url"
      else
        wget -q --header="Authorization: token ${GITHUB_TOKEN}" --header="Accept: application/octet-stream" -O "$output" "$asset_url"
      fi
      return $?
    fi
  fi

  # Fallback: direct URL (works for public repos)
  base_url="https://github.com/${REPO}/releases/download/${version}"
  download "${base_url}/${asset_name}" "$output"
}

# ── Download & Verify ────────────────────────────────────────────────────────

download_and_verify() {
  archive_name="flapjack-${target}.tar.gz"
  checksum_name="${archive_name}.sha256"

  tmpdir=$(mktemp -d)
  trap "rm -rf '$tmpdir'" EXIT

  info "Downloading ${archive_name}..."
  download_release_asset "$archive_name" "${tmpdir}/${archive_name}"

  info "Downloading checksum..."
  if download_release_asset "$checksum_name" "${tmpdir}/${checksum_name}" 2>/dev/null; then
    info "Verifying SHA256 checksum..."
    cd "$tmpdir"
    if command -v shasum > /dev/null 2>&1; then
      shasum -a 256 -c "${checksum_name}" > /dev/null 2>&1
    elif command -v sha256sum > /dev/null 2>&1; then
      sha256sum -c "${checksum_name}" > /dev/null 2>&1
    else
      warn "No checksum tool found — skipping verification"
      cd - > /dev/null
      return
    fi

    if [ $? -eq 0 ]; then
      printf "  ${GREEN}Checksum verified${NC}\n"
    else
      error "Checksum verification FAILED! The download may be corrupted."
      error "Expected checksum from: ${checksum_name}"
      exit 1
    fi
    cd - > /dev/null
  else
    warn "No checksum file available — skipping verification"
  fi
}

# ── Install ──────────────────────────────────────────────────────────────────

install_binary() {
  info "Installing to ${INSTALL_DIR}/${BINARY_NAME}..."

  mkdir -p "$INSTALL_DIR"

  tar xzf "${tmpdir}/${archive_name}" -C "$tmpdir"
  mv "${tmpdir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
  chmod 755 "${INSTALL_DIR}/${BINARY_NAME}"
}

# ── PATH Setup ───────────────────────────────────────────────────────────────

setup_path() {
  if [ "${NO_MODIFY_PATH:-0}" = "1" ]; then
    return
  fi

  # Check if already in PATH
  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*)
      return
      ;;
  esac

  export_line="export PATH=\"${INSTALL_DIR}:\$PATH\""

  profile_updated=false

  # Try ALL detected shell profiles, not just $SHELL.
  # Users often have multiple shells configured (e.g., $SHELL=zsh but terminal=fish).

  # Bash — update first existing file only
  for rc in "$HOME/.bashrc" "$HOME/.bash_profile"; do
    if [ -f "$rc" ]; then
      if ! grep -qF "$INSTALL_DIR" "$rc" 2>/dev/null; then
        if printf '\n# Flapjack\n%s\n' "$export_line" >> "$rc" 2>/dev/null; then
          profile_updated=true
          info "Added to ${rc}"
        else
          warn "Could not write to ${rc} (permission denied)"
        fi
      else
        profile_updated=true
      fi
      break
    fi
  done

  # Zsh
  rc="$HOME/.zshrc"
  if [ -f "$rc" ]; then
    if ! grep -qF "$INSTALL_DIR" "$rc" 2>/dev/null; then
      if printf '\n# Flapjack\n%s\n' "$export_line" >> "$rc" 2>/dev/null; then
        profile_updated=true
        info "Added to ${rc}"
      else
        warn "Could not write to ${rc} (permission denied)"
      fi
    else
      profile_updated=true
    fi
  fi

  # Fish
  fish_conf="${HOME}/.config/fish/config.fish"
  fish_line="set -gx PATH ${INSTALL_DIR} \$PATH"
  if [ -d "$(dirname "$fish_conf")" ]; then
    if ! grep -qF "$INSTALL_DIR" "$fish_conf" 2>/dev/null; then
      if printf '\n# Flapjack\n%s\n' "$fish_line" >> "$fish_conf" 2>/dev/null; then
        profile_updated=true
        info "Added to ${fish_conf}"
      else
        warn "Could not write to ${fish_conf} (permission denied)"
      fi
    else
      profile_updated=true
    fi
  fi

  if [ "$profile_updated" = "false" ]; then
    warn "Could not auto-update PATH. Add this to your shell profile:"
    printf "  %s\n" "$export_line"
  fi
}

# ── Main ─────────────────────────────────────────────────────────────────────

main() {
  setup_colors

  printf "\n"
  printf "  ${BOLD}Flapjack Installer${NC}\n"
  printf "  ${BLUE}https://github.com/${REPO}${NC}\n"
  printf "\n"

  detect_platform
  detect_downloader
  get_version "${1:-}"
  download_and_verify
  install_binary
  setup_path

  printf "\n"
  printf "  ${GREEN}${BOLD}Flapjack ${version} installed successfully!${NC}\n"
  printf "\n"
  printf "  Binary:  ${INSTALL_DIR}/${BINARY_NAME}\n"
  printf "  Run:     ${BOLD}flapjack${NC}\n"

  # Check if we need to remind about PATH
  if ! command -v "$BINARY_NAME" > /dev/null 2>&1; then
    case ":${PATH}:" in
      *":${INSTALL_DIR}:"*)
        ;;
      *)
        printf "\n"
        printf "  ${YELLOW}Restart your terminal or run:${NC}\n"
        # Detect the user's actual terminal shell (parent of this sh process)
        _term_shell=""
        if [ -n "${PPID:-}" ]; then
          _term_shell=$(ps -p "$PPID" -o comm= 2>/dev/null || true)
        fi
        _term_shell="${_term_shell:-$(basename "${SHELL:-sh}")}"
        case "$_term_shell" in
          *fish*)
            printf "    set -gx PATH %s \$PATH\n" "$INSTALL_DIR"
            ;;
          *)
            printf "    export PATH=\"%s:\$PATH\"\n" "$INSTALL_DIR"
            ;;
        esac
        ;;
    esac
  fi

  printf "\n"
}

main "$@"
