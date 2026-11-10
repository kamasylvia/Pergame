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
#   -T <name>    backup only the destination named <name> (used by the
#                per-destination cron scheduler in the backend)
#   -p <name>    probe: test connectivity of destination or source <name> and
#                print a PROBE|... result line (used by the admin UI test button)
#   -h           this help
#
# Requires: pg_dump, rclone. Per-destination cron lives in the conf records
# (fields 11-13: retain_days|keep_last|cron|crypt_pass); the backend scheduler
# fires `dr-backup.sh -T <name>` per due destination. Host cron still works:
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
TARGET_FILTER=""
PROBE_NAME=""

usage() { grep '^#   ' "$0" | sed 's/^#   //'; exit 0; }
while getopts ":c:lnt:R:yT:p:h" opt; do
  case "$opt" in
    c) CONF="$OPTARG" ;;
    l) LIST_ONLY=1 ;;
    n) DRY_RUN=1 ;;
    t) RESTORE_TS="$OPTARG" ;;
    R) RESTORE_SRC="$OPTARG" ;;
    y) ASSUME_YES=1 ;;
    T) TARGET_FILTER="$OPTARG" ;;
    p) PROBE_NAME="$OPTARG" ;;
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

# Probe mode only needs rclone/psql, and the matched target decides which; the
# strict pg_dump+rclone gate below applies to the backup path.
if [ -z "$PROBE_NAME" ]; then
  for bin in pg_dump rclone; do
    if ! command -v "$bin" >/dev/null; then
      if [ "$DRY_RUN" = 1 ]; then
        log "WARN: $bin not found (allowed in dry-run mode)"
      else
        fail "Required binary not found: $bin"; exit 1
      fi
    fi
  done
fi

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

# parse_dest <record> — split one destination record into D_* globals.
# Trailing knobs added 2026-11-09 (absent in old confs → inherit/off):
#   s3|name|endpoint|bucket|prefix|access|secret|secure|region|provider[|retain_days|keep_last|cron|crypt]
#   webdav|name|url|user|password|prefix[|retain_days|keep_last|cron|crypt]
parse_dest() {
  D_KIND="${1%%|*}"
  if [ "$D_KIND" = s3 ]; then
    IFS='|' read -r _ D_NAME D_ENDPOINT D_BUCKET D_PREFIX D_USER D_SECRET D_SECURE D_REGION D_PROVIDER D_RETAIN D_KEEP D_CRON D_CRYPT <<<"$1"
    D_URL=""
  else
    IFS='|' read -r _ D_NAME D_URL D_USER D_SECRET D_PREFIX D_RETAIN D_KEEP D_CRON D_CRYPT <<<"$1"
    D_ENDPOINT="$D_URL"
    D_BUCKET=""
    D_SECURE=true
    D_REGION=""
    D_PROVIDER=""
  fi
  D_RETAIN="${D_RETAIN:-0}"
  D_KEEP="${D_KEEP:-0}"
  D_CRON="${D_CRON:-}"
  D_CRYPT="${D_CRYPT:-}"
}

# apply_crypt <base-remote-name> <_unused-root> — sets C_REMOTE for the
# current destination (parse_dest'd): the crypt remote name when D_CRYPT is
# set (its section was generated by build_rclone_config), else the base.
apply_crypt() {
  local base="$1"
  if [ -z "$D_CRYPT" ]; then
    C_REMOTE="${base}:"
    return
  fi
  C_REMOTE="drb_crypt_${D_NAME}:"
}

# dest_prefix_root <record> — object-store path down to the destination root
# (bucket for S3, remote root for WebDAV), crypt-wrapped when enabled.
dest_prefix_root() {
  parse_dest "$1"
  if [ "$D_KIND" = s3 ]; then
    local r
    r="$(define_s3_remote "$1")"
    if [ -n "$D_CRYPT" ]; then
      apply_crypt "$r" "$D_BUCKET"
      printf '%s' "$C_REMOTE"
    else
      printf '%s:%s' "$r" "$D_BUCKET"
    fi
  else
    local r
    r="$(define_webdav_remote "$1")"
    if [ -n "$D_CRYPT" ]; then
      apply_crypt "$r" ""
      printf '%s' "$C_REMOTE"
    else
      printf '%s:' "$r"
    fi
  fi
}

