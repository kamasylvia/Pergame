#!/usr/bin/env bash
# ==============================================================================
# dr-backup — PostgreSQL disaster-recovery backups to typed destinations.
#
# Shape: X PostgreSQL sources -> every destination among Y S3 + Z WebDAV.
# Dumps each source (pg_dump custom format), fans every dump out to every
# destination (S3/WebDAV remotes are built on the fly via rclone connection
# environment — no pre-made rclone config needed), verifies uploads, prunes by
# retention age.
#
# Usage:
#   dr-backup.sh [options]        backup every source to every destination
#   dr-backup.sh -l               list snapshots per source/destination
#   dr-backup.sh -R <source> ...  restore a snapshot into its source database
# Options:
#   -c FILE      config file (default: ./dr-backup.conf next to this script)
#   -n           dry run: print actions, change nothing
#   -l           list snapshots (per source per destination, newest first)
#   -R <source>  restore mode: pull <source>'s snapshot and pg_restore it
#   -t <STAMP>   snapshot timestamp dir for -R (default: newest)
#   -y           non-interactive confirm for -R (--clean drops existing objects)
#   -h           this help
#
# Requires: pg_dump, rclone. Cron example (daily 03:30):
#   30 3 * * * /opt/pergame/scripts/dr-backup.sh >> /var/log/dr-backup.log 2>&1
#
# Restore: dr-backup.sh -R <source> [-t <timestamp>] [-y] (see Options)
# ==============================================================================
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF="${SCRIPT_DIR}/dr-backup.conf"
DRY_RUN=0
LIST_ONLY=0
RESTORE_SRC=""
RESTORE_TS=""
ASSUME_YES=0

usage() { grep '^#   ' "$0" | sed 's/^#   //'; exit 0; }
while getopts ":c:lnt:R:yh" opt; do
  case "$opt" in
    c) CONF="$OPTARG" ;;
    l) LIST_ONLY=1 ;;
    n) DRY_RUN=1 ;;
    t) RESTORE_TS="$OPTARG" ;;
    R) RESTORE_SRC="$OPTARG" ;;
    y) ASSUME_YES=1 ;;
    h) usage ;;
    \?) echo "Unknown option: -$OPTARG" >&2; exit 2 ;;
  esac
done

log()  { printf '[%s] %s\n' "$(date '+%F %T')" "$*"; }
fail() { log "ERROR: $*"; }

[ -r "$CONF" ] || { fail "Config not readable: $CONF (see dr-backup.conf.example)"; exit 1; }
# shellcheck disable=SC1090
source "$CONF"

RETAIN_DAYS="${RETAIN_DAYS:-14}"
VERIFY_UPLOAD="${VERIFY_UPLOAD:-1}"

for bin in pg_dump rclone; do
  if ! command -v "$bin" >/dev/null; then
    if [ "$DRY_RUN" = 1 ]; then
      log "WARN: $bin not found (allowed in dry-run mode)"
    else
      fail "Required binary not found: $bin"; exit 1
    fi
  fi
done

run() {
  if [ "$DRY_RUN" = 1 ]; then log "DRY-RUN: $*"; else "$@"; fi
}

# resolve_host <host> — print the first IPv4 for <host>, falling back to the
# hostname itself (dedup still groups when no resolver is available).
resolve_host() {
  local h="$1" out=""
  if command -v getent >/dev/null 2>&1; then
    out="$(getent ahostsv4 "$h" 2>/dev/null | awk '{print $1; exit}')"
  fi
  if [ -z "$out" ] && command -v host >/dev/null 2>&1; then
    out="$(host "$h" 2>/dev/null | awk '/has address/{print $4; exit}')"
  fi
  if [ -z "$out" ] && command -v python3 >/dev/null 2>&1; then
    out="$(python3 -c 'import socket,sys
try: print(socket.gethostbyname(sys.argv[1]))
except Exception: pass' "$h" 2>/dev/null)"
  fi
  printf '%s' "${out:-$h}"
}

url_host() {
  local rest="${1#*://}"
  rest="${rest%%/*}"; rest="${rest%%\?*}"
  printf '%s' "${rest%%:*}"
}

