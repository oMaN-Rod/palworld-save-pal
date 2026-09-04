#!/usr/bin/env bash
#
# dev.sh — one-shot launcher / preflight for Palworld Save Pal (PSP).
# macOS/Linux entry point (the PowerShell sibling is dev.ps1 for Windows).
#
# Does NOT auto-install anything (except the opt-in --install-wasm): on a
# missing or wrong tool it prints the exact command to fix it and exits
# non-zero. Defaults to --web; run `./dev.sh --help` for the full flag
# list.
set -euo pipefail

# Every port/path below is load-bearing in the real config.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UI_DIR="$REPO_ROOT/psp-ui"
PSP_DESKTOP_DIR="$REPO_ROOT/psp-desktop"
PSP_WEB_DIR="$REPO_ROOT/psp-web"
ENV_FILE="$UI_DIR/.env"
NODE_MODULES="$UI_DIR/node_modules"
WASM_OUT="$UI_DIR/src/lib/wasm/psp"

VITE_PORT_DEFAULT=7258   # vite.config.ts server.port, strictPort:true
SERVER_PORT_DEFAULT=7257 # psp-server default + Docker EXPOSE + WS_URL host

if [[ -t 1 ]] && [[ "${EASYRUN_NO_COLOR:-}" != "1" ]] && [[ "${EASYRUN_NO_COLOR:-}" != "true" ]]; then
    RESET=$'\033[0m'; BOLD=$'\033[1m'; DIM=$'\033[2m'
    RED=$'\033[31m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; CYAN=$'\033[36m'
else
    RESET=""; BOLD=""; DIM=""; RED=""; GREEN=""; YELLOW=""; CYAN=""
fi

PREVIOUS_ENV_EXISTS=0
PREVIOUS_ENV_CONTENT=""
RESTORE_ENV=0

log_info()  { printf '%s›%s %s\n' "${CYAN}${BOLD}" "$RESET" "$*" >&2; }
log_ok()    { printf '%s✓%s %s\n' "$GREEN" "$RESET" "$*" >&2; }
log_warn()  { printf '%s⚠%s %s\n' "$YELLOW" "$RESET" "$*" >&2; }
log_fail()  { printf '%s✗%s %s\n' "$RED" "$RESET" "$*" >&2; }
die()       { log_fail "$*"; exit 1; }
banner() {
    local line
    line="$(printf '%.0s─' $(seq 1 52))"
    printf '\n%s%s%s\n  %s%s%s\n%s%s%s\n' "$DIM" "$line" "$RESET" "$BOLD" "$1" "$RESET" "$DIM" "$line" "$RESET" >&2
}

# command -v first, then ~/.cargo/bin / ~/.local/bin / ~/.bun/bin — the
# "just installed, PATH not refreshed" fallback.
resolve_tool() {
    # echoes the path to stdout, returns 1 if not found.
    local tool="$1" d
    if command -v "$tool" >/dev/null 2>&1; then
        command -v "$tool"
        return 0
    fi
    for d in "$HOME/.cargo/bin" "$HOME/.local/bin" "$HOME/.bun/bin"; do
        if [[ -x "$d/$tool" ]]; then
            printf '%s/%s\n' "$d" "$tool"
            return 0
        fi
    done
    return 1
}

# Run a version probe quietly; echoes output, returns its exit code. Never aborts
# (set +e around it).
probe_version() {
    local tool="$1"; shift
    local path
    path="$(resolve_tool "$tool" 2>/dev/null || true)"
    [[ -n "$path" ]] || { echo ""; return 127; }
    "$path" "$@" 2>&1 | head -n1 || true
}

# Each check_* echoes a TSV line: name<TAB>status<TAB>detail<TAB>hint
# status ∈ ok|warn|crit. run_preflight collects them.

check_repo() {
    if [[ -f "$REPO_ROOT/psp-server/Cargo.toml" ]] && [[ -d "$UI_DIR" ]]; then
        return 0
    fi
    printf 'PSP repo\tcrit\tpsp-server/Cargo.toml or psp-ui/ not found at %s\tRun dev.sh from the PSP repo root.\n' "$REPO_ROOT"
}

check_bun() {
    if ! resolve_tool bun >/dev/null 2>&1; then
        printf 'bun\tcrit\tnot found on PATH\tInstall:  curl -fsSL https://bun.sh/install | bash\n'
        return
    fi
    local ver
    ver="$(probe_version bun --version)"
    printf 'bun\tok\t%s\t\n' "${ver:-present}"
}

check_node() {
    if ! resolve_tool node >/dev/null 2>&1; then
        printf 'Node.js\twarn\tnot found (optional; some npm scripts use it)\t\n'
        return
    fi
    local ver
    ver="$(probe_version node --version)"
    printf 'Node.js\tok\t%s\t\n' "${ver:-present}"
}

check_git() {
    if ! resolve_tool git >/dev/null 2>&1; then
        printf 'git\twarn\tnot found (optional for dev)\t\n'
        return
    fi
    local ver
    ver="$(probe_version git --version)"
    printf 'git\tok\t%s\t\n' "${ver:-present}"
}

check_cargo() {
    # $1 = strict (1) or not (0)
    local strict="${1:-0}" status
    if ! resolve_tool cargo >/dev/null 2>&1; then
        status="warn"; [[ "$strict" == "1" ]] && status="crit"
        printf 'Rust (cargo)\t%s\tnot found on PATH\tcurl --proto '\''=https'\'' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n' "$status"
        return
    fi
    local ver
    ver="$(probe_version cargo --version)"
    printf 'Rust (cargo)\tok\t%s\t\n' "${ver:-present}"
}

check_tauri_cli() {
    local strict="${1:-1}" status
    if ! resolve_tool cargo >/dev/null 2>&1; then
        status="warn"; [[ "$strict" == "1" ]] && status="crit"
        printf 'Tauri CLI\t%s\tcargo missing\tcurl --proto '\''=https'\'' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n' "$status"
        return
    fi
    local ver
    ver="$(cargo tauri --version 2>/dev/null | head -n1 || true)"
    if [[ -n "$ver" ]]; then
        printf 'Tauri CLI\tok\t%s\t\n' "$ver"
        return
    fi
    status="warn"; [[ "$strict" == "1" ]] && status="crit"
    printf 'Tauri CLI\t%s\tcargo tauri subcommand not available\tcargo install tauri-cli --version "^2" --locked\n' "$status"
}

check_wasm_pack() {
    local strict="${1:-1}" status
    if ! resolve_tool wasm-pack >/dev/null 2>&1; then
        status="warn"; [[ "$strict" == "1" ]] && status="crit"
        printf 'wasm-pack\t%s\tnot found on PATH\tcargo install wasm-pack && rustup target add wasm32-unknown-unknown\n' "$status"
        return
    fi
    local ver
    ver="$(probe_version wasm-pack --version)"
    printf 'wasm-pack\tok\t%s\t\n' "${ver:-present}"
}

check_wasm_target() {
    local strict="${1:-1}" status
    if ! resolve_tool rustup >/dev/null 2>&1; then
        status="warn"; [[ "$strict" == "1" ]] && status="crit"
        printf 'wasm32-unknown-unknown\t%s\trustup not found\tcurl --proto '\''=https'\'' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n' "$status"
        return
    fi
    if rustup target list --installed 2>/dev/null | grep -q 'wasm32-unknown-unknown'; then
        printf 'wasm32-unknown-unknown\tok\tinstalled\t\n'
        return
    fi
    status="warn"; [[ "$strict" == "1" ]] && status="crit"
    printf 'wasm32-unknown-unknown\t%s\ttarget not installed\trustup target add wasm32-unknown-unknown\n' "$status"
}

check_docker() {
    local strict="${1:-1}" status
    if ! resolve_tool docker >/dev/null 2>&1; then
        status="warn"; [[ "$strict" == "1" ]] && status="crit"
        printf 'Docker\t%s\tnot found on PATH\tInstall Docker Engine — https://docs.docker.com/get-docker/\n' "$status"
        return
    fi
    if docker info >/dev/null 2>&1; then
        printf 'Docker\tok\tdaemon reachable\t\n'
        return
    fi
    status="warn"; [[ "$strict" == "1" ]] && status="crit"
    printf 'Docker\t%s\tCLI present but daemon not reachable\tStart the docker daemon.\n' "$status"
}

check_webkit_linux() {
    # Only meaningful on Linux. macOS/Windows print nothing (caller skips empties).
    if [[ "$(uname -s)" != "Linux" ]]; then return; fi
    local strict="${1:-1}" status
    if ! resolve_tool pkg-config >/dev/null 2>&1; then
        status="warn"; [[ "$strict" == "1" ]] && status="crit"
        printf 'WebKit2GTK 4.1\t%s\tpkg-config not found\tapt/dnf install pkg-config webkit2gtk-4.1\n' "$status"
        return
    fi
    if pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
        printf 'WebKit2GTK 4.1\tok\tfound\t\n'
        return
    fi
    status="warn"; [[ "$strict" == "1" ]] && status="crit"
    printf 'WebKit2GTK 4.1\t%s\tpkg-config can'\''t find webkit2gtk-4.1\tDebian/Ubuntu: apt install libwebkit2gtk-4.1-dev   ·   Fedora: dnf install webkit2gtk4.1-devel\n' "$status"
}

check_appindicator_linux() {
    # Only meaningful on Linux (tray backend; macOS/Windows print nothing).
    if [[ "$(uname -s)" != "Linux" ]]; then return; fi
    local strict="${1:-1}" status
    if ! resolve_tool pkg-config >/dev/null 2>&1; then
        status="warn"; [[ "$strict" == "1" ]] && status="crit"
        printf 'AppIndicator\t%s\tpkg-config not found\tapt/dnf install pkg-config libayatana-appindicator3-dev\n' "$status"
        return
    fi
    # tauri's tray-icon feature links libappindicator at build time; pkg-config
    # accepts either the ayatana or the legacy Ubuntu name.
    if pkg-config --exists ayatana-appindicator3-0.1 2>/dev/null \
        || pkg-config --exists appindicator3-0.1 2>/dev/null; then
        printf 'AppIndicator\tok\tfound\t\n'
        return
    fi
    status="warn"; [[ "$strict" == "1" ]] && status="crit"
    printf 'AppIndicator\t%s\tpkg-config can'\''t find ayatana-appindicator3-0.1\tDebian/Ubuntu: apt install libayatana-appindicator3-dev   ·   Fedora: dnf install libayatana-appindicator3-devel\n' "$status"
}

check_wget() {
    # appimage-strip-graphics.sh fetches appimagetool with wget.
    if resolve_tool wget >/dev/null 2>&1; then
        printf 'wget\tok\t%s\t\n' "$(probe_version wget --version)"
        return
    fi
    printf 'wget\tcrit\tnot found (the AppImage repack step fetches appimagetool)\tapt/dnf install wget\n'
}

disk_free_mb() {
    # df in 1K-blocks; awk to MB. Portable across Linux/macOS.
    df -k "$1" 2>/dev/null | awk 'NR==2 {printf "%d", $4/1024}' || echo 0
}

check_disk_space() {
    local mode="$1"
    local min_mb=800
    case "$mode" in
        web) min_mb=800 ;; desktop) min_mb=2500 ;; build) min_mb=3500 ;;
        webapp) min_mb=1500 ;; landing) min_mb=300 ;; docker) min_mb=2500 ;;
        build-desktop|build-web|build-appimage) min_mb=3500 ;;
    esac
    local free
    free="$(disk_free_mb "$REPO_ROOT")"
    if (( free >= min_mb )); then
        printf 'Disk space\tok\t%s MB free (need %s MB)\t\n' "$free" "$min_mb"
        return
    fi
    printf 'Disk space\tcrit\t%s MB free — %s mode needs ≥%s MB\tFree space on %s\n' "$free" "$mode" "$min_mb" "$REPO_ROOT"
}

