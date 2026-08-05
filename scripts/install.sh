#!/usr/bin/env bash
# install.sh — Download and install the latest WhisperDesk release on macOS
#
# Usage (one-liner):
#   curl -fsSL https://raw.githubusercontent.com/Muminur/m/master/scripts/install.sh | bash
#
# Or clone and run:
#   bash scripts/install.sh

set -euo pipefail
umask 077

REPO="Muminur/m"
APP_NAME="WhisperDesk"
INSTALL_DIR="/Applications"
APP_BUNDLE="${INSTALL_DIR}/${APP_NAME}.app"
BUNDLE_ID="com.whisperdesk.app"

# ─── Colors ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}==>${NC} $1"; }
ok()   { echo -e "  ${GREEN}✓${NC}  $1"; }
warn() { echo -e "  ${YELLOW}!${NC}  $1"; }
die()  { echo -e "  ${RED}✗${NC}  $1" >&2; exit 1; }

# ─── Platform check ───────────────────────────────────────────────────────────
if [[ "$(uname -s)" != "Darwin" ]]; then
  die "This installer is for macOS only. Visit https://github.com/${REPO}/releases for other platforms."
fi

MACOS_VERSION="$(sw_vers -productVersion)"
MACOS_MAJOR="${MACOS_VERSION%%.*}"
if [[ ! "$MACOS_MAJOR" =~ ^[0-9]+$ ]] || (( MACOS_MAJOR < 13 )); then
  die "WhisperDesk requires macOS 13 or newer (found ${MACOS_VERSION})."
fi

# ─── Detect architecture ──────────────────────────────────────────────────────
ARCH="$(uname -m)"
if [[ "$ARCH" == "arm64" ]]; then
  DMG_ARCH="aarch64"
  FALLBACK_ARCH="x64"
  ARCH_LABEL="Apple Silicon"
elif [[ "$ARCH" == "x86_64" ]]; then
  DMG_ARCH="x64"
  FALLBACK_ARCH=""
  ARCH_LABEL="Intel"
else
  die "Unsupported architecture: $ARCH"
fi

info "Installing ${APP_NAME} for macOS ${ARCH_LABEL} (${ARCH})"

# ─── Resolve latest release ───────────────────────────────────────────────────
# Follow GitHub's public redirect instead of its rate-limited API. This keeps
# the one-liner credential-free even when the shared unauthenticated API quota
# has been exhausted.
info "Resolving latest release from GitHub..."
LATEST_RELEASE_URL="$(curl -fsSL -o /dev/null -w '%{url_effective}' \
  --proto '=https' --tlsv1.2 "https://github.com/${REPO}/releases/latest")"

case "$LATEST_RELEASE_URL" in
  "https://github.com/${REPO}/releases/tag/"*) ;;
  *) die "GitHub returned an unexpected latest-release URL." ;;
esac

VERSION="${LATEST_RELEASE_URL##*/}"
if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
  die "Could not determine latest release version."
fi

ok "Latest release: ${VERSION}"

# ─── Find a compatible DMG ────────────────────────────────────────────────────
VERSION_NUMBER="${VERSION#v}"

set_dmg_url() {
  DMG_FILE="${APP_NAME}_${VERSION_NUMBER}_${1}.dmg"
  DMG_URL="https://github.com/${REPO}/releases/download/${VERSION}/${DMG_FILE}"
}

asset_exists() {
  local status
  status="$(curl -LsS -o /dev/null -w '%{http_code}' --range 0-0 \
    --proto '=https' --tlsv1.2 "$1" || true)"
  [[ "$status" == "200" || "$status" == "206" ]]
}

set_dmg_url "$DMG_ARCH"

if asset_exists "$DMG_URL"; then
  :
elif [[ -n "$FALLBACK_ARCH" ]]; then
  warn "No Apple Silicon DMG found in ${VERSION}. Trying the Intel build through Rosetta..."
  set_dmg_url "$FALLBACK_ARCH"
  asset_exists "$DMG_URL" || \
    die "No compatible macOS DMG found for ${VERSION}. Visit: https://github.com/${REPO}/releases/tag/${VERSION}"
else
  die "No compatible macOS DMG found for ${VERSION}. Visit: https://github.com/${REPO}/releases/tag/${VERSION}"
fi

case "$DMG_URL" in
  "https://github.com/${REPO}/releases/download/"*) ;;
  *) die "GitHub returned an unexpected download URL." ;;
esac

if [[ "$ARCH" == "arm64" && "$DMG_URL" == *"x64.dmg" ]]; then
  warn "This release will run under Rosetta. macOS may prompt to install it on first launch."
fi

TMP_DIR="$(mktemp -d)"
TMP_DMG="${TMP_DIR}/${DMG_FILE}"
MOUNT_POINT=""
MOUNTED=false
STAGE_DIR=""
PREVIOUS_APP=""