url_port() {
  local rest="${1#*://}"; rest="${rest%%/*}"
  case "$rest" in
    *:*) printf '%s' "${rest##*:}" ;;
    *)   printf '%s' "$2" ;;
  esac
}

norm_prefix() { p="${1#/}"; p="${p%/}"; printf '%s' "${p:--}"; }

# define_remote <remote-name> — exports RCLONE_CONFIG_* env vars for one
# destination record and echoes the record again for logging.
define_s3_remote() {            # s3|name|endpoint|bucket|prefix|access|secret|secure|region|provider
  IFS='|' read -r _ name endpoint bucket _prefix access secret secure region provider <<<"$1"
  local r="drb_s3_${name}"
  export "RCLONE_CONFIG_${r}_TYPE=s3"
  export "RCLONE_CONFIG_${r}_PROVIDER=${provider:-Other}"
  export "RCLONE_CONFIG_${r}_ENDPOINT=${endpoint}"
  export "RCLONE_CONFIG_${r}_ACCESS_KEY_ID=${access}"
  export "RCLONE_CONFIG_${r}_SECRET_ACCESS_KEY=${secret}"
  export "RCLONE_CONFIG_${r}_USE_SSL=${secure:-true}"
  export "RCLONE_CONFIG_${r}_REGION=${region:-us-east-1}"
  printf '%s' "${r}"
}

define_webdav_remote() {        # webdav|name|url|user|password|prefix
  IFS='|' read -r _ name url user pass _prefix <<<"$1"
  local r="drb_dav_${name}"
  local obscured=""
  if command -v rclone >/dev/null && [ "$DRY_RUN" != 1 ]; then
    obscured="$(rclone obscure "$pass")"
  else
    obscured="DRYRUN_PLACEHOLDER"
  fi
  export "RCLONE_CONFIG_${r}_TYPE=webdav"
  export "RCLONE_CONFIG_${r}_URL=${url}"
  export "RCLONE_CONFIG_${r}_VENDOR=other"
  export "RCLONE_CONFIG_${r}_USER=${user}"
  export "RCLONE_CONFIG_${r}_PASS=${obscured}"
  printf '%s' "${r}"
}

STAMP="$(date '+%Y-%m-%d_%H%M%S')"
WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/dr-backup.XXXXXX")"
trap 'rm -rf "$WORKDIR"' EXIT

TOTAL_FAIL=0
declare -a FAILURES=()

# --- destination dedup: same target via IP and domain counts as one; the
# --- LAST configured record for a target wins, earlier ones auto-disable.
declare -a DEST_KEYS=()     # canonical keys, first-seen order
declare -a DEST_WINNER=()   # winning record per key (same index)
register_dest() {           # <kind> <record>
  local kind="$1" rec="$2" key idx=-1 i
  if [ "$kind" = s3 ]; then
    IFS='|' read -r _ name endpoint bucket prefix _r <<<"$rec"
    key="$(resolve_host "$(url_host "$endpoint")"):$(url_port "$endpoint" 443)|${bucket}|$(norm_prefix "$prefix")"
  else
    IFS='|' read -r _ name url user pass prefix _r <<<"$rec"
    key="$(resolve_host "$(url_host "$url")"):$(url_port "$url" 443)|$(norm_prefix "$prefix")"
  fi
  for i in "${!DEST_KEYS[@]}"; do
    [ "${DEST_KEYS[$i]}" = "$key" ] && { idx=$i; break; }
  done
  if [ "$idx" -ge 0 ]; then
    log "WARN: ${kind} destination '${name}' resolves to an already-configured target (${key}); earlier entry auto-disabled, this one wins"
    DEST_WINNER[$idx]="$rec"
  else
    DEST_KEYS+=("$key")
    DEST_WINNER+=("$rec")
  fi
}
for dest in ${S3_DESTINATIONS[@]+"${S3_DESTINATIONS[@]}"}; do register_dest s3 "$dest"; done
for dest in ${WEBDAV_DESTINATIONS[@]+"${WEBDAV_DESTINATIONS[@]}"}; do register_dest dav "$dest"; done