check_port() {
    local port="$1"
    if ! command -v nc >/dev/null 2>&1; then
        printf 'Port %s\tok\tchecked at launch (nc unavailable)\t\n' "$port"
        return
    fi
    # If something is already listening, it's in use → warn. We can't safely
    # bind-test in bash without holding the port; nc -z detects a listener.
    if nc -z 127.0.0.1 "$port" 2>/dev/null; then
        printf 'Port %s\twarn\tin use (something is listening)\tFree it, or use --vite-port/--server-port.\n' "$port"
        return
    fi
    printf 'Port %s\tok\tavailable\t\n' "$port"
}

# run_preflight <mode> — prints all check rows to stdout (TSV).
run_preflight() {
    local mode="$1"
    check_bun
    local needs_rust=0 needs_strict_rust=0
    case "$mode" in
        web|desktop|serve|webapp|build|build-desktop|build-appimage|build-web|docker) needs_rust=1 ;;
    esac
    case "$mode" in
        desktop|serve|web|build|build-desktop|build-appimage) needs_strict_rust=1 ;;
    esac
    if (( needs_rust )); then
        check_cargo "$needs_strict_rust"
    fi
    case "$mode" in
        desktop|build-desktop|build-appimage)
            check_tauri_cli 1
            check_webkit_linux 1
            check_appindicator_linux 1
            ;;
    esac
    case "$mode" in
        webapp|build-web)
            check_wasm_pack 1
            check_wasm_target 1
            ;;
    esac
    if [[ "$mode" == "docker" ]]; then check_docker 1; fi
    if [[ "$mode" == "build-appimage" ]]; then check_wget; fi
    check_node
    check_git
    local repo_row
    repo_row="$(check_repo || true)"
    [[ -n "$repo_row" ]] && printf '%s\n' "$repo_row"
    check_disk_space "$mode"
    case "$mode" in
        web) check_port "$VITE_PORT_DEFAULT"; check_port "$SERVER_PORT_DEFAULT" ;;
        # The embedded psp-server binds 7257 in desktop mode too; a busy port
        # makes the launcher exit via its port-busy path.
        desktop) check_port "$SERVER_PORT_DEFAULT" ;;
        serve|docker) check_port "$SERVER_PORT_DEFAULT" ;;
        webapp|landing) check_port "$VITE_PORT_DEFAULT" ;;
    esac
}

