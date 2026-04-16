#!/usr/bin/env bash
set -euo pipefail

# WhisperDesk2 Clean Build & Verify Script
# Uninstalls, rebuilds, and verifies all features work

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PASS=0
FAIL=0
SKIP=0

pass() { echo -e "  ${GREEN}PASS${NC} $1"; ((PASS++)); }
fail() { echo -e "  ${RED}FAIL${NC} $1"; ((FAIL++)); }
skip() { echo -e "  ${YELLOW}SKIP${NC} $1"; ((SKIP++)); }
step() { echo -e "\n${BLUE}==>${NC} $1"; }

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Ensure cargo is in PATH
export PATH="$HOME/.cargo/bin:$PATH"

# ─── Step 1: Clean ───
step "Cleaning previous build artifacts"
rm -rf node_modules dist src-tauri/target/debug/bundle 2>/dev/null || true
echo "  Removed node_modules, dist, bundle artifacts"

# ─── Step 2: Install dependencies ───
step "Installing npm dependencies"
if npm install --loglevel=error 2>&1; then
  pass "npm install"
else
  fail "npm install"
  echo "Cannot continue without dependencies"
  exit 1
fi

# ─── Step 3: TypeScript type check ───
step "TypeScript type check"
if npx tsc --noEmit 2>&1; then
  pass "tsc --noEmit (zero type errors)"
else
  fail "tsc --noEmit"
fi

# ─── Step 4: Vite frontend build ───
step "Vite frontend build"
if npx vite build 2>&1 | tail -5; then
  pass "vite build"
else
  fail "vite build"
fi

# ─── Step 5: Rust cargo check ───
step "Rust cargo check"
if command -v cargo &>/dev/null; then
  cd src-tauri
  if cargo check 2>&1 | tail -5; then
    pass "cargo check (compiles)"
  else
    fail "cargo check"
  fi
  cd "$ROOT"
else
  skip "cargo check (cargo not found)"
fi

# ─── Step 6: Tauri build ───
step "Tauri app build (release)"
if command -v cargo &>/dev/null; then
  if npx tauri build 2>&1 | tail -20; then
    pass "tauri build"
  else
    fail "tauri build"
  fi
else
  skip "tauri build (cargo not found)"
fi

# ─── Step 7: E2E tests ───
step "Playwright E2E tests"
if npx playwright test --reporter=list 2>&1 | tail -35; then
  pass "playwright e2e tests"
else
  fail "playwright e2e tests"
fi

# ─── Step 8: Verify serde fixes in built Rust binary ───
step "Verify serde camelCase annotations"
check_serde() {
  local file="$1" struct="$2"
  if grep -B1 "pub struct $struct" "$file" | grep -q 'rename_all.*camelCase'; then
    pass "serde camelCase on $struct ($file)"
  else
    fail "serde camelCase MISSING on $struct ($file)"
  fi
}
check_serde "src-tauri/src/models/registry.rs" "ModelInfo"
check_serde "src-tauri/src/settings.rs" "WatchFolderConfig"
check_serde "src-tauri/src/settings.rs" "AppSettings"
check_serde "src-tauri/src/transcription/engine.rs" "TranscriptionParams"
check_serde "src-tauri/src/transcription/engine.rs" "SegmentResult"
check_serde "src-tauri/src/transcription/engine.rs" "TranscriptionOutput"

# ─── Step 9: Verify frontend wiring ───
step "Verify frontend feature wiring"

# convertFileSrc in usePlayer
if grep -q 'convertFileSrc' src/hooks/usePlayer.ts; then
  pass "convertFileSrc in usePlayer.ts (audio playback fix)"
else
  fail "convertFileSrc missing in usePlayer.ts"
fi

# No more snake_case key mapping in settingsStore
if grep -q 'default_model_id\|network_policy\|logs_enabled' src/stores/settingsStore.ts; then
  fail "settingsStore still has snake_case key mapping (should be removed)"
else
  pass "settingsStore sends camelCase directly"
fi

# Recording playback UI
if grep -q 'lastRecordingPath' src/components/recording/RecordingPanel.tsx; then
  pass "RecordingPanel has post-recording playback state"
else
  fail "RecordingPanel missing playback state"
fi

# TranscriptDetail action buttons
if grep -q 'ExportDialog' src/components/library/TranscriptDetail.tsx && \
   grep -q 'AiPanel' src/components/library/TranscriptDetail.tsx && \
   grep -q 'starTranscript' src/components/library/TranscriptDetail.tsx; then
  pass "TranscriptDetail has Export, AI, Star, Delete buttons"
else
  fail "TranscriptDetail missing action buttons"
fi

# Batch route
if grep -q 'BatchDashboard' src/App.tsx && grep -q '/batch' src/App.tsx; then
  pass "BatchDashboard route in App.tsx"
else
  fail "BatchDashboard route missing"
fi

# Batch nav link
if grep -q 'Layers' src/components/common/Sidebar.tsx && grep -q '/batch' src/components/common/Sidebar.tsx; then
  pass "Batch nav link in Sidebar"
else
  fail "Batch nav link missing in Sidebar"
fi

# IntegrationWizard in Settings
if grep -q 'IntegrationWizard' src/pages/SettingsPage.tsx; then
  pass "IntegrationWizard wired in SettingsPage"
else
  fail "IntegrationWizard missing from SettingsPage"
fi

# SearchBar in LibraryList
if grep -q 'SearchBar' src/components/library/LibraryList.tsx; then
  pass "SearchBar wired in LibraryList"
else
  fail "SearchBar missing from LibraryList"
fi

# ─── Summary ───
echo ""
echo -e "${BLUE}═══════════════════════════════════════${NC}"
echo -e "  ${GREEN}PASS: $PASS${NC}  ${RED}FAIL: $FAIL${NC}  ${YELLOW}SKIP: $SKIP${NC}"
echo -e "${BLUE}═══════════════════════════════════════${NC}"

if [ "$FAIL" -gt 0 ]; then
  echo -e "\n${RED}Some checks failed. Review output above.${NC}"
  exit 1
else
  echo -e "\n${GREEN}All checks passed!${NC}"
  exit 0
fi