# dest_source_dir <record> <source-name> — full remote path of one source's
# snapshot directory ("<root>/<prefix>/<source>").
dest_source_dir() {
  local root src
  root="$(dest_prefix_root "$1")"
  parse_dest "$1"
  src="$2"
  printf '%s/%s%s' "${root%/}" "${D_PREFIX:+$D_PREFIX/}" "$src"
}

# dest_fingerprint <record> — canonical physical-store identity (Siyuan
# deletecheck parity: same store via different configs shares a fingerprint).
# Field separator is a comma, not a pipe: the fingerprint travels inside the
# pipe-delimited PROBE line and must not break its field structure.
dest_fingerprint() {
  parse_dest "$1"
  if [ "$D_KIND" = s3 ]; then
    printf 's3,%s,%s,%s' \
      "$(resolve_host "$(url_host "$D_ENDPOINT")"):$(url_port "$D_ENDPOINT" 443)" \
      "$D_BUCKET" "$(norm_prefix "$D_PREFIX")"
  else
    printf 'webdav,%s,%s' \
      "$(resolve_host "$(url_host "$D_URL")"):$(url_port "$D_URL" 443)" \
      "$(norm_prefix "$D_PREFIX")"
  fi
}

# eff_retain_days — per-destination retain_days, falling back to the global.
eff_retain_days() {
  if [ "${D_RETAIN:-0}" -gt 0 ]; then printf '%s' "$D_RETAIN"; else printf '%s' "$RETAIN_DAYS"; fi
}

# rclone remote definitions are written to a private config file under
# WORKDIR instead of RCLONE_CONFIG_<name>_* env sections: rclone ≥ 1.76 no
# longer resolves remote types from env sections ("didn't find section in
# config file"), while a real --config file works on every version.
# build_rclone_config regenerates the whole file from the ACTIVE destinations
# (called once after the -T filter); define_*_remote only echo remote names.
RCLONE_CONF_FILE=""

build_rclone_config() {
  RCLONE_CONF_FILE="${WORKDIR}/rclone.conf"
  : >"$RCLONE_CONF_FILE"
  export RCLONE_CONFIG="$RCLONE_CONF_FILE"
  local rec
  for rec in ${ACTIVE_S3[@]+"${ACTIVE_S3[@]}"}; do
    parse_dest "$rec"
    local obscured="DRYRUN_PLACEHOLDER"
    if [ -n "$D_CRYPT" ] && [ "$DRY_RUN" != 1 ] && command -v rclone >/dev/null; then
      obscured="$(rclone obscure "$D_CRYPT")"
    fi
    {
      printf '[drb_s3_%s]\n' "$D_NAME"
      printf 'type = s3\n'
      printf 'provider = %s\n' "${D_PROVIDER:-Other}"
      printf 'endpoint = %s\n' "$D_ENDPOINT"
      printf 'access_key_id = %s\n' "$D_USER"
      printf 'secret_access_key = %s\n' "$D_SECRET"
      printf 'use_ssl = %s\n' "${D_SECURE:-true}"
      printf 'region = %s\n' "${D_REGION:-us-east-1}"
      if [ -n "$D_CRYPT" ]; then
        printf '\n[drb_crypt_%s]\n' "$D_NAME"
        printf 'type = crypt\n'
        printf 'remote = drb_s3_%s:%s\n' "$D_NAME" "$D_BUCKET"
        printf 'pass = %s\n' "$obscured"
      fi
      printf '\n'
    } >>"$RCLONE_CONF_FILE"
  done
  for rec in ${ACTIVE_DAV[@]+"${ACTIVE_DAV[@]}"}; do
    parse_dest "$rec"
    local obscured="DRYRUN_PLACEHOLDER"
    if [ "$DRY_RUN" != 1 ] && command -v rclone >/dev/null; then
      obscured="$(rclone obscure "$D_SECRET")"
    fi
    {
      printf '[drb_dav_%s]\n' "$D_NAME"
      printf 'type = webdav\n'
      printf 'url = %s\n' "$D_URL"
      printf 'vendor = other\n'
      printf 'user = %s\n' "$D_USER"
      printf 'pass = %s\n' "$obscured"
      if [ -n "$D_CRYPT" ]; then
        printf '\n[drb_crypt_%s]\n' "$D_NAME"
        printf 'type = crypt\n'
        printf 'remote = drb_dav_%s:\n' "$D_NAME"
        printf 'pass = %s\n' "$obscured"
      fi
      printf '\n'
    } >>"$RCLONE_CONF_FILE"
  done
}