# report_preflight <mode> [json] — prints the report card; returns 1 if any crit.
report_preflight() {
    local mode="$1" as_json="${2:-0}"
    local results n_ok=0 n_warn=0 n_crit=0
    results="$(run_preflight "$mode")"
    if [[ "$as_json" == "1" ]]; then
        # Minimal JSON array. Escapes detail/hint minimally (no quotes/backslashes
        # expected in our own output, but we escape " and \ just in case).
        printf '['
        local first=1
        while IFS=$'\t' read -r name status detail hint; do
            [[ -z "$name" ]] && continue
            detail="${detail//\\/\\\\}"; detail="${detail//\"/\\\"}"
            hint="${hint//\\/\\\\}"; hint="${hint//\"/\\\"}"
            [[ $first -eq 1 ]] || printf ','
            printf '{"name":"%s","status":"%s","detail":"%s","hint":"%s"}' \
                "$name" "$status" "$detail" "$hint"
            first=0
        done <<< "$results"
        printf ']\n'
    else
        printf '\n' >&2
        while IFS=$'\t' read -r name status detail hint; do
            [[ -z "$name" ]] && continue
            local color sym
            case "$status" in
                ok) color="$GREEN"; sym="✓"; n_ok=$((n_ok+1)) ;;
                warn) color="$YELLOW"; sym="⚠"; n_warn=$((n_warn+1)) ;;
                crit) color="$RED"; sym="✗"; n_crit=$((n_crit+1)) ;;
            esac
            printf '  %s%s%s %s%-22s%s %s\n' "$color" "$sym" "$RESET" "$BOLD" "$name" "$RESET" "$detail" >&2
            [[ -n "$hint" ]] && printf '      %s→ %s%s\n' "$DIM" "$hint" "$RESET" >&2
        done <<< "$results"
        printf '\n  %s%s ok%s  %s%s warn%s  %s%s critical%s\n' \
            "$BOLD" "$n_ok" "$RESET" "$YELLOW" "$n_warn" "$RESET" "$RED" "$n_crit" "$RESET" >&2
        if (( n_crit > 0 )); then
            printf '  %sFix the %s critical issue(s) above, then re-run dev.sh.%s\n' "$RED" "$n_crit" "$RESET" >&2
            printf '  %s(Tip: dev.sh --check for a standalone report.)%s\n' "$DIM" "$RESET" >&2
        fi
    fi
    (( n_crit > 0 )) && return 1
    return 0
}

# Mirrors psp-ui/scripts/ensure-{desktop,web}-env.mjs — keep both in sync.
snapshot_env() {
    if [[ -f "$ENV_FILE" ]]; then
        PREVIOUS_ENV_EXISTS=1
        PREVIOUS_ENV_CONTENT="$(cat "$ENV_FILE")"
    else
        PREVIOUS_ENV_EXISTS=0
        PREVIOUS_ENV_CONTENT=""
    fi
}

restore_env_on_exit() {
    (( RESTORE_ENV )) || return 0
    if (( PREVIOUS_ENV_EXISTS )); then
        printf '%s' "$PREVIOUS_ENV_CONTENT" > "$ENV_FILE" 2>/dev/null || true
    else
        rm -f "$ENV_FILE" 2>/dev/null || true
    fi
}

write_web_env() {
    # $1 = ws_url (may be empty for the worker/browser-only build)
    mkdir -p "$UI_DIR"
    printf 'PUBLIC_WS_URL=%s\nPUBLIC_DESKTOP_MODE=false\n' "${1:-}" > "$ENV_FILE"
    log_info "Wrote psp-ui/.env (web mode, WS_URL=${1:-<empty>})"
}

write_desktop_env() {
    mkdir -p "$UI_DIR"
    printf 'PUBLIC_WS_URL=127.0.0.1:7257/ws\nPUBLIC_DESKTOP_MODE=true\n' > "$ENV_FILE"
    log_info "Wrote psp-ui/.env (desktop mode)"
}

