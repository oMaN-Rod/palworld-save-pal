#!/usr/bin/env bash
# End-to-end test of the palstudio.app/install flow against a local mock
# (scripts/install-mock-server.py): fabricates a release, then runs the real
# one-liners — curl|sh, irm|iex through pwsh — and asserts the results,
# including checksum tampering and update-over-existing-install behavior.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tag="v9.9.9-test"
port="${MOCK_PORT:-8917}"
base="http://127.0.0.1:$port"

work="$(mktemp -d)"
mock_pid=""
unsigned_pid=""
cleanup() {
  [ -n "$unsigned_pid" ] && kill "$unsigned_pid" 2>/dev/null || true
  [ -n "$mock_pid" ] && kill "$mock_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

pass() { printf 'ok   %s\n' "$*"; }
fail() { printf 'FAIL %s\n' "$*" >&2; exit 1; }

# ------------------------------------------------------------ fake release
release_dir="$work/release"
mkdir -p "$release_dir"
printf '#!/bin/sh\necho fake-palstudio-desktop\n' \
  > "$release_dir/PalStudio-$tag-linux.AppImage"
printf 'not-a-real-dmg' > "$release_dir/PalStudio-$tag-macos.dmg"
printf 'not-a-real-msi' > "$release_dir/PalStudio-$tag-windows.msi"
python3 - "$release_dir/PalStudio-$tag-windows-standalone.zip" "$tag" <<'PY'
import sys, zipfile
zip_path, tag = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(zip_path, "w") as z:
    z.writestr(f"PalStudio/bin/palstudio.exe", "fake launcher PE")
    z.writestr(f"PalStudio/bin/palstudio-desktop.exe", "fake desktop PE")
    z.writestr("PalStudio/ui_build/index.html", "<html></html>")
    z.writestr("PalStudio/data/json/game.json", "{}")
PY
(
  cd "$release_dir"
  sha256sum "PalStudio-$tag-linux.AppImage" "PalStudio-$tag-macos.dmg" \
    "PalStudio-$tag-windows.msi" "PalStudio-$tag-windows-standalone.zip" \
    > "PalStudio-$tag-checksums.txt"
)
# A throwaway signing key stands in for the release key pinned in the real
# installers; the runs below inject it through the loopback-only override.
test_signing_key="$work/test-signing-key.pem"
test_signing_pub="$work/test-signing-public.pem"
openssl genpkey -algorithm ed25519 -out "$test_signing_key" 2>/dev/null
openssl pkey -in "$test_signing_key" -pubout -out "$test_signing_pub"
openssl pkeyutl -sign -rawin -inkey "$test_signing_key" \
  -in "$release_dir/PalStudio-$tag-checksums.txt" \
  -out "$release_dir/PalStudio-$tag-checksums.txt.sig"

# ------------------------------------------------------------ mock server
python3 "$repo_root/scripts/install-mock-server.py" \
  --release-dir "$release_dir" --tag "$tag" --port "$port" &
mock_pid=$!
for _ in $(seq 1 50); do
  curl -fsS -o /dev/null "$base/install.sh" 2>/dev/null && break
  sleep 0.1
done
curl -fsS -o /dev/null "$base/install.sh" || fail "mock server did not come up"

# ------------------------------------------------- endpoint User-Agent routing
loc_of() { curl -fsS -o /dev/null -w '%{redirect_url}' -A "$1" "$base/install"; }
[ "$(loc_of 'curl/8.5.0')" = "$base/install.sh" ] || fail "curl UA must get install.sh"
[ "$(loc_of 'Wget/1.21.4 (linux-gnu)')" = "$base/install.sh" ] || fail "wget UA must get install.sh"
[ "$(loc_of 'Mozilla/5.0 (Windows NT; Windows NT 10.0; en-US)WindowsPowerShell/5.1.19041.1682')" = "$base/install.ps1" ] \
  || fail "Windows PowerShell UA must get install.ps1"
[ "$(loc_of 'Mozilla/5.0 (Linux; Ubuntu 24.04) PowerShell/7.4.1')" = "$base/install.ps1" ] \
  || fail "pwsh-on-Linux UA must get install.ps1"
[ "$(loc_of 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/126.0')" = \
  "https://github.com/oMaN-Rod/palworld-save-pal/releases" ] \
  || fail "browser UA must get the releases page"
pass "endpoint serves the right script per User-Agent"

# ------------------------------------------------------------ sh install
sh_env() {
  env PALSTUDIO_API_BASE="$base/api" PALSTUDIO_DOWNLOAD_BASE="$base" \
    PALSTUDIO_SIGNING_PUBLIC_KEY_FILE="$test_signing_pub" "$@"
}
bin_dir="$work/sh-bin"
mkdir -p "$bin_dir"
sh_env PALSTUDIO_BIN_DIR="$bin_dir" \
  sh -c "curl -fsSL $base/install | sh" >"$work/sh-install.log" 2>&1 \
  || { cat "$work/sh-install.log" >&2; fail "curl|sh install failed"; }
grep -q "checksum verified" "$work/sh-install.log" || fail "sh install did not verify the checksum"
appimage="$bin_dir/PalStudio.AppImage"
[ -f "$appimage" ] || fail "AppImage not installed"
[ -x "$appimage" ] || fail "AppImage not executable"
[ "$("$appimage")" = "fake-palstudio-desktop" ] || fail "AppImage content is wrong"
pass "curl|sh installs and verifies the AppImage"

# ------------------------------------------------------------ pwsh install
if command -v pwsh >/dev/null 2>&1; then
  ps_dir="$work/ps-install"
  # pwsh rejects the parenthesized real-world UA header, but any UA carrying
  # the PowerShell token routes the same way.
  run_ps() {
    env PROCESSOR_ARCHITECTURE=AMD64 \
      PALSTUDIO_API_BASE="$base/api" PALSTUDIO_DOWNLOAD_BASE="$base" \
      PALSTUDIO_SIGNING_PUBLIC_KEY_FILE="$test_signing_pub" \
      PALSTUDIO_INSTALL_DIR="$ps_dir" \
      pwsh -NoProfile -Command "Invoke-RestMethod '$base/install' -UserAgent 'WindowsPowerShell/5.1.19041' | Invoke-Expression"
  }
  run_ps >"$work/ps-install.log" 2>&1 \
    || { cat "$work/ps-install.log" >&2; fail "irm|iex install failed"; }
  grep -q "checksum verified" "$work/ps-install.log" || fail "ps1 install did not verify the checksum"
  for f in bin/palstudio.exe bin/palstudio-desktop.exe ui_build/index.html data/json/game.json; do
    [ -f "$ps_dir/$f" ] || fail "missing $f after zip install"
  done
  pass "irm|iex installs and verifies the standalone zip"

  # Re-running over an existing install must preserve the user's database.
  printf 'sqlite-ish' >"$ps_dir/ps-rs.db"
  run_ps >"$work/ps-reinstall.log" 2>&1 \
    || { cat "$work/ps-reinstall.log" >&2; fail "irm|iex reinstall failed"; }
  [ "$(cat "$ps_dir/ps-rs.db")" = "sqlite-ish" ] || fail "reinstall wiped ps-rs.db"
  pass "zip update preserves ps-rs.db"
else
  echo "skip pwsh not found — irm|iex path untested here" >&2
fi

# ------------------------------------------- tampered artifact (checksum layer)
printf '\0tamper' >>"$release_dir/PalStudio-$tag-linux.AppImage"
neg_dir="$work/neg-bin"; mkdir -p "$neg_dir"
if sh_env PALSTUDIO_BIN_DIR="$neg_dir" \
  sh -c "curl -fsSL $base/install | sh" >"$work/sh-neg.log" 2>&1; then
  fail "sh install accepted a tampered artifact"
fi
grep -q "checksum mismatch" "$work/sh-neg.log" || fail "sh negative test lacked the mismatch error"
if command -v pwsh >/dev/null 2>&1; then
  printf '\0tamper' >>"$release_dir/PalStudio-$tag-windows-standalone.zip"
  neg_ps="$work/neg-ps"
  if env PROCESSOR_ARCHITECTURE=AMD64 \
    PALSTUDIO_API_BASE="$base/api" PALSTUDIO_DOWNLOAD_BASE="$base" \
    PALSTUDIO_SIGNING_PUBLIC_KEY_FILE="$test_signing_pub" \
    PALSTUDIO_INSTALL_DIR="$neg_ps" \
    pwsh -NoProfile -Command "Invoke-RestMethod '$base/install' -UserAgent 'WindowsPowerShell/5.1.19041' | Invoke-Expression" \
    >"$work/ps-neg.log" 2>&1; then
    fail "ps1 install accepted a tampered artifact"
  fi
  grep -q "checksum mismatch" "$work/ps-neg.log" || fail "ps1 negative test lacked the mismatch error"
fi
pass "tampered artifacts are rejected on both scripts"

# ------------------------------- tampered manifest (signature layer, stale sig)
sed -i 's/^[0-9a-f]/0000000000000000000000000000000000000000000000000000000000000000/' \
  "$release_dir/PalStudio-$tag-checksums.txt"
stale_dir="$work/stale-bin"; mkdir -p "$stale_dir"
if sh_env PALSTUDIO_BIN_DIR="$stale_dir" \
  sh -c "curl -fsSL $base/install | sh" >"$work/sh-stale.log" 2>&1; then
  fail "sh install accepted a manifest that does not match its signature"
fi
grep -q "signed release manifest verification failed" "$work/sh-stale.log" \
  || fail "stale-signature test lacked the verification error"
pass "tampered manifests fail signature verification"

# ------------------------------------------- unsigned release (missing .sig)
unsigned_dir="$work/release-unsigned"
mkdir -p "$unsigned_dir"
for f in "$release_dir"/*; do
  case "$f" in *.sig) ;; *) cp "$f" "$unsigned_dir/" ;; esac
done
unsigned_port=$((port + 1))
unsigned_base="http://127.0.0.1:$unsigned_port"
python3 "$repo_root/scripts/install-mock-server.py" \
  --release-dir "$unsigned_dir" --tag "$tag" --port "$unsigned_port" &
unsigned_pid=$!
for _ in $(seq 1 50); do
  curl -fsS -o /dev/null "$unsigned_base/install.sh" 2>/dev/null && break
  sleep 0.1
done
curl -fsS -o /dev/null "$unsigned_base/install.sh" || fail "unsigned mock server did not come up"
uns_dir="$work/uns-bin"; mkdir -p "$uns_dir"
if env PALSTUDIO_API_BASE="$unsigned_base/api" PALSTUDIO_DOWNLOAD_BASE="$unsigned_base" \
  PALSTUDIO_SIGNING_PUBLIC_KEY_FILE="$test_signing_pub" \
  PALSTUDIO_BIN_DIR="$uns_dir" \
  sh -c "curl -fsSL $unsigned_base/install.sh | sh" >"$work/sh-uns.log" 2>&1; then
  fail "sh install accepted an unsigned release"
fi
grep -q "has no signed checksum manifest" "$work/sh-uns.log" \
  || fail "unsigned-release test lacked the missing-manifest error"
pass "unsigned releases are rejected with a clear error"

printf '\nall install-flow checks passed\n'
