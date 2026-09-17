#!/usr/bin/env bash
# PalStudio server — fail-closed Linux/macOS installer.
#
#   curl -fsSL https://raw.githubusercontent.com/oMaN-Rod/palworld-save-pal/main/scripts/install-server.sh | bash
#
# The script downloads a release bundle, verifies a signed checksum manifest,
# rejects unsafe archives, stages the complete replacement, and only then
# changes the active installation. A failed health check restores the prior
# installation and restarts the prior service when one was running.

set -euo pipefail
IFS=$'\n\t'

REPO="${REPO:-oMaN-Rod/palworld-save-pal}"
VERSION="${VERSION:-}"
BIND_HOST="${PS_HOST:-${HOST:-127.0.0.1}}"
PORT="${PS_PORT:-${PORT:-5174}}"
LISTEN="${PS_LISTEN:-${LISTEN:-}}"
PIN="${PS_PIN:-${PIN:-}}"
PREFIX="${PREFIX:-}"
MODE="${MODE:-}"
NO_SERVICE="${NO_SERVICE:-0}"

# This key is the release publisher's Ed25519 public key. The matching private
# key is kept only in the release CI secret PALSTUDIO_RELEASE_PRIVATE_KEY.
SIGNING_PUBLIC_KEY='-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAHKMHHPKodOXSvmhcn14se0QmS1WY4i/ef0cfoB8NUd4=
-----END PUBLIC KEY-----'