# Children run in the SCRIPT's own process group (NOT a new session via setsid).
# This is deliberate: when the user hits Ctrl-C, the terminal sends SIGINT to
# the foreground process group, which reaches the script AND every child in the
# same group — including grandchildren vite/cargo spawn — so they die naturally.
# We still install traps to guarantee cleanup even if a child ignores SIGINT or
# the script exits via a non-signal path (set -e failure, etc.).
CHILD_PIDS=()

# spawn_fg_tagged <tag> <cmd...> — foreground, prefixed lines, returns child rc.
# Used for finite builds. Honors SPAWN_CWD (cd there first) + exported env.
spawn_fg_tagged() {
    local tag="$1"; shift
    log_info "Starting $tag: ${BOLD}$*${RESET}"
    local sed_tag="${tag//\//\\/}"
    set +e
    if [[ -n "${SPAWN_CWD:-}" ]]; then
        ( cd "$SPAWN_CWD" && "$@" 2>&1 | sed -u "s/^/[${sed_tag}] /" ) >&2
    else
        ( "$@" 2>&1 | sed -u "s/^/[${sed_tag}] /" ) >&2
    fi
    local rc=${PIPESTATUS[0]}
    set -e
    return $rc
}

# spawn_bg_tagged <tag> <cmd...> — background, prefixed lines, SAME process group.
# Sets LAST_BG_PID to the new background job's PID; do NOT call via $() command
# substitution — that runs this function in a subshell whose exit reaps the
# background job, so the parent shell never sees it. Callers read $LAST_BG_PID.
# Honors SPAWN_CWD; extra env vars must be exported by the caller first.
LAST_BG_PID=""
spawn_bg_tagged() {
    local tag="$1"; shift
    log_info "Starting $tag: ${BOLD}$*${RESET}"
    local sed_tag="${tag//\//\\/}"
    if [[ -n "${SPAWN_CWD:-}" ]]; then
        ( cd "$SPAWN_CWD" && "$@" 2>&1 | sed -u "s/^/[${sed_tag}] /" ) >&2 &
    else
        ( "$@" 2>&1 | sed -u "s/^/[${sed_tag}] /" ) >&2 &
    fi
    LAST_BG_PID=$!
    CHILD_PIDS+=("$LAST_BG_PID")
}

cleanup_children() {
    # Kill the whole process group rooted at THIS script. Because children run
    # in the same process group (no setsid), a negative-PGID kill reaches every
    # child AND grandchild (vite→esbuild, cargo→rustc, psp-server→…). This is
    # more reliable than tracking individual child PIDs, which can be reaped
    # before we resolve their PGID. SIGTERM first (grace), then SIGKILL.
    set +e
    local self_pgid="${EASYRUN_PGID:-}"
    if [[ -z "$self_pgid" ]]; then
        self_pgid="$(ps -o pgid= -p $$ 2>/dev/null | tr -d ' ' || true)"
    fi
    # Group-kill ONLY when this script owns its process group — the interactive
    # case, where the terminal's job control ran dev.sh as its own job. When a
    # non-interactive caller (make, CI, another script) shares our group, a
    # negative-PGID kill would take out the CALLER too; there we fall back to
    # the per-PID kills below, which still cover everything we spawned.
    local parent_pgid
    parent_pgid="$(ps -o pgid= -p "${PPID}" 2>/dev/null | tr -d ' ' || true)"
    if [[ -n "$self_pgid" && "$self_pgid" != "$parent_pgid" ]]; then
        # This shell is a member of the group it is about to signal. SIGTERM is
        # survivable (ignored below for the duration), but a group-wide SIGKILL
        # is not trappable and would kill us before restore_env_on_exit runs,
        # leaving psp-ui/.env pointing at the dev build. So pass 2 enumerates the
        # survivors and skips our own PID instead of killing the group.
        trap '' TERM
        kill -TERM -- -"$self_pgid" 2>/dev/null
        sleep 0.25
        local gpid
        for gpid in $(ps -e -o pid=,pgid= 2>/dev/null \
            | awk -v g="$self_pgid" -v me="$$" '$2==g && $1!=me {print $1}'); do
            kill -KILL "$gpid" 2>/dev/null
        done
        trap - TERM
    fi
    # Belt-and-suspenders: direct kill on each tracked PID too.
    local pid
    for pid in "${CHILD_PIDS[@]}"; do
        kill -TERM "$pid" 2>/dev/null
        kill -KILL "$pid" 2>/dev/null
    done
    set -e
    CHILD_PIDS=()
}

wait_for_http() {
    # $1=url $2=label $3=timeout(sec, default 60)
    local url="$1" label="$2" timeout="${3:-60}"
    log_info "Waiting for $label at $url …"
    local i
    for ((i=0; i<timeout; i++)); do
        if curl -sf -o /dev/null --connect-timeout 2 "$url" 2>/dev/null; then
            log_ok "$label is up: $url"
            return 0
        fi
        sleep 1
    done
    log_warn "$label did not become reachable at $url within ${timeout}s"
    return 1
}

ensure_bun_install() {
    # $1 = force (1) to run bun install even if node_modules exists.
    local force="${1:-0}" bun
    bun="$(resolve_tool bun || true)"
    [[ -n "$bun" ]] || die "bun not found — run ./dev.sh --check first."
    if [[ -d "$NODE_MODULES" ]] && (( ! force )); then
        log_info "psp-ui/node_modules present — skipping bun install."
        return
    fi
    log_info "Running \`bun install\` in psp-ui/ (first run can take a while)…"
    ( cd "$UI_DIR" && "$bun" install ) >&2
    log_ok "bun install complete."
}

