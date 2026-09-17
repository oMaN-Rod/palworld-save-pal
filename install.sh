#!/usr/bin/env bash
# PalStudio desktop — fail-closed Linux/macOS installer.
#
#   curl -fsSL https://palstudio.app/install | bash
#
# Release artifacts are accepted only when their signed checksum manifest
# verifies. Installation is staged and atomically swapped; a failed copy or
# post-install validation restores the previous application.

set -euo pipefail
IFS=$'\n\t'

REPO="${PALSTUDIO_REPO:-oMaN-Rod/palworld-save-pal}"
VERSION="${PALSTUDIO_VERSION:-}"
API_BASE="${PALSTUDIO_API_BASE:-https://api.github.com}"
DL_BASE="${PALSTUDIO_DOWNLOAD_BASE:-https://github.com}"
BIN_DIR_OVERRIDE="${PALSTUDIO_BIN_DIR:-}"
APP_DIR_OVERRIDE="${PALSTUDIO_APP_DIR:-}"

SIGNING_PUBLIC_KEY='-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAe6TtXDrzhlHFk605YUwwC9oKz42CkwFcrta4jVGWdUM=
-----END PUBLIC KEY-----'

log()  { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die()  { printf 'error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required but was not found"; }

validate_repo() {
  [[ "$REPO" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || die 'PALSTUDIO_REPO must be a GitHub owner/name pair'
}

validate_version() {
  [[ "$VERSION" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$ ]] || die 'release tag must look like v1.5.0'
}

is_loopback_base() {
  [[ "$1" = http://127.* || "$1" = 'http://[::1]'* ]]
}

validate_https_base() {
  local name="$1" value="$2"
  [[ "$value" = https://* ]] || is_loopback_base "$value" || die "$name must use HTTPS (plain http is only allowed for loopback test mocks)"
  [[ "$value" != *$'\n'* && "$value" != *$'\r'* ]] || die "$name contains a control character"
}

# Test hook for scripts/test-install-flow.sh: a loopback mock may substitute
# its own signing key so the signed-manifest path can be exercised end to end.
if [[ -n "${PALSTUDIO_SIGNING_PUBLIC_KEY_FILE:-}" ]]; then
  is_loopback_base "$DL_BASE" || die 'PALSTUDIO_SIGNING_PUBLIC_KEY_FILE requires a loopback download base'
  SIGNING_PUBLIC_KEY="$(<"$PALSTUDIO_SIGNING_PUBLIC_KEY_FILE")"
fi

# Every URL this script touches is built from the validated bases above, so
# widening curl to plain http is safe exactly when a base is a loopback mock.
CURL_PROTO=(--proto '=https' --proto-redir '=https')
if is_loopback_base "$API_BASE" || is_loopback_base "$DL_BASE"; then
  CURL_PROTO=(--proto '=http,https' --proto-redir '=http,https')
fi

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'; else shasum -a 256 "$1" | awk '{print $1}'; fi
}

secure_directory() {
  local path="$1" expected_uid="$2" mode owner
  mkdir -p "$path"
  [[ ! -L "$path" ]] || die "refusing to use symlinked directory: $path"
  if stat -c '%u %a' "$path" >/dev/null 2>&1; then IFS=' ' read -r owner mode < <(stat -c '%u %a' "$path"); else owner=$(stat -f '%u' "$path"); mode=$(stat -f '%Lp' "$path"); fi
  [[ "$owner" = "$expected_uid" ]] || die "$path is not owned by uid $expected_uid"
  (( (8#$mode & 022) == 0 )) || die "$path is writable by group or other"
}

rollback=0
backup_path=""
target_path=""
mounted_path=""
restore_previous() {
  set +e
  [[ -z "$mounted_path" ]] || hdiutil detach "$mounted_path" >/dev/null 2>&1
  if (( rollback )); then
    [[ ! -e "$target_path" && ! -L "$target_path" ]] || mv "$target_path" "${target_path}.failed.$$"
    [[ -z "$backup_path" || ! -e "$backup_path" ]] || mv "$backup_path" "$target_path"
  fi
}

main() {
  need curl; need awk; need openssl; need mktemp; need stat; need cp; need mv
  validate_repo; validate_https_base PALSTUDIO_API_BASE "$API_BASE"; validate_https_base PALSTUDIO_DOWNLOAD_BASE "$DL_BASE"
  local kernel machine os arch asset_kind asset checksums_asset base_url tmp download checksums signature public_key sig_http expected actual
  kernel="$(uname -s)"; machine="$(uname -m)"
  case "$kernel" in Linux) os=linux ;; Darwin) os=macos ;; *) die "unsupported OS: $kernel" ;; esac
  case "$machine" in x86_64|amd64) arch=x86_64 ;; aarch64|arm64) arch=aarch64 ;; *) die "unsupported architecture: $machine" ;; esac
  if [[ "$os" = linux ]]; then [[ "$arch" = x86_64 ]] || die 'no aarch64 AppImage is published; use the server installer'; asset_kind=linux.AppImage; else asset_kind=macos.dmg; fi
  if [[ -z "$VERSION" ]]; then
    log "looking up the latest release of $REPO"
    VERSION="$(curl --fail --silent --show-error --location "${CURL_PROTO[@]}" --tlsv1.2 --retry 3 --connect-timeout 10 --max-time 30 -H 'Accept: application/vnd.github+json' "$API_BASE/repos/$REPO/releases/latest" | awk -F'"' '/"tag_name"[[:space:]]*:/ {print $4; exit}')"
  fi
  validate_version
  asset="PalStudio-${VERSION}-${asset_kind}"; checksums_asset="PalStudio-${VERSION}-checksums.txt"; base_url="$DL_BASE/$REPO/releases/download/$VERSION"
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/palstudio-desktop-install.XXXXXX")"; trap 'restore_previous; rm -rf "$tmp"' EXIT
  download="$tmp/$asset"; checksums="$tmp/$checksums_asset"; signature="$checksums.sig"; public_key="$tmp/release-public.pem"
  log "downloading $base_url/$asset"
  curl --fail --silent --show-error --location "${CURL_PROTO[@]}" --tlsv1.2 --retry 3 --connect-timeout 10 --max-time 180 -o "$download" "$base_url/$asset"
  curl --fail --silent --show-error --location "${CURL_PROTO[@]}" --tlsv1.2 --retry 3 --connect-timeout 10 --max-time 30 -o "$checksums" "$base_url/$checksums_asset"
  sig_http="$(curl --silent --show-error --location "${CURL_PROTO[@]}" --tlsv1.2 --retry 3 --connect-timeout 10 --max-time 30 -w '%{http_code}' -o "$signature" "$base_url/$checksums_asset.sig")" || die "could not fetch the signature for $asset"
  [[ "$sig_http" = 200 ]] || die "release $VERSION has no signed checksum manifest (HTTP $sig_http fetching ${checksums_asset}.sig); releases published before signed manifests cannot be installed"
  printf '%s\n' "$SIGNING_PUBLIC_KEY" > "$public_key"; chmod 0644 "$public_key"
  openssl pkeyutl -verify -pubin -inkey "$public_key" -rawin -in "$checksums" -sigfile "$signature" >/dev/null || die 'signed release manifest verification failed'
  expected="$(awk -v f="$asset" '$2 == f {print $1; exit}' "$checksums")"; [[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]] || die "signed manifest has no valid checksum for $asset"
  actual="$(sha256_file "$download")"; [[ "$actual" = "$expected" ]] || die "checksum mismatch for $asset"; log 'signed manifest and checksum verified'

  if [[ "$os" = linux ]]; then
    need chmod; local bin_dir dest staged
    if [[ -n "$BIN_DIR_OVERRIDE" ]]; then bin_dir="$BIN_DIR_OVERRIDE"; elif [[ "$(id -u)" = 0 ]]; then bin_dir=/usr/local/bin; else bin_dir="$HOME/.local/bin"; fi
    secure_directory "$bin_dir" "$(id -u)"
    dest="$bin_dir/PalStudio.AppImage"; staged="$tmp/PalStudio.AppImage.new"
    cp "$download" "$staged"; chmod 0755 "$staged"; [[ -s "$staged" ]] || die 'downloaded AppImage is empty'
    target_path="$dest"; backup_path="${dest}.previous.$$"; [[ ! -e "$backup_path" && ! -L "$backup_path" ]] || die "rollback path already exists: $backup_path"
    if [[ -e "$dest" || -L "$dest" ]]; then [[ ! -L "$dest" ]] || die "existing AppImage is a symlink: $dest"; mv "$dest" "$backup_path"; fi
    rollback=1; mv "$staged" "$dest"; chmod 0755 "$dest"; [[ -x "$dest" && -s "$dest" ]] || die 'installed AppImage validation failed'
    [[ ! -e "$backup_path" ]] || rm -f "$backup_path"; rollback=0
    log "PalStudio $VERSION installed: $dest"
  else
    need hdiutil; need ditto; need codesign; need spctl
    local app_dir mnt app app_count dest staged_app
    app_dir="${APP_DIR_OVERRIDE:-/Applications}"
    if [[ ! -d "$app_dir" ]]; then mkdir -p "$app_dir"; fi
    if [[ ! -w "$app_dir" ]]; then app_dir="$HOME/Applications"; mkdir -p "$app_dir"; fi
    secure_directory "$app_dir" "$(id -u)"
    mnt="$tmp/mnt"; mkdir -p "$mnt"
    hdiutil attach -readonly -nobrowse -mountpoint "$mnt" "$download" >/dev/null
    mounted_path="$mnt"
    app_count="$(find "$mnt" -maxdepth 1 -type d -name '*.app' -print | wc -l | tr -d ' ')"; [[ "$app_count" = 1 ]] || die 'DMG must contain exactly one application bundle'
    app="$(find "$mnt" -maxdepth 1 -type d -name '*.app' -print -quit)"; staged_app="$tmp/PalStudio.app"
    ditto "$app" "$staged_app"
    hdiutil detach "$mnt" >/dev/null
    mounted_path=""
    codesign --verify --deep --strict --verbose=2 "$staged_app" >/dev/null
    spctl --assess --type execute --strict "$staged_app" >/dev/null
    dest="$app_dir/$(basename "$app")"; target_path="$dest"; backup_path="${dest}.previous.$$"; [[ ! -e "$backup_path" && ! -L "$backup_path" ]] || die "rollback path already exists: $backup_path"
    if [[ -e "$dest" || -L "$dest" ]]; then [[ ! -L "$dest" ]] || die "existing application is a symlink: $dest"; mv "$dest" "$backup_path"; fi
    rollback=1; mv "$staged_app" "$dest"; codesign --verify --deep --strict "$dest" >/dev/null; rollback=0
    [[ ! -e "$backup_path" ]] || rm -rf "$backup_path"
    log "PalStudio $VERSION installed: $dest"
  fi
}

main "$@"