# Materialize deduped destinations in first-seen order.
ACTIVE_S3=()
ACTIVE_DAV=()
for i in "${!DEST_WINNER[@]}"; do
  rec="${DEST_WINNER[$i]}"
  case "$rec" in s3\|*) ACTIVE_S3+=("$rec");; *) ACTIVE_DAV+=("$rec");; esac
done

# ---------------- list snapshots -------------------------------------------
list_snapshots() {
  if ! command -v rclone >/dev/null; then
    fail "rclone not found (required for -l)"; exit 1
  fi
  for src in ${SOURCES[@]+"${SOURCES[@]}"}; do
    IFS='|' read -r kind name _host _port _user _pass _db <<<"$src"
    [ "$kind" = "pg" ] || continue
    for dest in ${ACTIVE_S3[@]+"${ACTIVE_S3[@]}"}; do
      IFS='|' read -r _ destName bucket prefix _r <<<"$dest"
      remote="$(define_s3_remote "$dest")"
      log "snapshots source='${name}' dest=s3/${destName}:"
      rclone lsf --dirs-only "${remote}:${bucket}/${prefix:+$prefix/}${name}" 2>/dev/null | sort -r | sed 's/^/  /'
    done
    for dest in ${ACTIVE_DAV[@]+"${ACTIVE_DAV[@]}"}; do
      IFS='|' read -r _ destName _url _user _pass prefix <<<"$dest"
      remote="$(define_webdav_remote "$dest")"
      log "snapshots source='${name}' dest=webdav/${destName}:"
      rclone lsf --dirs-only "${remote}:${prefix:+$prefix/}${name}" 2>/dev/null | sort -r | sed 's/^/  /'
    done
  done
}

