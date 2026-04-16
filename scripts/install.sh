#!/usr/bin/env bash
# install.sh — Download and install the latest WhisperDesk release on macOS
#
# Usage (one-liner):
#   curl -fsSL https://raw.githubusercontent.com/Muminur/m/master/scripts/install.sh | bash
#
# Or clone and run:
#   bash scripts/install.sh

set -euo pipefail

REPO="Muminur/m"
APP_NAME="WhisperDesk"
INSTALL_DIR="/Applications"

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

# ─── Detect architecture ──────────────────────────────────────────────────────
ARCH="$(uname -m)"
if [[ "$ARCH" == "arm64" ]]; then
  DMG_PATTERN="aarch64.dmg"
  ARCH_LABEL="Apple Silicon"
elif [[ "$ARCH" == "x86_64" ]]; then
  DMG_PATTERN="x64.dmg"
  ARCH_LABEL="Intel"
else
  die "Unsupported architecture: $ARCH"
fi

info "Installing ${APP_NAME} for macOS ${ARCH_LABEL} (${ARCH})"

# ─── Fetch latest release ─────────────────────────────────────────────────────
info "Fetching latest release from GitHub..."
RELEASE_JSON="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")"
VERSION="$(echo "$RELEASE_JSON" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": "\(.*\)".*/\1/')"

if [[ -z "$VERSION" ]]; then
  die "Could not determine latest release version."
fi

ok "Latest release: ${VERSION}"

# ─── Find the DMG URL ─────────────────────────────────────────────────────────
DMG_URL="$(echo "$RELEASE_JSON" | grep '"browser_download_url"' | grep "${DMG_PATTERN}" | head -1 | sed 's/.*"browser_download_url": "\(.*\)".*/\1/')"

if [[ -z "$DMG_URL" ]]; then
  # Fallback: try aarch64 if x64 isn't in this release yet
  if [[ "$ARCH" == "x86_64" ]]; then
    warn "No Intel DMG found in ${VERSION}. Trying Apple Silicon build (requires Rosetta 2)..."
    DMG_URL="$(echo "$RELEASE_JSON" | grep '"browser_download_url"' | grep "aarch64.dmg" | head -1 | sed 's/.*"browser_download_url": "\(.*\)".*/\1/')"
  fi
  if [[ -z "$DMG_URL" ]]; then
    die "No macOS DMG found for ${VERSION}. Visit: https://github.com/${REPO}/releases/tag/${VERSION}"
  fi
fi

DMG_FILE="$(basename "$DMG_URL")"
TMP_DIR="$(mktemp -d)"
TMP_DMG="${TMP_DIR}/${DMG_FILE}"

ok "Found: ${DMG_FILE}"

# ─── Download ─────────────────────────────────────────────────────────────────
info "Downloading ${DMG_FILE}..."
curl -fL --progress-bar -o "$TMP_DMG" "$DMG_URL"
ok "Downloaded to ${TMP_DMG}"

# ─── Mount DMG ────────────────────────────────────────────────────────────────
info "Mounting disk image..."
MOUNT_POINT="$(mktemp -d)"
hdiutil attach -quiet -mountpoint "$MOUNT_POINT" "$TMP_DMG"

# ─── Copy to /Applications ────────────────────────────────────────────────────
info "Installing ${APP_NAME}.app to ${INSTALL_DIR}..."

APP_SOURCE="$(find "$MOUNT_POINT" -maxdepth 1 -name "*.app" | head -1)"
if [[ -z "$APP_SOURCE" ]]; then
  hdiutil detach -quiet "$MOUNT_POINT" || true
  die "Could not find .app bundle in DMG."
fi

# Remove existing installation
if [[ -d "${INSTALL_DIR}/${APP_NAME}.app" ]]; then
  warn "Replacing existing ${APP_NAME}.app"
  rm -rf "${INSTALL_DIR:?}/${APP_NAME}.app"
fi

cp -r "$APP_SOURCE" "${INSTALL_DIR}/${APP_NAME}.app"

# ─── Cleanup ──────────────────────────────────────────────────────────────────
hdiutil detach -quiet "$MOUNT_POINT" || true
rm -rf "$TMP_DIR"

# ─── Register with Launch Services ───────────────────────────────────────────
LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -f "$LSREGISTER" ]]; then
  "$LSREGISTER" -f "${INSTALL_DIR}/${APP_NAME}.app" 2>/dev/null || true
fi

# ─── Done ─────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}WhisperDesk ${VERSION} installed successfully!${NC}"
echo -e "  Launch: ${BLUE}open ${INSTALL_DIR}/${APP_NAME}.app${NC}"
echo -e "  Or find it in Launchpad / Spotlight."
echo ""
echo -e "  ${YELLOW}Note:${NC} On first launch macOS may show a security prompt."
echo -e "  Go to System Settings → Privacy & Security → Open Anyway if needed."