ensure_wasm() {
    # $1 = rebuild (1) to force wasm-pack even if the artifact exists.
    #
    # psp_bg.wasm existing is not a safe skip condition: psp.js is tracked and
    # psp_bg.wasm is gitignored, so a checkout/pull restores the throwing stub
    # over the real entry while the stale .wasm survives.
    local rebuild="${1:-0}"
    local wasm_file="$WASM_OUT/psp_bg.wasm"
    local entry_js="$WASM_OUT/psp.js"
    local pkg_json="$WASM_OUT/package.json"
    local stub_marker='psp wasm not built' # text baked into the committed psp.js placeholder

    local reason=""
    if (( rebuild )); then
        reason="--rebuild-wasm"
    elif [[ ! -f "$wasm_file" ]]; then
        reason="psp_bg.wasm missing"
    elif [[ ! -f "$pkg_json" || ! -f "$entry_js" ]]; then
        reason="incomplete wasm package (interrupted build?)"
    elif grep -q "$stub_marker" "$entry_js" 2>/dev/null; then
        reason="psp.js is the committed placeholder (git restored it over the build output)"
    else
        # head -n1 rather than find -quit, for BSD/macOS find.
        local stale=""
        stale="$(find "$REPO_ROOT/psp-web" "$REPO_ROOT/psp-app" "$REPO_ROOT/psp-core" \
            "$REPO_ROOT/psp-db" \
            \( -name '*.rs' -o -name 'Cargo.toml' \) -newer "$wasm_file" -print 2>/dev/null \
            | head -n1 || true)"
        if [[ -z "$stale" && -f "$REPO_ROOT/Cargo.toml" && "$REPO_ROOT/Cargo.toml" -nt "$wasm_file" ]]; then
            stale="$REPO_ROOT/Cargo.toml"
        fi
        if [[ -n "$stale" ]]; then
            reason="newer Rust sources (e.g. ${stale#"$REPO_ROOT"/})"
        fi
    fi

    if [[ -z "$reason" ]]; then
        log_info "WASM up to date (psp-ui/src/lib/wasm/psp/psp_bg.wasm) (--rebuild-wasm to redo)."
        return
    fi

    local cargo wasm_pack
    cargo="$(resolve_tool cargo || true)"; [[ -n "$cargo" ]] || die "cargo not found — run ./dev.sh --check first."
    wasm_pack="$(resolve_tool wasm-pack || true)"; [[ -n "$wasm_pack" ]] || die "wasm-pack not found — run ./dev.sh --install-wasm first."
    log_info "Building psp-web (wasm-pack): $reason"
    # Clear generated output only — an interrupted build must still leave a
    # resolvable $lib/wasm/psp behind, so the tracked placeholders have to
    # survive for wasm-pack to overwrite. Outside a work tree nothing is
    # tracked, so rm -rf is the only option.
    if ( cd "$REPO_ROOT" && git rev-parse --is-inside-work-tree >/dev/null 2>&1 ); then
        ( cd "$REPO_ROOT" && git clean -fdx -- "${WASM_OUT#"$REPO_ROOT"/}" ) >/dev/null 2>&1 || true
    else
        rm -rf "$WASM_OUT"
    fi
    # --out-name psp keeps output aligned with the committed placeholder, the
    # worker import ($lib/wasm/psp), and the .gitignore (psp_bg.wasm).
    ( cd "$PSP_WEB_DIR" && "$wasm_pack" build --target web --out-name psp --out-dir "$WASM_OUT" ) >&2 || \
        die "wasm-pack build failed."
    [[ -f "$wasm_file" ]] || die "wasm-pack reported success but psp_bg.wasm is missing — check output above."
    log_ok "psp-web WASM built."
}

gen_json_manifest() {
    local bun manifest_script="$REPO_ROOT/scripts/gen-json-manifest.mjs"
    bun="$(resolve_tool bun || true)"; [[ -n "$bun" ]] || die "bun not found."
    [[ -f "$manifest_script" ]] || { log_warn "$manifest_script missing — skipping."; return; }
    log_info "Generating JSON manifest (scripts/gen-json-manifest.mjs)…"
    ( cd "$REPO_ROOT" && "$bun" "$manifest_script" ) >&2 || die "JSON manifest generation failed."
}

# The one exception to "never auto-install".
run_install_wasm() {
    banner "Install: WASM toolchain  (wasm32 target + wasm-pack)"
    local rustup
    rustup="$(resolve_tool rustup || true)"
    [[ -n "$rustup" ]] || die "rustup is required to manage the wasm32 target, but it's not on PATH.
    Install it first:
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    Then open a NEW terminal and re-run:  ./dev.sh --install-wasm"

    if "$rustup" target list --installed 2>/dev/null | grep -q 'wasm32-unknown-unknown'; then
        log_ok "wasm32-unknown-unknown target already installed."
    else
        log_info "Installing wasm32-unknown-unknown target (rustup target add)…"
        "$rustup" target add wasm32-unknown-unknown >&2 || die "rustup target add failed."
        "$rustup" target list --installed 2>/dev/null | grep -q 'wasm32-unknown-unknown' \
            || die "rustup reported success but the target isn't listed."
        log_ok "wasm32-unknown-unknown target installed."
    fi

    local cargo
    cargo="$(resolve_tool cargo || true)"; [[ -n "$cargo" ]] || die "cargo not found — rustup toolchain incomplete."
    if resolve_tool wasm-pack >/dev/null 2>&1; then
        log_info "wasm-pack already present ($(probe_version wasm-pack --version))."
        printf '%s  Reinstall/upgrade with: cargo install wasm-pack --force%s\n' "$DIM" "$RESET" >&2
    else
        log_info "Installing wasm-pack (cargo install — first run compiles, ~2-3 min)…"
        "$cargo" install wasm-pack >&2 || die "cargo install wasm-pack failed.
    Common cause: missing C/C++ toolchain (cc/gcc/clang) or libssl dev headers."
        if ! resolve_tool wasm-pack >/dev/null 2>&1; then
            printf '\n%swasm-pack installed but not on PATH.%s\n' "$YELLOW" "$RESET" >&2
            printf '  It'\''s at %s~/.cargo/bin/wasm-pack%s.\n' "$DIM" "$RESET" >&2
            printf '  Open a NEW terminal (so PATH refreshes), then verify:\n    wasm-pack --version\n' >&2
            printf '  Then re-run: %s./dev.sh --check --webapp%s\n' "$BOLD" "$RESET" >&2
            return 0
        fi
        log_ok "wasm-pack installed ($(probe_version wasm-pack --version))."
    fi

    printf '\n' >&2
    banner "Verifying  (--check --webapp)"
    report_preflight webapp 0 >&2 || true
}