log()  { printf '==> %s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die()  { printf 'error: %s\n' "$*" >&2; exit 1; }

need() {
  command -v "$1" >/dev/null 2>&1 || die "$1 is required but was not found"
}

validate_text() {
  local name="$1" value="$2"
  [[ -n "$value" ]] || die "$name must not be empty"
  [[ "$value" != *$'\n'* && "$value" != *$'\r'* ]] || die "$name contains a control character"
}

validate_repo() {
  [[ "$REPO" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || die "REPO must be a GitHub owner/name pair"
}

validate_version() {
  [[ "$VERSION" =~ ^v?[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$ ]] || die "VERSION must be a release tag such as v1.5.0"
}

validate_port() {
  [[ "$PORT" =~ ^[0-9]+$ ]] || die "PORT must be a number between 1 and 65535"
  (( PORT >= 1 && PORT <= 65535 )) || die "PORT must be a number between 1 and 65535"
}

validate_configuration() {
  validate_text HOST "$BIND_HOST"
  validate_text PREFIX "$PREFIX"
  [[ "$BIND_HOST" != *[[:space:]]* ]] || die "HOST may not contain whitespace"
  [[ "$BIND_HOST" != *[';|&$()`<>']* ]] || die "HOST contains shell metacharacters"
  validate_port
  if [[ -n "$LISTEN" ]]; then
    case "$LISTEN" in
      localhost|lan|tailscale|wan) ;;
      *) die "LISTEN must be localhost, lan, tailscale, or wan" ;;
    esac
  fi
  if [[ -n "$PIN" ]]; then
    (( ${#PIN} >= 4 && ${#PIN} <= 128 )) || die "PIN must contain 4 to 128 characters"
    [[ "$PIN" != *$'\n'* && "$PIN" != *$'\r'* ]] || die "PIN may not contain newlines"
  fi
  case "$MODE" in
    ""|ask|standalone|service) ;;
    *) die "MODE must be ask, standalone, or service" ;;
  esac
  [[ "$PREFIX" = /* ]] || die "PREFIX must be an absolute path"
  [[ "$PREFIX" != / && "$PREFIX" != /usr && "$PREFIX" != /usr/local ]] || die "PREFIX is too broad; choose a dedicated installation directory"
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    need shasum
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

secure_path() {
  local path="$1" expected_owner="$2" mode owner
  [[ -e "$path" ]] || die "required path does not exist: $path"
  [[ ! -L "$path" ]] || die "refusing to use symlinked path: $path"
  if stat -c '%u %a' "$path" >/dev/null 2>&1; then
    read -r owner mode < <(stat -c '%u %a' "$path")
  else
    owner=$(stat -f '%u' "$path")
    mode=$(stat -f '%Lp' "$path")
  fi
  [[ "$owner" = "$expected_owner" ]] || die "$path is not owned by uid $expected_owner"
  (( (8#$mode & 022) == 0 )) || die "$path is writable by group or other"
}

archive_is_safe() {
  local archive="$1" member line first names verbose
  names="$(tar -tzf "$archive")" || die "could not list release archive"
  verbose="$(tar -tvzf "$archive")" || die "could not inspect release archive entries"
  while IFS= read -r member; do
    [[ -n "$member" ]] || die "archive contains an empty member name"
    [[ "$member" != /* ]] || die "archive contains an absolute path: $member"
    case "/$member/" in
      */../*|*/..|../*) die "archive contains a path traversal member: $member" ;;
    esac
    [[ "$member" != *$'\n'* && "$member" != *$'\r'* ]] || die "archive member contains a control character"
  done <<< "$names"
  while IFS= read -r line; do
    first="${line:0:1}"
    case "$first" in
      -|d) ;;
      *) die "archive contains a link or special-file entry, which is not permitted" ;;
    esac
  done <<< "$verbose"
}

extract_archive() {
  local archive="$1" destination="$2" help link
  archive_is_safe "$archive"
  help="$(tar --help 2>&1 || true)"
  if [[ "$help" == *--no-same-owner* && "$help" == *--no-same-permissions* ]]; then
    tar --no-same-owner --no-same-permissions -xzf "$archive" -C "$destination"
  else
    tar -xzf "$archive" -C "$destination"
  fi
  link="$(find "$destination" -type l -print -quit)"
  [[ -z "$link" ]] || die "extracted archive contains a symlink: $link"
  local privileged_entry
  privileged_entry="$(find "$destination" \( -perm -4000 -o -perm -2000 \) -print -quit)"
  [[ -z "$privileged_entry" ]] || die "extracted archive contains a setuid/setgid entry"
}

secure_tree() {
  local root="$1" owner="$2" group="$3"
  if [[ "$owner" = root ]]; then
    chown -R root:root "$root"
    find "$root" -type d -exec chmod 0755 {} +
    find "$root" -type f -exec chmod 0644 {} +
    find "$root" -type f -name '*.db' -exec chmod 0600 {} +
    chmod 0755 "$root/bin/palstudio"
    [[ ! -e "$root/bin/palstudio-desktop" ]] || chmod 0755 "$root/bin/palstudio-desktop"
  else
    chown -R "$owner:$group" "$root"
    find "$root" -type d -exec chmod 0750 {} +
    find "$root" -type f -exec chmod 0640 {} +
    find "$root" -type f -name '*.db' -exec chmod 0600 {} +
    chmod 0750 "$root/bin/palstudio"
    [[ ! -e "$root/bin/palstudio-desktop" ]] || chmod 0750 "$root/bin/palstudio-desktop"
  fi
}

atomic_secret_file() {
  local destination="$1" contents="$2" parent temporary
  parent="$(dirname "$destination")"
  mkdir -p "$parent"
  [[ ! -L "$destination" ]] || die "refusing to replace symlinked secret file: $destination"
  chmod 0700 "$parent"
  temporary="$(mktemp "$parent/.$(basename "$destination").tmp.XXXXXX")"
  if ! ( umask 077; printf '%s\n' "$contents" > "$temporary" ); then
    rm -f -- "$temporary"
    die "could not write secret file: $destination"
  fi
  chmod 0600 "$temporary"
  mv -f "$temporary" "$destination"
}

systemd_quote() {
  local value="$1" escaped
  [[ "$value" != *$'\n'* && "$value" != *$'\r'* && "$value" != *'%'* ]] || die "service definition value contains a control character or '%'"
  escaped="${value//\\/\\\\}"
  escaped="${escaped//\"/\\\"}"
  printf '"%s"' "$escaped"
}

systemd_env_quote() {
  local value="$1" escaped
  [[ "$value" != *$'\n'* && "$value" != *$'\r'* ]] || die 'environment value contains a newline'
  escaped="${value//\\/\\\\}"
  escaped="${escaped//\"/\\\"}"
  printf '"%s"' "$escaped"
}

launchd_xml_escape() {
  local value="$1" amp='&amp;' lt='&lt;' gt='&gt;' quote='&quot;' apostrophe='&apos;'
  [[ "$value" != *$'\n'* && "$value" != *$'\r'* ]] || die 'launchd value contains a newline'
  value="${value//&/$amp}"
  value="${value//</$lt}"
  value="${value//>/$gt}"
  value="${value//\"/$quote}"
  value="${value//\'/$apostrophe}"
  printf '%s' "$value"
}

write_systemd_unit() {
  local unit_path="$1" env_file="$2" data_dir="$3" user_unit="$4" binary="$5" unit_dir temporary exec_args arg
  unit_dir="$(dirname "$unit_path")"
  mkdir -p "$unit_dir"
  exec_args=""
  for arg in "$binary" serve --host "$BIND_HOST" --port "$PORT" --ui-dir "$PREFIX/ui" --data-dir "$PREFIX/data" --db "$data_dir/ps-rs.db"; do
    exec_args+="$(systemd_quote "$arg") "
  done
  temporary="$(mktemp "$unit_dir/.palstudio.service.tmp.XXXXXX")"
  {
    printf '%s\n' '[Unit]' 'Description=PalStudio server' 'After=network-online.target' 'Wants=network-online.target' '' '[Service]'
    printf 'Environment=PALSTUDIO_SERVICE=1\n'
    if [[ "$user_unit" = 1 ]]; then
      printf 'EnvironmentFile=-%s\n' "$(systemd_quote "$env_file")"
    else
      printf '%s\n' 'LoadCredential=network.env:/etc/palstudio/network.env' 'EnvironmentFile=%d/network.env'
    fi
    printf 'ExecStart=%s\n' "${exec_args% }"
    printf 'WorkingDirectory=%s\n' "$(systemd_quote "$PREFIX")"
    printf 'ReadWritePaths=%s\n' "$(systemd_quote "$data_dir")"
    printf '%s\n' 'UMask=0077' 'Restart=on-failure' 'RestartSec=5' 'NoNewPrivileges=true' 'PrivateTmp=true' 'ProtectSystem=strict'
    if [[ "$user_unit" = 1 ]]; then
      printf '%s\n' 'ProtectHome=read-only'
    else
      printf '%s\n' 'User=palstudio' 'Group=palstudio' 'ProtectHome=true'
    fi
    printf '%s\n' 'ProtectKernelTunables=true' 'ProtectKernelModules=true' 'ProtectControlGroups=true' 'PrivateDevices=true' 'RestrictSUIDSGID=true' 'LockPersonality=true' 'RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6' 'CapabilityBoundingSet=' '' '[Install]'
    if [[ "$user_unit" = 1 ]]; then printf '%s\n' 'WantedBy=default.target'; else printf '%s\n' 'WantedBy=multi-user.target'; fi
  } > "$temporary"
  chmod 0644 "$temporary"
  mv -f "$temporary" "$unit_path"
}

validate_service_target() {
  local binary="$1" unit="$2"
  [[ ! -L "$unit" ]] || die "refusing to replace symlinked service definition: $unit"
  [[ -f "$unit" ]] || return 0
  grep -Fq "ExecStart=\"$binary\"" "$unit" || die "existing service definition is not a PalStudio service: $unit"
}

service_was_running=0
service_kind=""
service_unit=""
service_env=""
old_service_unit_backup=""
old_service_env_backup=""
old_service_unit_present=0
old_service_env_present=0
service_definition_changed=0
detect_service() {
  if [[ "$OS" = linux ]] && command -v systemctl >/dev/null 2>&1; then
    if (( AM_ROOT )); then
      service_kind=systemd-system
      service_unit=/etc/systemd/system/palstudio.service
      service_env=/etc/palstudio/network.env
    elif systemctl --user status >/dev/null 2>&1; then
      service_kind=systemd-user
      service_unit="$HOME/.config/systemd/user/palstudio.service"
      service_env="$HOME/.config/palstudio/network.env"
    fi
  elif [[ "$OS" = macos ]] && command -v launchctl >/dev/null 2>&1; then
    service_kind=launchd
    service_unit="$HOME/Library/LaunchAgents/app.palstudio.server.plist"
  fi
}

capture_service_state() {
  detect_service
  [[ -z "$service_kind" ]] && return 0
  if [[ -n "$service_unit" ]]; then
    [[ ! -L "$service_unit" ]] || die "refusing to use symlinked service definition: $service_unit"
    if [[ -e "$service_unit" ]]; then
      [[ -f "$service_unit" ]] || die "service definition is not a regular file: $service_unit"
      old_service_unit_backup="$tmp/old-service-unit"
      cp -p "$service_unit" "$old_service_unit_backup"
      old_service_unit_present=1
    fi
  fi
  if [[ -n "$service_env" ]]; then
    [[ ! -L "$service_env" ]] || die "refusing to use symlinked service credentials: $service_env"
    if [[ -e "$service_env" ]]; then
      [[ -f "$service_env" ]] || die "service credentials are not a regular file: $service_env"
      old_service_env_backup="$tmp/old-service-env"
      cp -p "$service_env" "$old_service_env_backup"
      old_service_env_present=1
    fi
  fi
}

stop_existing_service() {
  detect_service
  [[ -n "$service_kind" ]] || return 0
  local binary="$PREFIX/bin/palstudio"
  case "$service_kind" in
    systemd-system)
      validate_service_target "$binary" "$service_unit"
      if systemctl is-active --quiet palstudio.service; then service_was_running=1; systemctl stop palstudio.service; fi
      ;;
    systemd-user)
      validate_service_target "$binary" "$service_unit"
      if systemctl --user is-active --quiet palstudio.service; then service_was_running=1; systemctl --user stop palstudio.service; fi
      ;;
    launchd)
      if [[ -f "$service_unit" ]]; then
        launchctl print "gui/$(id -u)/app.palstudio.server" >/dev/null 2>&1 && service_was_running=1 || true
        launchctl bootout "gui/$(id -u)" "$service_unit" >/dev/null 2>&1 || true
      fi
      ;;
  esac
}

stop_current_service() {
  case "$service_kind" in
    systemd-system) systemctl stop palstudio.service >/dev/null 2>&1 || true ;;
    systemd-user) systemctl --user stop palstudio.service >/dev/null 2>&1 || true ;;
    launchd) launchctl bootout "gui/$(id -u)" "$service_unit" >/dev/null 2>&1 || true ;;
  esac
}

restore_service_state() {
  (( service_definition_changed )) || return 0
  stop_current_service
  if [[ -n "$service_unit" ]]; then
    rm -f -- "$service_unit"
    if (( old_service_unit_present )); then cp -p "$old_service_unit_backup" "$service_unit"; fi
  fi
  if [[ -n "$service_env" ]]; then
    rm -f -- "$service_env"
    if (( old_service_env_present )); then cp -p "$old_service_env_backup" "$service_env"; fi
  fi
  case "$service_kind" in
    systemd-system) systemctl daemon-reload >/dev/null 2>&1 || warn 'could not reload systemd after rollback' ;;
    systemd-user) systemctl --user daemon-reload >/dev/null 2>&1 || warn 'could not reload the user systemd manager after rollback' ;;
  esac
}

remove_service_definition() {
  case "$service_kind" in
    systemd-system)
      systemctl disable palstudio.service >/dev/null 2>&1 || true
      rm -f -- "$service_unit" "$service_env"
      systemctl daemon-reload >/dev/null 2>&1 || die 'could not reload systemd after removing the service'
      ;;
    systemd-user)
      systemctl --user disable palstudio.service >/dev/null 2>&1 || true
      rm -f -- "$service_unit" "$service_env"
      systemctl --user daemon-reload >/dev/null 2>&1 || die 'could not reload the user systemd manager after removing the service'
      ;;
    launchd)
      rm -f -- "$service_unit"
      ;;
  esac
}

restart_prior_service() {
  (( service_was_running )) || return 0
  case "$service_kind" in
    systemd-system) systemctl start palstudio.service || warn 'could not restart the previous system service' ;;
    systemd-user) systemctl --user start palstudio.service || warn 'could not restart the previous user service' ;;
    launchd) launchctl bootstrap "gui/$(id -u)" "$service_unit" >/dev/null 2>&1 || warn 'could not restart the previous launchd agent' ;;
  esac
}

install_service() {
  local binary="$PREFIX/bin/palstudio" data_dir db_path plist temporary shell_path env_contents service_uid service_gid
  data_dir="$PREFIX"
  db_path="$data_dir/ps-rs.db"
  case "$service_kind" in
    systemd-system)
      if ! id palstudio >/dev/null 2>&1; then useradd --system --user-group --home-dir /var/lib/palstudio --shell /usr/sbin/nologin palstudio; fi
      service_uid="$(id -u palstudio)"
      service_gid="$(getent group palstudio | awk -F: 'NR == 1 {print $3}')"
      [[ "$service_uid" =~ ^[1-9][0-9]*$ ]] || die 'palstudio service user must be a non-root account'
      [[ "$service_gid" =~ ^[1-9][0-9]*$ ]] || die 'palstudio service group must exist and be non-root'
      shell_path="$(getent passwd palstudio | awk -F: '{print $7}')"
      case "$shell_path" in /usr/sbin/nologin|/sbin/nologin) ;; *) usermod --shell /usr/sbin/nologin palstudio ;; esac
      install -d -o palstudio -g palstudio -m 0750 /var/lib/palstudio
      if [[ -f "$PREFIX/ps-rs.db" && ! -e /var/lib/palstudio/ps-rs.db ]]; then install -o palstudio -g palstudio -m 0600 "$PREFIX/ps-rs.db" /var/lib/palstudio/ps-rs.db; fi
      data_dir=/var/lib/palstudio
      db_path="$data_dir/ps-rs.db"
      env_contents=$'PS_NETWORK_ENV=firstboot\nPS_PORT='"$(systemd_env_quote "$PORT")"
      [[ -z "$LISTEN" ]] || env_contents+=$'\nPS_LISTEN='"$(systemd_env_quote "$LISTEN")"
      [[ -z "$PIN" ]] || env_contents+=$'\nPS_PIN='"$(systemd_env_quote "$PIN")"
      atomic_secret_file /etc/palstudio/network.env "$env_contents"
      write_systemd_unit /etc/systemd/system/palstudio.service /etc/palstudio/network.env "$data_dir" 0 "$binary"
      systemctl daemon-reload
      systemctl enable --now palstudio.service
      ;;
    systemd-user)
      env_contents=$'PS_NETWORK_ENV=firstboot\nPS_PORT='"$(systemd_env_quote "$PORT")"
      [[ -z "$LISTEN" ]] || env_contents+=$'\nPS_LISTEN='"$(systemd_env_quote "$LISTEN")"
      [[ -z "$PIN" ]] || env_contents+=$'\nPS_PIN='"$(systemd_env_quote "$PIN")"
      atomic_secret_file "$service_env" "$env_contents"
      write_systemd_unit "$service_unit" "$service_env" "$data_dir" 1 "$binary"
      systemctl --user daemon-reload
      systemctl --user enable --now palstudio.service
      ;;
    launchd)
      plist="$service_unit"
      mkdir -p "$(dirname "$plist")"
      temporary="$(mktemp "$(dirname "$plist")/.app.palstudio.server.plist.tmp.XXXXXX")"
      {
        printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>' '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">' '<plist version="1.0"><dict>' '  <key>Label</key><string>app.palstudio.server</string>' '  <key>ProgramArguments</key><array>'
        for arg in "$binary" serve --host "$BIND_HOST" --port "$PORT" --ui-dir "$PREFIX/ui" --data-dir "$PREFIX/data" --db "$PREFIX/ps-rs.db"; do
          printf '    <string>%s</string>\n' "$(launchd_xml_escape "$arg")"
        done
        printf '%s\n' '  </array>' '  <key>WorkingDirectory</key>'
        printf '  <string>%s</string>\n' "$(launchd_xml_escape "$PREFIX")"
        printf '%s\n' '  <key>EnvironmentVariables</key><dict><key>PALSTUDIO_SERVICE</key><string>1</string>'
        [[ -z "$LISTEN" ]] || printf '    <key>PS_LISTEN</key><string>%s</string>\n' "$(launchd_xml_escape "$LISTEN")"
        printf '    <key>PS_PORT</key><string>%s</string>\n' "$(launchd_xml_escape "$PORT")"
        [[ -z "$PIN" ]] || printf '    <key>PS_PIN</key><string>%s</string>\n' "$(launchd_xml_escape "$PIN")"
        printf '%s\n' '  </dict></dict>' '  <key>RunAtLoad</key><true/>' '  <key>KeepAlive</key><true/>' '  <key>ProcessType</key><string>Background</string>'
        printf '  <key>StandardOutPath</key><string>%s/palstudio.log</string>\n' "$(launchd_xml_escape "$PREFIX")"
        printf '  <key>StandardErrorPath</key><string>%s/palstudio.log</string>\n' "$(launchd_xml_escape "$PREFIX")"
        printf '%s\n' '</dict></plist>'
      } > "$temporary"
      chmod 0600 "$temporary"
      mv -f "$temporary" "$plist"
      launchctl bootstrap "gui/$(id -u)" "$plist"
      ;;
    *) die 'no supported service manager is available; use MODE=standalone' ;;
  esac
  printf '%s\n' "$db_path" >/dev/null
}

health_check() {
  local host="$BIND_HOST" url code
  if [[ "$LISTEN" = tailscale ]]; then
    host="$(trusted_tailscale_ip)" || return 1
  fi
  [[ -n "$host" ]] || return 1
  [[ "$host" = 0.0.0.0 || "$host" = :: ]] && host=127.0.0.1
  if [[ "$host" == *:* ]]; then url="http://[$host]:$PORT/"; else url="http://$host:$PORT/"; fi
  code="$(curl --silent --show-error --connect-timeout 2 --max-time 3 -o /dev/null -w '%{http_code}' "$url" || true)"
  [[ "$code" =~ ^[1-5][0-9][0-9]$ ]]
}

trusted_tailscale_ip() {
  local candidate resolved result
  for candidate in /usr/local/bin/tailscale /opt/homebrew/bin/tailscale /usr/bin/tailscale /bin/tailscale; do
    [[ -x "$candidate" ]] || continue
    resolved="$(realpath "$candidate" 2>/dev/null)" || continue
    secure_path "$resolved" 0 || continue
    result="$("$resolved" ip -4 2>/dev/null | awk 'NF {print $1; exit}')"
    if [[ "$result" =~ ^[0-9]+(\.[0-9]+){3}$ ]]; then
      printf '%s' "$result"
      return 0
    fi
  done
  return 1
}

rollback_required=0
old_prefix=""
old_prefix_moved=0
new_prefix_moved=0
old_link_target=""
old_link_present=0
link_changed=0
rollback_installation() {
  set +e
  if (( rollback_required )); then
    if (( new_prefix_moved )); then
      [[ ! -d "$PREFIX" ]] || mv "$PREFIX" "${PREFIX}.failed.$$"
    fi
    if (( old_prefix_moved )); then
      [[ -z "$old_prefix" || ! -d "$old_prefix" ]] || mv "$old_prefix" "$PREFIX"
    fi
    if (( link_changed )); then
      rm -f -- "$BINDIR/palstudio"
      if (( old_link_present )); then
        ln -s "$old_link_target" "$BINDIR/palstudio"
      fi
    fi
    restore_service_state
    restart_prior_service
  fi
}

main() {
  need curl; need tar; need openssl; need awk; need find; need grep; need mktemp; need stat; need id; need realpath; need readlink
  validate_repo
  local kernel machine os arch platform base_url asset checksums_asset tmp bundle checksums signature public_key expected actual stage staged binary backup ln_target link_tmp
  kernel="$(uname -s)"; machine="$(uname -m)"
  case "$kernel" in Linux) os=linux ;; Darwin) os=macos ;; *) die "unsupported OS: $kernel" ;; esac
  case "$machine" in x86_64|amd64) arch=x86_64 ;; aarch64|arm64) arch=aarch64 ;; *) die "unsupported architecture: $machine" ;; esac
  platform="$os-$arch"
  if [[ -z "$VERSION" ]]; then
    log "looking up the latest release of $REPO"
    VERSION="$(curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --tlsv1.2 --retry 3 --connect-timeout 10 --max-time 30 -H 'Accept: application/vnd.github+json' "https://api.github.com/repos/$REPO/releases/latest" | awk -F'"' '/"tag_name"[[:space:]]*:/ {print $4; exit}')"
  fi
  validate_version
  if [[ -z "$PREFIX" ]]; then if [[ "$(id -u)" = 0 ]]; then PREFIX=/opt/palstudio; else PREFIX="$HOME/.local/share/palstudio"; fi; fi
  validate_configuration
  AM_ROOT=0; [[ "$(id -u)" = 0 ]] && AM_ROOT=1
  OS="$os"
  BINDIR="$([[ "$AM_ROOT" = 1 ]] && printf /usr/local/bin || printf '%s/.local/bin' "$HOME")"
  mkdir -p "$PREFIX" "$BINDIR"
  if (( AM_ROOT )); then
    secure_path "$(dirname "$PREFIX")" 0
    secure_path "$PREFIX" 0
    secure_path "$BINDIR" 0
  else
    secure_path "$(dirname "$PREFIX")" "$(id -u)"
    secure_path "$PREFIX" "$(id -u)"
    secure_path "$BINDIR" "$(id -u)"
  fi

  base_url="https://github.com/$REPO/releases/download/$VERSION"
  asset="palstudio-${VERSION}-server-${platform}.tar.gz"; checksums_asset="palstudio-${VERSION}-server-checksums.txt"
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/palstudio-install.XXXXXX")"
  trap 'rollback_installation; rm -rf "$tmp"' EXIT
  bundle="$tmp/$asset"; checksums="$tmp/$checksums_asset"; signature="$checksums.sig"; public_key="$tmp/release-public.pem"
  log "downloading $base_url/$asset"
  curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --tlsv1.2 --retry 3 --connect-timeout 10 --max-time 180 -o "$bundle" "$base_url/$asset"
  curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --tlsv1.2 --retry 3 --connect-timeout 10 --max-time 30 -o "$checksums" "$base_url/$checksums_asset"
  curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --tlsv1.2 --retry 3 --connect-timeout 10 --max-time 30 -o "$signature" "$base_url/$checksums_asset.sig"
  printf '%s\n' "$SIGNING_PUBLIC_KEY" > "$public_key"; chmod 0644 "$public_key"
  openssl pkeyutl -verify -pubin -inkey "$public_key" -rawin -in "$checksums" -sigfile "$signature" >/dev/null || die 'signed release manifest verification failed'
  expected="$(awk -v f="$asset" '$2 == f { print $1; exit }' "$checksums")"; [[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]] || die "signed manifest has no valid checksum for $asset"
  actual="$(sha256_file "$bundle")"; [[ "$actual" = "$expected" ]] || die "checksum mismatch for $asset"; log 'signed manifest and checksum verified'

  stage="$tmp/stage"; mkdir -p "$stage"; extract_archive "$bundle" "$stage"; staged="$stage/palstudio"
  [[ -d "$staged/bin" && -f "$staged/bin/palstudio" ]] || die 'bundle layout error: expected palstudio/bin/palstudio'
  [[ -d "$staged/ui" && -d "$staged/data/json" ]] || die 'bundle is missing ui/ or data/json'
  if [[ -e "$PREFIX/ps-rs.db" ]]; then [[ -f "$PREFIX/ps-rs.db" && ! -L "$PREFIX/ps-rs.db" ]] || die 'existing database is not a regular file'; cp -p "$PREFIX/ps-rs.db" "$staged/ps-rs.db"; fi
  if (( AM_ROOT )); then secure_tree "$staged" root root; else secure_tree "$staged" "$(id -un)" "$(id -gn)"; fi

  if [[ -z "$MODE" ]]; then if [[ "$NO_SERVICE" = 1 ]]; then MODE=standalone; elif [[ -t 0 && -t 1 ]]; then MODE=ask; else MODE=service; fi; fi
  if [[ "$MODE" = ask ]]; then
    printf '\nHow do you want to run PalStudio?\n  1) standalone\n  2) background service\nChoice [1]: '
    local answer; read -r answer; case "$answer" in 2|s|service) MODE=service ;; *) MODE=standalone ;; esac
  fi
  if [[ "$MODE" = service ]]; then detect_service; [[ -n "$service_kind" ]] || die 'no supported service manager is available; use MODE=standalone'; if [[ "$service_kind" = systemd-system && "$AM_ROOT" != 1 ]]; then die 'system service installation requires root'; fi; if [[ "$service_kind" = systemd-user && "$AM_ROOT" = 1 ]]; then die 'root must use the system service'; fi; fi

  rollback_required=1
  capture_service_state
  stop_existing_service
  old_prefix="${PREFIX}.previous.$$"; [[ ! -e "$old_prefix" && ! -L "$old_prefix" ]] || die "temporary rollback path already exists: $old_prefix"
  if [[ -e "$PREFIX" ]]; then
    mv "$PREFIX" "$old_prefix"
    old_prefix_moved=1
  fi
  mv "$staged" "$PREFIX"
  new_prefix_moved=1
  binary="$PREFIX/bin/palstudio"; ln_target="$BINDIR/palstudio"
  if [[ -e "$ln_target" || -L "$ln_target" ]]; then
    [[ -L "$ln_target" ]] || die "refusing to replace a non-symlink launcher: $ln_target"
    old_link_target="$(readlink "$ln_target")"
    old_link_present=1
  fi
  link_tmp="$BINDIR/.palstudio-link.$$"; ln -s "$binary" "$link_tmp"; mv -f "$link_tmp" "$ln_target"
  link_changed=1
  if [[ "$MODE" = service ]]; then
    service_definition_changed=1
    install_service; log 'service installed and started'; local attempt=0; log 'waiting for the server health check'
    until health_check; do (( attempt += 1 )); (( attempt < 30 )) || die 'server did not pass the health check within 30 seconds'; sleep 1; done
  else
    if (( old_service_unit_present || old_service_env_present )); then
      service_definition_changed=1
      remove_service_definition
    fi
    log 'standalone mode selected; no service is installed'
  fi
  [[ ! -d "$old_prefix" ]] || rm -rf "$old_prefix"
  old_prefix_moved=0
  new_prefix_moved=0
  link_changed=0
  service_definition_changed=0
  rollback_required=0
  log "PalStudio $VERSION installed under $PREFIX"; printf '  launcher: %s\n' "$BINDIR/palstudio"
}

main "$@"