# Remote names only — the definitions themselves live in the generated file.
# (_rest is mandatory: bash read assigns the LAST variable the whole line
# remainder, so a two-variable read would swallow the record into the name.)
define_s3_remote() {            # s3|name|...
  IFS='|' read -r _ name _rest <<<"$1"
  printf '%s' "drb_s3_${name}"
}

define_webdav_remote() {        # webdav|name|...
  IFS='|' read -r _ name _rest <<<"$1"
  printf '%s' "drb_dav_${name}"
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

# -T <name>: per-destination cron backups target one record. Names are
# expected to be unique across kinds; a match in both kinds would back up to
# both, so keep every match and fail only when nothing matches.
if [ -n "$TARGET_FILTER" ]; then
  S3F=()
  DAVF=()
  for dest in ${ACTIVE_S3[@]+"${ACTIVE_S3[@]}"}; do
    parse_dest "$dest"
    if [ "$D_NAME" = "$TARGET_FILTER" ]; then S3F+=("$dest"); fi
  done
  for dest in ${ACTIVE_DAV[@]+"${ACTIVE_DAV[@]}"}; do
    parse_dest "$dest"
    if [ "$D_NAME" = "$TARGET_FILTER" ]; then DAVF+=("$dest"); fi
  done
  if [ "$(( ${#S3F[@]} + ${#DAVF[@]} ))" -eq 0 ]; then
    fail "destination '${TARGET_FILTER}' not found"
    exit 1
  fi
  ACTIVE_S3=()
  ACTIVE_DAV=()
  for dest in ${S3F[@]+"${S3F[@]}"}; do ACTIVE_S3+=("$dest"); done
  for dest in ${DAVF[@]+"${DAVF[@]}"}; do ACTIVE_DAV+=("$dest"); done
fi

# Generate the private rclone config for every active destination (plain +
# crypt sections). No-op in probe mode is not desired: probe needs it too, so
# it runs unconditionally right here.
build_rclone_config

# ---------------- retention --------------------------------------------------
# prune_dest <source-dir> — age + keep-last retention for one source on the
# current destination (D_RETAIN/D_KEEP set by parse_dest). Both dimensions are
# optional (Siyuan parity); a zero disables that dimension.
prune_dest() {
  local base="$1" rd stamps n=0 stamp
  rd="$(eff_retain_days)"
  if [ "$rd" -gt 0 ]; then
    log "  prune: removing dumps older than ${rd}d"
    run rclone delete --min-age "${rd}d" "$base" --log-level NOTICE || \
      log "WARN: age prune failed (non-fatal)"
  fi
  if [ "$D_KEEP" -gt 0 ]; then
    stamps="$(rclone lsf --dirs-only "$base" 2>/dev/null | tr -d '/' | sort -r || true)"
    while IFS= read -r stamp; do
      [ -n "$stamp" ] || continue
      n=$((n + 1))
      if [ "$n" -le "$D_KEEP" ]; then continue; fi
      log "  prune: keep-last removing ${stamp} (beyond ${D_KEEP})"
      run rclone purge "${base}/${stamp}" --log-level NOTICE || \
        log "WARN: keep-last purge failed for ${stamp} (non-fatal)"
    done <<<"$stamps"
  fi
}

# ---------------- probe (UI "test connection") -------------------------------
# probe_target <name> — tests one destination (S3 first, then WebDAV) or one
# pg source and prints a single machine-readable line that the backend
# surfaces via the job API:
#   PROBE|<kind>|<name>|reachable=true|snapshots=<n>|fingerprint=<fp>|note=
# probe_target <name>
probe_target() {
  local name="$1" rec base listing snaps fp
  for rec in ${ACTIVE_S3[@]+"${ACTIVE_S3[@]}"}; do
    parse_dest "$rec"
    if [ "$D_NAME" != "$name" ]; then continue; fi
    fp="$(dest_fingerprint "$rec")"
    if [ "$DRY_RUN" = 1 ]; then
      printf 'PROBE|s3|%s|reachable=dry-run|snapshots=|fingerprint=%s|note=dry-run\n' "$name" "$fp"
      return 0
    fi
    base="$(dest_source_dir "$rec" '__probe__')"
    base="${base%/*}"   # drop the placeholder source: path root only
    if ! listing="$(rclone lsf --files-only -R "$base" 2>&1)"; then
      printf 'PROBE|s3|%s|reachable=false|snapshots=|fingerprint=%s|note=%s\n' \
        "$name" "$fp" "$(printf '%s' "$listing" | head -n1 | tr '|/\n' '  ' | head -c 100)"
      return 0
    fi
    snaps="$(printf '%s\n' "$listing" | grep -c '\.dump$' || true)"
    printf 'PROBE|s3|%s|reachable=true|snapshots=%s|fingerprint=%s|note=\n' "$name" "$snaps" "$fp"
    return 0
  done
  for rec in ${ACTIVE_DAV[@]+"${ACTIVE_DAV[@]}"}; do
    parse_dest "$rec"
    if [ "$D_NAME" != "$name" ]; then continue; fi
    fp="$(dest_fingerprint "$rec")"
    if [ "$DRY_RUN" = 1 ]; then
      printf 'PROBE|webdav|%s|reachable=dry-run|snapshots=|fingerprint=%s|note=dry-run\n' "$name" "$fp"
      return 0
    fi
    base="$(dest_source_dir "$rec" '__probe__')"
    base="${base%/*}"
    if ! listing="$(rclone lsf --files-only -R "$base" 2>&1)"; then
      printf 'PROBE|webdav|%s|reachable=false|snapshots=|fingerprint=%s|note=%s\n' \
        "$name" "$fp" "$(printf '%s' "$listing" | head -n1 | tr '|/\n' '  ' | head -c 100)"
      return 0
    fi
    snaps="$(printf '%s\n' "$listing" | grep -c '\.dump$' || true)"
    printf 'PROBE|webdav|%s|reachable=true|snapshots=%s|fingerprint=%s|note=\n' "$name" "$snaps" "$fp"
    return 0
  done
  for rec in ${SOURCES[@]+"${SOURCES[@]}"}; do
    IFS='|' read -r kind sname host port user pass db <<<"$rec"
    if [ "$kind" != pg ] || [ "$sname" != "$name" ]; then continue; fi
    if [ "$DRY_RUN" = 1 ]; then
      printf 'PROBE|pg|%s|reachable=dry-run|snapshots=|fingerprint=|note=dry-run\n' "$name"
      return 0
    fi
    if PGCONNECT_TIMEOUT=10 PGPASSWORD="$pass" psql -h "$host" -p "$port" -U "$user" -d "$db" -tAc 'SELECT 1' >/dev/null 2>&1; then
      printf 'PROBE|pg|%s|reachable=true|snapshots=|fingerprint=|note=\n' "$name"
    else
      printf 'PROBE|pg|%s|reachable=false|snapshots=|fingerprint=|note=psql connect failed\n' "$name"
    fi
    return 0
  done
  printf 'PROBE|unknown|%s|reachable=false|snapshots=|fingerprint=|note=no such source or destination\n' "$name"
  return 1
}

# ---------------- list snapshots -------------------------------------------
list_snapshots() {
  if ! command -v rclone >/dev/null; then
    fail "rclone not found (required for -l)"; exit 1
  fi
  for src in ${SOURCES[@]+"${SOURCES[@]}"}; do
    IFS='|' read -r kind name _host _port _user _pass _db <<<"$src"
    [ "$kind" = "pg" ] || continue
    for dest in ${ACTIVE_S3[@]+"${ACTIVE_S3[@]}"}; do
      parse_dest "$dest"
      base="$(dest_source_dir "$dest" "$name")"
      log "snapshots source='${name}' dest=s3/${D_NAME}:"
      rclone lsf --dirs-only "$base" 2>/dev/null | sort -r | sed 's/^/  /'
    done
    for dest in ${ACTIVE_DAV[@]+"${ACTIVE_DAV[@]}"}; do
      parse_dest "$dest"
      base="$(dest_source_dir "$dest" "$name")"
      log "snapshots source='${name}' dest=webdav/${D_NAME}:"
      rclone lsf --dirs-only "$base" 2>/dev/null | sort -r | sed 's/^/  /'
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
    parse_dest "$dest"
    base="$(dest_source_dir "$dest" "$name")"
    ts="$RESTORE_TS"
    if [ -z "$ts" ]; then
      ts="$(rclone lsf --dirs-only "$base" 2>/dev/null | sort -r | head -1 | tr -d '/')"
    fi
    if [ -n "$ts" ] && rclone lsf "$base/$ts/" >/dev/null 2>&1; then
      found="s3/${D_NAME}"; remoteFile="${base}/${ts}/${name}.dump"; break
    fi
  done
  if [ -z "$found" ]; then
    for dest in ${ACTIVE_DAV[@]+"${ACTIVE_DAV[@]}"}; do
      parse_dest "$dest"
      base="$(dest_source_dir "$dest" "$name")"
      ts="$RESTORE_TS"
      if [ -z "$ts" ]; then
        ts="$(rclone lsf --dirs-only "$base" 2>/dev/null | sort -r | head -1 | tr -d '/')"
      fi
      if [ -n "$ts" ] && rclone lsf "$base/$ts/" >/dev/null 2>&1; then
        found="webdav/${D_NAME}"; remoteFile="${base}/${ts}/${name}.dump"; break
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

if [ -n "$PROBE_NAME" ]; then
  probe_target "$PROBE_NAME"
  exit $?
fi

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
  # NOTE: dest_source_dir runs in a command-substitution subshell, so its
  # D_* side effects never reach this shell — parse_dest here first.
  for dest in ${ACTIVE_S3[@]+"${ACTIVE_S3[@]}"}; do
    parse_dest "$dest"
    base="$(dest_source_dir "$dest" "$name")"
    destPath="${base}/${STAMP}"
    log "S3 destination '${D_NAME}': copying ${name}.dump -> ${destPath}"
    if ! run rclone copy "$dumpFile" "$destPath" --log-level NOTICE; then
      fail "S3 destination '${D_NAME}': copy failed for '${name}'"
      TOTAL_FAIL=$((TOTAL_FAIL+1)); FAILURES+=("${name}->s3/${D_NAME}:copy")
      continue
    fi
    # Size verify compares plaintext bytes; the crypt layer stores its own
    # authenticated containers whose size differs, so it skips the check
    # (rclone crypt verifies integrity internally via nonce+MAC).
    if [ "$VERIFY_UPLOAD" = 1 ] && [ "$DRY_RUN" != 1 ] && [ -z "$D_CRYPT" ]; then
      remoteSize=$(rclone ls "$destPath" 2>/dev/null | awk '{s+=$1} END {print s+0}')
      if [ "$remoteSize" != "$size" ]; then
        fail "S3 destination '${D_NAME}': size mismatch (local ${size}, remote ${remoteSize})"
        TOTAL_FAIL=$((TOTAL_FAIL+1)); FAILURES+=("${name}->s3/${D_NAME}:verify")
        continue
      fi
    fi
    prune_dest "$base"
  done

  for dest in ${ACTIVE_DAV[@]+"${ACTIVE_DAV[@]}"}; do
    parse_dest "$dest"
    base="$(dest_source_dir "$dest" "$name")"
    destPath="${base}/${STAMP}"
    if [ "$DRY_RUN" = 1 ] && [ -n "$D_CRYPT" ]; then
      log "DRY-RUN: webdav remote '${D_NAME}' uses crypt (pass obscured by rclone in real mode)"
    fi
    log "WebDAV destination '${D_NAME}': copying ${name}.dump -> ${destPath}"
    if ! run rclone copy "$dumpFile" "$destPath" --log-level NOTICE; then
      fail "WebDAV destination '${D_NAME}': copy failed for '${name}'"
      TOTAL_FAIL=$((TOTAL_FAIL+1)); FAILURES+=("${name}->dav/${D_NAME}:copy")
      continue
    fi
    if [ "$VERIFY_UPLOAD" = 1 ] && [ "$DRY_RUN" != 1 ] && [ -z "$D_CRYPT" ]; then
      remoteSize=$(rclone ls "$destPath" 2>/dev/null | awk '{s+=$1} END {print s+0}')
      if [ "$remoteSize" != "$size" ]; then
        fail "WebDAV destination '${D_NAME}': size mismatch (local ${size}, remote ${remoteSize})"
        TOTAL_FAIL=$((TOTAL_FAIL+1)); FAILURES+=("${name}->dav/${D_NAME}:verify")
        continue
      fi
    fi
    prune_dest "$base"
  done
done

if [ "$TOTAL_FAIL" -gt 0 ]; then
  fail "Completed with ${TOTAL_FAIL} failure(s): ${FAILURES[*]}"
  exit 1
fi
log "All sources backed up to all destinations."