run_web() {
    # env already snapshotted by main; write web env for this run.
    local host="${ARG_HOST:-127.0.0.1}"
    local vite_port="${ARG_VITE_PORT:-$VITE_PORT_DEFAULT}"
    local server_port="${ARG_SERVER_PORT:-$SERVER_PORT_DEFAULT}"
    local ws_url="${host}:${server_port}/ws"
    local bun cargo
    bun="$(resolve_tool bun || true)"; [[ -n "$bun" ]] || die "bun not found."
    cargo="$(resolve_tool cargo || true)"; [[ -n "$cargo" ]] || die "cargo not found."

    ensure_bun_install 0
    write_web_env "$ws_url"
    banner "Dev: web  (${host}:${vite_port}  +  psp-server :${server_port})"

    local vite_pid server_pid
    SPAWN_CWD="$UI_DIR" spawn_bg_tagged vite "$bun" run dev:vite -- --host "$host" --port "$vite_port"
    vite_pid="$LAST_BG_PID"
    if [[ "${ARG_NO_SERVER:-0}" != "1" ]]; then
        SPAWN_CWD="$REPO_ROOT" spawn_bg_tagged psp-server "$cargo" run -p psp-server -- \
            --host "$host" --port "$server_port" \
            --ui-dir "$UI_DIR" --data-dir "$REPO_ROOT/data" \
            --db "$REPO_ROOT/psp-rs.db" --dev
        server_pid="$LAST_BG_PID"
    fi
    wait_for_http "http://${host}:${vite_port}" "Vite" 60 || true
    printf '\n%s%s  ▸ PSP web dev running:%s  %shttp://%s:%s%s\n\n' \
        "$GREEN" "$BOLD" "$RESET" "$CYAN" "$host" "$vite_port" "$RESET" >&2
    printf '%s  Ctrl-C to stop. dev.sh restores psp-ui/.env on exit.%s\n\n' "$DIM" "$RESET" >&2
    if [[ "${ARG_NO_SERVER:-0}" != "1" ]]; then
        wait_on_pids "$vite_pid" "$server_pid"
    else
        wait_on_pids "$vite_pid"
    fi
}

run_desktop() {
    local cargo
    cargo="$(resolve_tool cargo || true)"; [[ -n "$cargo" ]] || die "cargo not found."
    if ! cargo tauri --version >/dev/null 2>&1; then
        die "Tauri CLI not available. Install it:
    cargo install tauri-cli --version \"^2\" --locked"
    fi
    ensure_bun_install 0
    write_desktop_env
    # tauri_build::build() validates resource paths exist even in dev; create an
    # empty ui_build/ (gitignored) so the check passes.
    if [[ ! -d "$REPO_ROOT/ui_build" ]]; then
        mkdir -p "$REPO_ROOT/ui_build"
        log_info "Created empty ui_build/ (Tauri dev resource check)."
    fi
    banner "Dev: desktop  (Tauri + embedded psp-server)"
    local tauri_pid
    # cargo tauri dev must run from psp-desktop/.
    SPAWN_CWD="$PSP_DESKTOP_DIR" spawn_bg_tagged tauri "$cargo" tauri dev
    tauri_pid="$LAST_BG_PID"
    printf '%s  Ctrl-C to stop. dev.sh restores psp-ui/.env on exit.%s\n\n' "$DIM" "$RESET" >&2
    wait_on_pids "$tauri_pid"
}

run_webapp() {
    local bun host="${ARG_HOST:-127.0.0.1}" port="${ARG_VITE_PORT:-$VITE_PORT_DEFAULT}"
    bun="$(resolve_tool bun || true)"; [[ -n "$bun" ]] || die "bun not found."
    ensure_bun_install 0
    ensure_wasm "${ARG_REBUILD_WASM:-0}"
    gen_json_manifest
    write_web_env ""
    banner "Dev: webapp  (landing page + tool, browser-only)"
    local vite_pid
    VITE_TRANSPORT=worker SPAWN_CWD="$UI_DIR" spawn_bg_tagged vite "$bun" run dev:vite -- --host "$host" --port "$port"
    vite_pid="$LAST_BG_PID"
    wait_for_http "http://${host}:${port}" "Vite (webapp)" 60 || true
    printf '\n%s%s  ▸ PSP webapp dev running:%s  %shttp://%s:%s%s\n\n' \
        "$GREEN" "$BOLD" "$RESET" "$CYAN" "$host" "$port" "$RESET" >&2
    printf '%s  Landing-page mode (VITE_TRANSPORT=worker). Ctrl-C to stop.%s\n\n' "$DIM" "$RESET" >&2
    wait_on_pids "$vite_pid"
}

run_landing() {
    local bun host="${ARG_HOST:-127.0.0.1}" port="${ARG_VITE_PORT:-$VITE_PORT_DEFAULT}"
    bun="$(resolve_tool bun || true)"; [[ -n "$bun" ]] || die "bun not found."
    ensure_bun_install 0
    write_web_env ""
    banner "Dev: landing-only  (${host}:${port}, no wasm / no server)"
    local vite_pid
    VITE_TRANSPORT=worker VITE_LANDING_ONLY=true SPAWN_CWD="$UI_DIR" spawn_bg_tagged vite "$bun" run dev:vite -- --host "$host" --port "$port"
    vite_pid="$LAST_BG_PID"
    wait_for_http "http://${host}:${port}" "Vite (landing)" 60 || true
    printf '\n%s%s  ▸ PSP landing preview:%s  %shttp://%s:%s%s\n' \
        "$GREEN" "$BOLD" "$RESET" "$CYAN" "$host" "$port" "$RESET" >&2
    printf '%s  Landing page only — WASM/server skipped (VITE_LANDING_ONLY).%s\n' "$DIM" "$RESET" >&2
    printf '%s  Buttons that load a save won'\''t work. Ctrl-C to stop.%s\n\n' "$DIM" "$RESET" >&2
    wait_on_pids "$vite_pid"
}

