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
#   dr-backup.sh [options]
# Options:
#   -c FILE   config file (default: ./dr-backup.conf next to this script)
#   -n        dry run: print actions, dump nothing
#   -h        this help
#
# Requires: pg_dump, rclone. Cron example (daily 03:30):
#   30 3 * * * /opt/collectanea/scripts/dr-backup.sh >> /var/log/dr-backup.log 2>&1
#
# Restore:
#   rclone copy "<remote>:<prefix>/<source>/<timestamp>/" <local-dir>
#   pg_restore --clean --if-exists -h <host> -U <user> -d <db> <dump.dump>
# ==============================================================================
set -Eeuo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONF="${SCRIPT_DIR}/dr-backup.conf"
DRY_RUN=0

usage() { grep '^#   ' "$0" | sed 's/^#   //'; exit 0; }
while getopts ":c:nh" opt; do
  case "$opt" in
    c) CONF="$OPTARG" ;;
    n) DRY_RUN=1 ;;
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
  for dest in ${S3_DESTINATIONS[@]+"${S3_DESTINATIONS[@]}"}; do
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

  for dest in ${WEBDAV_DESTINATIONS[@]+"${WEBDAV_DESTINATIONS[@]}"}; do
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
