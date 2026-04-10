#!/usr/bin/env bash
# Generate the ed25519 keypair for WhisperDesk's Tauri auto-updater.
#
# Run this ONCE before your first release. The public key goes into
# src-tauri/tauri.conf.json (plugins.updater.pubkey) and is safe to commit.
# The private key must be added to GitHub Secrets as TAURI_SIGNING_PRIVATE_KEY.
#
# Usage: bash scripts/generate-updater-keys.sh

set -euo pipefail

KEY_PATH="$HOME/.tauri/whisperdesk.key"

if [ -f "$KEY_PATH" ]; then
  echo "Key already exists at $KEY_PATH"
  echo "Delete it first if you want to regenerate (this will invalidate all existing update signatures)."
  exit 1
fi

mkdir -p "$(dirname "$KEY_PATH")"

echo "Generating WhisperDesk updater signing keypair..."
npx tauri signer generate -w "$KEY_PATH"

echo ""
echo "========================================="
echo "NEXT STEPS"
echo "========================================="
echo ""
echo "1. PUBLIC KEY → commit to repo:"
echo "   Copy the public key printed above into:"
echo "   src-tauri/tauri.conf.json → plugins.updater.pubkey"
echo "   (The public key is NOT a secret — it is safe to commit.)"
echo ""
echo "2. PRIVATE KEY → add to GitHub Secrets:"
echo "   Secret name: TAURI_SIGNING_PRIVATE_KEY"
echo "   Value: contents of $KEY_PATH"
echo "   Command to copy: cat \"$KEY_PATH\" | pbcopy"
echo ""
echo "3. PASSWORD → add to GitHub Secrets:"
echo "   Secret name: TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
echo "   Value: the passphrase you entered above"
echo ""
echo "4. NEVER commit $KEY_PATH to the repository."
echo ""
echo "5. Add Apple signing secrets (macOS notarization):"
echo "   APPLE_CERTIFICATE         — base64-encoded Developer ID .p12"
echo "   APPLE_CERTIFICATE_PASSWORD — password for the .p12"
echo "   APPLE_SIGNING_IDENTITY    — e.g. 'Developer ID Application: Your Name (TEAMID)'"
echo "   APPLE_ID                  — your Apple ID email"
echo "   APPLE_PASSWORD            — app-specific password from appleid.apple.com"
echo "   APPLE_TEAM_ID             — 10-character team ID from developer.apple.com"
echo ""
echo "6. Deploy the Cloudflare Worker updater endpoint:"
echo "   cd workers/updater && npm install && npx wrangler deploy"
echo "   Then point releases.whisperdesk.app DNS to the worker."