run_serve() {
    local cargo host="${ARG_HOST:-0.0.0.0}" port="${ARG_SERVER_PORT:-$SERVER_PORT_DEFAULT}"
    cargo="$(resolve_tool cargo || true)"; [[ -n "$cargo" ]] || die "cargo not found."
    banner "Serve: psp-server  (${host}:${port})"
    local server_pid
    SPAWN_CWD="$REPO_ROOT" spawn_bg_tagged psp-server "$cargo" run -p psp-server -- \
        --host "$host" --port "$port" \
        --ui-dir "$UI_DIR" --data-dir "$REPO_ROOT/data" \
        --db "$REPO_ROOT/psp-rs.db" --dev
    server_pid="$LAST_BG_PID"
    wait_on_pids "$server_pid"
}

run_docker() {
    local docker host ws_url
    docker="$(resolve_tool docker || true)"; [[ -n "$docker" ]] || die "docker not found."
    [[ -f "$REPO_ROOT/docker-compose.yml" ]] || die "docker-compose.yml not found at repo root."
    host="${ARG_HOST:-$(detect_lan_ip)}"
    host="${host:-127.0.0.1}"
    ws_url="${host}:${SERVER_PORT_DEFAULT}/ws"
    banner "Docker: build + up  (PUBLIC_WS_URL=${ws_url}, port ${SERVER_PORT_DEFAULT})"
    log_info "Building image (first build is slow; bakes WS_URL into the SPA)…"
    spawn_fg_tagged docker-build "$docker" compose build --build-arg "PUBLIC_WS_URL=${ws_url}" \
        || die "docker compose build failed."
    spawn_fg_tagged docker-up "$docker" compose up -d \
        || die "docker compose up failed."
    log_ok "Docker backend up — connect at http://${host}:${SERVER_PORT_DEFAULT}"
    printf '%s  Logs: docker compose logs -f   ·   Stop: docker compose down%s\n' "$DIM" "$RESET" >&2
}

run_build_desktop() {
    local cargo script
    cargo="$(resolve_tool cargo || true)"; [[ -n "$cargo" ]] || die "cargo not found."
    ensure_bun_install 1
    write_desktop_env
    banner "Build: desktop (cargo tauri build)"
    script="$REPO_ROOT/scripts/build-desktop.sh"
    if [[ -f "$script" ]]; then
        log_info "Using platform script scripts/build-desktop.sh"
        spawn_fg_tagged build-desktop bash "$script" || die "desktop build failed."
    else
        spawn_fg_tagged build-desktop "$cargo" tauri build || die "desktop build failed."
    fi
    log_ok "Desktop build complete."
}

run_build_appimage() {
    [[ "$(uname -s)" == "Linux" ]] \
        || die "--build-appimage is Linux-only (tauri's appimage bundler targets the host OS; from Windows use WSL + dev.sh)."
    local script="$REPO_ROOT/scripts/build-appimage.sh"
    [[ -f "$script" ]] || die "scripts/build-appimage.sh not found."
    ensure_bun_install 1
    write_desktop_env
    banner "Build: AppImage (tauri appimage bundle + host-graphics strip)"
    # Same pipeline as the release CI: bundle, strip bundled graphics libs, copy
    # to dist/ — so local and shipped AppImages behave identically.
    spawn_fg_tagged build-appimage bash "$script" || die "AppImage build failed."
    log_ok "AppImage build complete → dist/PalworldSavePal-<version>-linux.AppImage"
}

run_build_web() {
    local bun
    bun="$(resolve_tool bun || true)"; [[ -n "$bun" ]] || die "bun not found."
    ensure_bun_install 1
    ensure_wasm "${ARG_REBUILD_WASM:-0}"
    gen_json_manifest
    write_web_env ""
    banner "Build: web (landing-page bundle → ui_build/)"
    VITE_TRANSPORT=worker SPAWN_CWD="$UI_DIR" spawn_fg_tagged build "$bun" run build \
        || die "web build failed."
    log_ok "Web build complete → ui_build/"
}

run_build_plain() {
    local bun
    bun="$(resolve_tool bun || true)"; [[ -n "$bun" ]] || die "bun not found."
    ensure_bun_install 1
    write_web_env "127.0.0.1:${SERVER_PORT_DEFAULT}/ws"
    banner "Build: plain SPA (server-served → ui_build/)"
    SPAWN_CWD="$UI_DIR" spawn_fg_tagged build "$bun" run build || die "build failed."
    log_ok "Plain SPA build complete → ui_build/"
}

detect_lan_ip() {
    # mirrors build-docker.sh: macOS ipconfig, Linux hostname -I.
    local ip=""
    if [[ "$(uname -s)" == "Darwin" ]]; then
        ip="$(ipconfig getifaddr en0 2>/dev/null || true)"
    else
        ip="$(hostname -I 2>/dev/null | awk '{print $1}')"
    fi
    [[ -n "$ip" && "$ip" != 127.* ]] && printf '%s' "$ip" || true
}