cleanup() {
  if [[ "$MOUNTED" == true && -n "$MOUNT_POINT" ]]; then
    hdiutil detach -quiet "$MOUNT_POINT" 2>/dev/null || true
  fi
  if [[ -n "$MOUNT_POINT" && -d "$MOUNT_POINT" ]]; then
    rmdir "$MOUNT_POINT" 2>/dev/null || true
  fi
  if [[ -n "$PREVIOUS_APP" && -d "$PREVIOUS_APP" && ! -e "$APP_BUNDLE" ]]; then
    mv "$PREVIOUS_APP" "$APP_BUNDLE" 2>/dev/null || true
  fi
  if [[ -n "$STAGE_DIR" && -d "$STAGE_DIR" ]]; then
    rm -rf -- "$STAGE_DIR"
  fi
  if [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]]; then
    rm -rf -- "$TMP_DIR"
  fi
}
trap cleanup EXIT INT TERM

ok "Found: ${DMG_FILE}"

# ─── Download ─────────────────────────────────────────────────────────────────
info "Downloading ${DMG_FILE}..."
curl -fL --proto '=https' --tlsv1.2 --progress-bar -o "$TMP_DMG" "$DMG_URL"
ok "Downloaded to ${TMP_DMG}"

# ─── Mount DMG ────────────────────────────────────────────────────────────────
info "Mounting disk image..."
MOUNT_POINT="$(mktemp -d)"
hdiutil attach -quiet -nobrowse -mountpoint "$MOUNT_POINT" "$TMP_DMG"
MOUNTED=true

# ─── Copy to /Applications ────────────────────────────────────────────────────
info "Installing ${APP_NAME}.app to ${INSTALL_DIR}..."

APP_SOURCE="${MOUNT_POINT}/${APP_NAME}.app"
if [[ ! -d "$APP_SOURCE" ]]; then
  die "The DMG does not contain the expected ${APP_NAME}.app bundle."
fi

SOURCE_BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "${APP_SOURCE}/Contents/Info.plist" 2>/dev/null || true)"
if [[ "$SOURCE_BUNDLE_ID" != "$BUNDLE_ID" ]]; then
  die "The downloaded app has an unexpected bundle identifier."
fi

# Copy and validate the replacement before touching an existing installation.
STAGE_DIR="$(mktemp -d "${INSTALL_DIR}/.${APP_NAME}.install.XXXXXX")" || \
  die "Could not create a staging directory in ${INSTALL_DIR}."
STAGED_APP="${STAGE_DIR}/${APP_NAME}.app"
if ! ditto "$APP_SOURCE" "$STAGED_APP"; then
  die "Failed to stage ${APP_NAME}.app in ${INSTALL_DIR}."
fi

STAGED_BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "${STAGED_APP}/Contents/Info.plist" 2>/dev/null || true)"
if [[ "$STAGED_BUNDLE_ID" != "$BUNDLE_ID" ]]; then
  die "The staged app failed bundle validation."
fi

# Never replace a symlink or a non-bundle object at the installation path. The
# rollback path is deliberately scoped to a real application directory.
if [[ -L "$APP_BUNDLE" || ( -e "$APP_BUNDLE" && ! -d "$APP_BUNDLE" ) ]]; then
  die "${APP_BUNDLE} exists but is not a regular application bundle directory."
fi

# Swap only after staging succeeds. The trap restores the previous app if the
# final move fails or the installer is interrupted between these operations.
if [[ -e "$APP_BUNDLE" || -L "$APP_BUNDLE" ]]; then
  warn "Replacing existing ${APP_NAME}.app"
  PREVIOUS_APP="${STAGE_DIR}/${APP_NAME}.previous.app"
  if ! mv "$APP_BUNDLE" "$PREVIOUS_APP"; then
    die "Could not move the existing ${APP_NAME}.app out of the way."
  fi
fi

if ! mv "$STAGED_APP" "$APP_BUNDLE"; then
  if [[ -n "$PREVIOUS_APP" && -d "$PREVIOUS_APP" ]]; then
    mv "$PREVIOUS_APP" "$APP_BUNDLE" 2>/dev/null || true
  fi
  die "Failed to install ${APP_NAME}.app."
fi

if [[ -n "$PREVIOUS_APP" && -d "$PREVIOUS_APP" ]]; then
  rm -rf -- "$PREVIOUS_APP"
fi
PREVIOUS_APP=""
rm -rf -- "$STAGE_DIR"
STAGE_DIR=""

# ─── Cleanup ──────────────────────────────────────────────────────────────────
if hdiutil detach -quiet "$MOUNT_POINT"; then
  MOUNTED=false
  rmdir "$MOUNT_POINT" 2>/dev/null || true
  MOUNT_POINT=""
fi
rm -rf -- "$TMP_DIR"
TMP_DIR=""

# ─── Register with Launch Services ───────────────────────────────────────────
LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -f "$LSREGISTER" ]]; then
  "$LSREGISTER" -f "${INSTALL_DIR}/${APP_NAME}.app" 2>/dev/null || true
fi

# ─── Done ─────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}WhisperDesk ${VERSION} installed successfully!${NC}"
echo -e "  Launch: ${BLUE}open ${APP_BUNDLE}${NC}"
echo -e "  Or find it in Launchpad / Spotlight."
echo ""
echo -e "  ${YELLOW}Note:${NC} On first launch macOS may show a security prompt."
echo -e "  Go to System Settings → Privacy & Security → Open Anyway if needed."
