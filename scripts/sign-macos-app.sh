#!/usr/bin/env bash
# Ad-hoc signs a built .app and rebuilds its DMG from the signed bundle.
#
# Tauri's bundler can produce a .app without a valid code signature, and
# Gatekeeper then rejects the DMG with "app is damaged" (missing sealed
# resource list). Ad-hoc signing (identity "-") fixes the broken bundle
# without an Apple Developer certificate or notarization; users still get a
# one-time Gatekeeper approval prompt.
#
# Usage: ./scripts/sign-macos-app.sh <path-to.app> <output.dmg>
set -euo pipefail

app="$1"
out="$2"

[[ -d "$app" ]] || { echo "app not found: $app" >&2; exit 1; }

codesign --force --sign - --timestamp=none "$app"
codesign --verify --deep --strict --verbose=4 "$app"

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT

ditto "$app" "$staging/$(basename "$app")"
hdiutil create -volname "$(basename "$app" .app)" -srcfolder "$staging" \
  -ov -format UDZO "$out"
echo "Signed and packaged: $out"