# wait_on_pids <pid...> — block until any child exits or a signal arrives.
# `wait` with no args blocks until ALL background jobs finish AND returns
# immediately when a trapped signal (INT/TERM) is received, so the trap handler
# runs. This is more reliable than a poll/sleep loop, which can swallow signals.
# We `wait -n` (bash 4.3+) to return as soon as the FIRST child exits.
wait_on_pids() {
    local pids=("$@") rc=0
    if (( ${#pids[@]} == 0 )); then return 0; fi
    if wait -n 2>/dev/null; then
        rc=0
    else
        rc=$?
    fi

    # If we get here via a normal child exit (not a signal), report it. Signal
    # paths are handled by the trap + _on_interrupt before we ever reach here.
    log_warn "process exited (code $rc)."
    return $rc
}

usage() {
    cat <<'EOF' >&2
dev.sh — Palworld Save Pal dev/launch/build helper (macOS/Linux).
Runs from source; does NOT auto-install tools (run --check for a report card).

mode (pick one; defaults to --web):
  --web              Dev: Vite + psp-server (tool-only SPA).
  --desktop          Dev: Tauri native window + embedded server (Linux picks
                     Desktop vs System Tray / Browser on first run).
  --webapp           Dev: landing page + tool (VITE_TRANSPORT=worker).
  --landing          Dev: landing page ONLY — no WASM, no server (VITE_LANDING_ONLY).
  --docker           Build & run the self-build Docker image.
  --serve            Run only the Rust psp-server.
  --build-desktop    Production desktop build → dist/.
  --build-appimage   Production Linux AppImage (bundle + host-graphics strip,
                     same pipeline as release CI) → dist/. Linux only.
  --build-web        Production web build (landing page) → ui_build/.
  --build            Plain SPA build (server-served) → ui_build/.

options:
  --check, --doctor  Run only the preflight for the selected mode, then exit.
                     Combine with a mode flag (e.g. --check --desktop).
  --install-wasm     Install the WASM toolchain (wasm32 target + wasm-pack).
                     The one opt-in installer; everything else stays
                     fail-with-instructions. Skips anything already present.
  --host <ip>        Host/IP bind or WS_URL host (--web/--serve/--docker).
  --vite-port <p>    Vite port (default 7258).
  --server-port <p>  psp-server port (default 7257).
  --no-server        (--web) skip psp-server (Vite only).
  --skip-check       Skip the preflight (advanced).
  --no-install       Skip bun install if node_modules exists.
  --rebuild-wasm     (--webapp/--build-web) force wasm-pack rebuild.
  --json             Machine-readable preflight JSON (implies --check).
  --force-check-mode <m>  Override the preflight mode (advanced).
  -h, --help         Show this help.

Windows users: run dev.ps1 instead.
EOF
}

ARG_MODE=""; ARG_CHECK=0; ARG_INSTALL_WASM=0; ARG_HOST=""
ARG_VITE_PORT=""; ARG_SERVER_PORT=""; ARG_NO_SERVER=0; ARG_SKIP_CHECK=0
ARG_NO_INSTALL=0; ARG_REBUILD_WASM=0; ARG_JSON=0; ARG_FORCE_CHECK_MODE=""

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --web) ARG_MODE="web"; shift ;;
            --desktop) ARG_MODE="desktop"; shift ;;
            --webapp) ARG_MODE="webapp"; shift ;;
            --landing) ARG_MODE="landing"; shift ;;
            --docker) ARG_MODE="docker"; shift ;;
            --serve) ARG_MODE="serve"; shift ;;
            --build-desktop) ARG_MODE="build-desktop"; shift ;;
            --build-appimage) ARG_MODE="build-appimage"; shift ;;
            --build-web) ARG_MODE="build-web"; shift ;;
            --build) ARG_MODE="build"; shift ;;
            --check|--doctor) ARG_CHECK=1; shift ;;
            --install-wasm) ARG_INSTALL_WASM=1; shift ;;
            --host) ARG_HOST="$2"; shift 2 ;;
            --vite-port) ARG_VITE_PORT="$2"; shift 2 ;;
            --server-port) ARG_SERVER_PORT="$2"; shift 2 ;;
            --no-server) ARG_NO_SERVER=1; shift ;;
            --skip-check) ARG_SKIP_CHECK=1; shift ;;
            --no-install) ARG_NO_INSTALL=1; shift ;;
            --rebuild-wasm) ARG_REBUILD_WASM=1; shift ;;
            --json) ARG_JSON=1; ARG_CHECK=1; shift ;;
            --force-check-mode) ARG_FORCE_CHECK_MODE="$2"; shift 2 ;;
            -h|--help) usage; exit 0 ;;
            *) log_fail "Unknown argument: $1"; usage; exit 2 ;;
        esac
    done
}

main() {
    parse_args "$@"

    local mode="${ARG_FORCE_CHECK_MODE:-${ARG_MODE:-web}}"

    if (( ARG_INSTALL_WASM )); then
        run_install_wasm
        return $?
    fi

    snapshot_env
    RESTORE_ENV=1
    # Capture OUR process-group ID now, before any child spawns. All children
    # run in this same group (no setsid), so a negative-PGID kill in cleanup
    # reaches the whole tree even if individual child PIDs have been reaped.
    export EASYRUN_PGID="$(ps -o pgid= -p $$ 2>/dev/null | tr -d ' ')"
    # Install the cleanup trap EARLY — before any child is spawned — so an
    # interrupt at ANY point (during preflight, bun install, wait_for_http, or
    # the run loop) tears down spawned children. INT/TERM print a message and
    # exit 130; EXIT handles the normal/failure paths. Children share our
    # process group, so Ctrl-C reaches them directly too; the trap is the
    # guarantee for anything that survives or for non-interactive kills.
    _cleanup_done=0
    _on_exit_or_interrupt() {
        # Guard against double-cleanup (EXIT fires after INT handler's exit).
        (( _cleanup_done )) && return 0
        _cleanup_done=1
        cleanup_children
        restore_env_on_exit
    }
    _on_interrupt() {
        printf '\n%sInterrupted — cleaning up…%s\n' "$YELLOW" "$RESET" >&2
        _on_exit_or_interrupt
        exit 130
    }
    trap _on_interrupt INT TERM
    trap _on_exit_or_interrupt EXIT

    if (( ARG_CHECK )); then
        if (( ! ARG_JSON )); then
            banner "Environment check  (mode: ${mode})"
        fi
        local rc=0
        report_preflight "$mode" "$ARG_JSON" || rc=$?
        return $rc
    fi

    if (( ! ARG_SKIP_CHECK )); then
        banner "Preflight  (mode: ${mode})"
        if ! report_preflight "$mode" 0; then
            printf '\n%sPreflight reported critical issues — aborting.%s\n' "$RED" "$RESET" >&2
            printf '%sRe-run with --skip-check to bypass (not recommended).%s\n' "$DIM" "$RESET" >&2
            return 1
        fi
        printf '\n' >&2
    fi

    # --no-install: stub ensure_bun_install to a no-op.
    if (( ARG_NO_INSTALL )); then
        ensure_bun_install() { log_info "--no-install: skipping bun install."; }
    fi

    case "${ARG_MODE:-web}" in
        web) run_web ;;
        desktop) run_desktop ;;
        webapp) run_webapp ;;
        landing) run_landing ;;
        docker) run_docker ;;
        serve) run_serve ;;
        build-desktop) run_build_desktop ;;
        build-appimage) run_build_appimage ;;
        build-web) run_build_web ;;
        build) run_build_plain ;;
        *) die "internal: unknown mode ${ARG_MODE}" ;;
    esac
}

main "$@"