# ---------------- restore one source snapshot -------------------------------
restore_snapshot() {
  if [ -z "$RESTORE_SRC" ]; then fail "-R requires a source name"; exit 2; fi
  src_record=""
  for src in ${SOURCES[@]+"${SOURCES[@]}"}; do
    IFS='|' read -r kind name _h _p _u _p2 _d <<<"$src"
    [ "$name" = "$RESTORE_SRC" ] && { src_record="$src"; break; }
  done
  [ -n "$src_record" ] || { fail "source '${RESTORE_SRC}' not found in SOURCES"; exit 2; }
  IFS='|' read -r kind name host port user pass db <<<"$src_record"

  dest_count=$(( ${#ACTIVE_S3[@]} + ${#ACTIVE_DAV[@]} ))
  [ "$dest_count" -gt 0 ] || { fail "no destinations configured"; exit 1; }
  if ! command -v rclone >/dev/null; then
    if [ "$DRY_RUN" = 1 ] && [ -n "$RESTORE_TS" ]; then
      log "WARN: rclone not found (dry-run with explicit -t: printing plan only)"
      log "DRY-RUN: rclone copy <dest>:<prefix>/${RESTORE_SRC}/${RESTORE_TS}/${RESTORE_SRC}.dump $WORKDIR/"
      log "DRY-RUN: PGPASSWORD=*** pg_restore --clean --if-exists -h $host -p $port -U $user -d $db $WORKDIR/${RESTORE_SRC}.dump"
      return 0
    fi
    fail "rclone not found (required for -R)"; exit 1
  fi

  # First destination that holds the wanted timestamp dir (fan-out keeps all
  # destinations identical; the first hit wins).
  found=""
  for dest in ${ACTIVE_S3[@]+"${ACTIVE_S3[@]}"}; do
    IFS='|' read -r _ destName bucket prefix _r <<<"$dest"
    remote="$(define_s3_remote "$dest")"
    base="${remote}:${bucket}/${prefix:+$prefix/}${name}"
    ts="$RESTORE_TS"
    if [ -z "$ts" ]; then
      ts="$(rclone lsf --dirs-only "$base" 2>/dev/null | sort -r | head -1 | tr -d '/')"
    fi
    if [ -n "$ts" ] && rclone lsf "$base/$ts/" >/dev/null 2>&1; then
      found="s3/${destName}"; remoteFile="${base}/${ts}/${name}.dump"; break
    fi
  done
  if [ -z "$found" ]; then
    for dest in ${ACTIVE_DAV[@]+"${ACTIVE_DAV[@]}"}; do
      IFS='|' read -r _ destName _url _user _pass prefix <<<"$dest"
      remote="$(define_webdav_remote "$dest")"
      base="${remote}:${prefix:+$prefix/}${name}"
      ts="$RESTORE_TS"
      if [ -z "$ts" ]; then
        ts="$(rclone lsf --dirs-only "$base" 2>/dev/null | sort -r | head -1 | tr -d '/')"
      fi
      if [ -n "$ts" ] && rclone lsf "$base/$ts/" >/dev/null 2>&1; then
        found="webdav/${destName}"; remoteFile="${base}/${ts}/${name}.dump"; break
      fi
    done
  fi
  [ -n "$found" ] || { fail "no snapshot found for source '${RESTORE_SRC}'${RESTORE_TS:+ at $RESTORE_TS}"; exit 1; }
  log "Restore plan: $found -> $found snapshot ${ts}/${RESTORE_SRC}.dump"
  log "Restore target: ${db}@${host}:${port} (pg_restore --clean --if-exists)"
  if [ "$DRY_RUN" = 1 ]; then
    log "DRY-RUN: rclone copy $remoteFile $WORKDIR/"
    log "DRY-RUN: PGPASSWORD=*** pg_restore --clean --if-exists -h $host -p $port -U $user -d $db $WORKDIR/${RESTORE_SRC}.dump"
    return 0
  fi

  if [ "$ASSUME_YES" != 1 ]; then
    if [ -t 0 ]; then
      printf 'pg_restore --clean will DROP existing objects in %s@%s. Type YES to continue: ' "$db" "$host"
      read -r answer
      [ "$answer" = "YES" ] || { fail "aborted"; exit 1; }
    else
      fail "refusing non-interactive restore without -y (--clean drops existing objects)"
      exit 1
    fi
  fi

  log "Pulling snapshot from $found ..."
  if ! run rclone copy "$remoteFile" "$WORKDIR/" --log-level NOTICE; then
    fail "snapshot copy failed"; exit 1
  fi
  dumpFile="${WORKDIR}/${RESTORE_SRC}.dump"
  [ -s "$dumpFile" ] || { fail "downloaded dump is missing/empty"; exit 1; }
  log "Running pg_restore into ${db}@${host}:${port} ..."
  if PGPASSWORD="$pass" pg_restore --clean --if-exists -h "$host" -p "$port" -U "$user" -d "$db" "$dumpFile"; then
    log "Restore of '${RESTORE_SRC}' from ${ts} completed."
  else
    fail "pg_restore reported errors (check above; --clean restores are all-or-nothing per object)"
    exit 1
  fi
}

if [ "$LIST_ONLY" = 1 ]; then
  list_snapshots
  exit 0
fi
if [ -n "$RESTORE_SRC" ]; then
  restore_snapshot
  exit $?
fi

for src in ${SOURCES[@]+"${SOURCES[@]}"}; do
  IFS='|' read -r kind name host port user pass db <<<"$src"
  [ "$kind" = "pg" ] || { fail "Skipping unsupported source kind: $kind ($name)"; continue; }

  dumpFile="${WORKDIR}/${name}.dump"
  log "Source '${name}': dumping ${db}@${host}:${port}"
  if [ "$DRY_RUN" = 1 ]; then
    log "DRY-RUN: PGPASSWORD=*** pg_dump -Fc -h $host -p $port -U $user -d $db -f $dumpFile"
    echo "dummy" >"$dumpFile"   # stand-in so the flow below can be rehearsed
  else
    if ! PGPASSWORD="$pass" pg_dump -Fc -h "$host" -p "$port" -U "$user" -d "$db" -f "$dumpFile"; then
      fail "Source '${name}': pg_dump failed"; TOTAL_FAIL=$((TOTAL_FAIL+1)); FAILURES+=("${name}:pg_dump"); continue
    fi
  fi

  size=$(wc -c <"$dumpFile" | tr -d ' ')
  [ "$size" -gt 0 ] || { fail "Source '${name}': empty dump"; TOTAL_FAIL=$((TOTAL_FAIL+1)); FAILURES+=("${name}:empty"); continue; }
  sha=$(shasum -a 256 "$dumpFile" 2>/dev/null | awk '{print $1}' || sha256sum "$dumpFile" | awk '{print $1}')
  log "Source '${name}': dump ok (${size} bytes, sha256 ${sha:0:12}...)"

  # --- fan out to every destination of every kind ---
  for dest in ${ACTIVE_S3[@]+"${ACTIVE_S3[@]}"}; do
    IFS='|' read -r kind destName bucket prefix _rest <<<"$dest"
    if [ "$kind" != "s3" ]; then fail "Skipping non-s3 record in S3_DESTINATIONS: $kind"; continue; fi
    remote="$(define_s3_remote "$dest")"
    destPath="${remote}:${bucket}/${prefix:+$prefix/}${name}/${STAMP}"
    log "S3 destination '${destName}': copying ${name}.dump -> ${destPath}"
    if ! run rclone copy "$dumpFile" "$destPath" --log-level NOTICE; then
      fail "S3 destination '${destName}': copy failed for '${name}'"
      TOTAL_FAIL=$((TOTAL_FAIL+1)); FAILURES+=("${name}->s3/${destName}:copy")
      continue
    fi
    if [ "$VERIFY_UPLOAD" = 1 ] && [ "$DRY_RUN" != 1 ]; then
      remoteSize=$(rclone ls "$destPath" 2>/dev/null | awk '{s+=$1} END {print s+0}')
      if [ "$remoteSize" != "$size" ]; then
        fail "S3 destination '${destName}': size mismatch (local ${size}, remote ${remoteSize})"
        TOTAL_FAIL=$((TOTAL_FAIL+1)); FAILURES+=("${name}->s3/${destName}:verify")
        continue
      fi
    fi
    log "S3 destination '${destName}': pruning older than ${RETAIN_DAYS}d"
    run rclone delete --min-age "${RETAIN_DAYS}d" "${remote}:${bucket}/${prefix:+$prefix/}${name}" --log-level NOTICE || \
      log "WARN: prune failed on s3/${destName} (non-fatal)"
  done

  for dest in ${ACTIVE_DAV[@]+"${ACTIVE_DAV[@]}"}; do
    IFS='|' read -r kind destName _url _user _pass prefix <<<"$dest"
    if [ "$kind" != "webdav" ]; then fail "Skipping non-webdav record in WEBDAV_DESTINATIONS: $kind"; continue; fi
    remote="$(define_webdav_remote "$dest")"
    if [ "$DRY_RUN" = 1 ]; then
      log "DRY-RUN: webdav remote '${remote}' (pass obscured by rclone in real mode)"
    fi
    destPath="${remote}:${prefix:+$prefix/}${name}/${STAMP}"
    log "WebDAV destination '${destName}': copying ${name}.dump -> ${destPath}"
    if ! run rclone copy "$dumpFile" "$destPath" --log-level NOTICE; then
      fail "WebDAV destination '${destName}': copy failed for '${name}'"
      TOTAL_FAIL=$((TOTAL_FAIL+1)); FAILURES+=("${name}->dav/${destName}:copy")
      continue
    fi
    if [ "$VERIFY_UPLOAD" = 1 ] && [ "$DRY_RUN" != 1 ]; then
      remoteSize=$(rclone ls "$destPath" 2>/dev/null | awk '{s+=$1} END {print s+0}')
      if [ "$remoteSize" != "$size" ]; then
        fail "WebDAV destination '${destName}': size mismatch (local ${size}, remote ${remoteSize})"
        TOTAL_FAIL=$((TOTAL_FAIL+1)); FAILURES+=("${name}->dav/${destName}:verify")
        continue
      fi
    fi
    log "WebDAV destination '${destName}': pruning older than ${RETAIN_DAYS}d"
    run rclone delete --min-age "${RETAIN_DAYS}d" "${remote}:${prefix:+$prefix/}${name}" --log-level NOTICE || \
      log "WARN: prune failed on dav/${destName} (non-fatal)"
  done
done

if [ "$TOTAL_FAIL" -gt 0 ]; then
  fail "Completed with ${TOTAL_FAIL} failure(s): ${FAILURES[*]}"
  exit 1
fi
log "All sources backed up to all destinations."
