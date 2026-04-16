#!/usr/bin/env bash
# reinstall.sh — Uninstall, build, and reinstall WhisperDesk on macOS
# Usage: ./scripts/reinstall.sh [--keep-data] [--skip-build] [--launch]
set -euo pipefail

# ─── Colors ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${BLUE}==>${NC} $1"; }
ok()    { echo -e "  ${GREEN}OK${NC}  $1"; }
warn()  { echo -e "  ${YELLOW}WARN${NC} $1"; }
die()   { echo -e "  ${RED}ERROR${NC} $1"; exit 1; }

# ─── Flags ────────────────────────────────────────────────────────────────────
KEEP_DATA=false
SKIP_BUILD=false
LAUNCH_AFTER=false

for arg in "$@"; do
  case "$arg" in
    --keep-data)   KEEP_DATA=true ;;
    --skip-build)  SKIP_BUILD=true ;;
    --launch)      LAUNCH_AFTER=true ;;
    *) die "Unknown flag: $arg  (valid: --keep-data  --skip-build  --launch)" ;;
  esac
done

# ─── Paths ────────────────────────────────────────────────────────────────────
APP_NAME="WhisperDesk"
APP_BUNDLE="/Applications/${APP_NAME}.app"
BUNDLE_ID="com.whisperdesk.app"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILT_APP="${ROOT}/src-tauri/target/release/bundle/macos/${APP_NAME}.app"

# Ensure cargo is in PATH
export PATH="$HOME/.cargo/bin:$PATH"

# ─── Step 1: Quit ─────────────────────────────────────────────────────────────
info "Quitting ${APP_NAME} (if running)"
if pgrep -f "whisper-desk-app" &>/dev/null; then
  pkill -f "whisper-desk-app" 2>/dev/null || true
  sleep 1
  ok "Quit running process"
else
  ok "App not running"
fi

# ─── Step 2: Uninstall ────────────────────────────────────────────────────────
info "Uninstalling ${APP_NAME}"

if [ -d "$APP_BUNDLE" ]; then
  rm -rf "$APP_BUNDLE"
  ok "Removed $APP_BUNDLE"
else
  warn "$APP_BUNDLE not found — skipping"
fi

if [ "$KEEP_DATA" = true ]; then
  warn "--keep-data: preserving app data, database, and models"
else
  DATA_DIRS=(
    "$HOME/Library/Application Support/${BUNDLE_ID}"
    "$HOME/Library/Caches/${BUNDLE_ID}"
    "$HOME/Library/Saved Application State/${BUNDLE_ID}.savedState"
  )
  PREFS_FILE="$HOME/Library/Preferences/${BUNDLE_ID}.plist"

  for dir in "${DATA_DIRS[@]}"; do
    if [ -d "$dir" ]; then
      rm -rf "$dir"
      ok "Removed $dir"
    fi
  done

  if [ -f "$PREFS_FILE" ]; then
    rm -f "$PREFS_FILE"
    ok "Removed $PREFS_FILE"
  fi
fi

# ─── Step 3: Build ────────────────────────────────────────────────────────────
if [ "$SKIP_BUILD" = true ]; then
  warn "--skip-build: skipping build step"
else
  cd "$ROOT"

  info "Installing npm dependencies"
  npm install --loglevel=error || die "npm install failed"
  ok "npm install"

  info "Building app (this takes a few minutes)"
  # npx tauri build runs 'npm run build' (tsc + vite) then compiles Rust
  if npx tauri build --bundles app 2>&1; then
    ok "tauri build"
  else
    die "tauri build failed — check output above"
  fi
fi

# ─── Step 4: Verify build output ─────────────────────────────────────────────
info "Checking build output"
if [ ! -d "$BUILT_APP" ]; then
  die "Built app not found at: $BUILT_APP\nRun without --skip-build first"
fi
ok "Found $BUILT_APP"

# ─── Step 5: Install ─────────────────────────────────────────────────────────
info "Installing to /Applications/"
cp -r "$BUILT_APP" "$APP_BUNDLE" || die "Failed to copy — try running with sudo or grant Terminal full disk access"
ok "Installed $APP_BUNDLE"

# Register with Launch Services so macOS recognizes the new version
if [ -f "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister" ]; then
  /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
    -f "$APP_BUNDLE" 2>/dev/null || true
fi

# ─── Step 6: Launch (optional) ───────────────────────────────────────────────
if [ "$LAUNCH_AFTER" = true ]; then
  info "Launching ${APP_NAME}"
  open "$APP_BUNDLE"
  ok "Launched"
fi

# ─── Done ─────────────────────────────────────────────────────────────────────
echo ""
echo -e "${GREEN}All done!${NC} ${APP_NAME} reinstalled at ${APP_BUNDLE}"
if [ "$LAUNCH_AFTER" = false ]; then
  echo -e "  Run:  ${BLUE}open /Applications/${APP_NAME}.app${NC}"
fi
