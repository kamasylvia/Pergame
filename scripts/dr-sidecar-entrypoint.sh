#!/usr/bin/env bash
# dr-backup sidecar entrypoint (spec backup-restore.md §10).
#
# Watches $JOBS_DIR/pending/*.json (job requests written by the Pergame
# backend admin API) and executes dr-backup.sh for each one, writing the
# result to $JOBS_DIR/done/<id>.json. Optional scheduled backups run every
# SCHEDULE_HOURS hours when SCHEDULE_HOURS > 0.
#
# Job file shape:  { "id": "...", "action": "backup|list|restore",
#                    "flags": ["-R pergame", "-t ...", "-y"] }
# Done file shape: { "id", "action", "exit", "output_tail", "finished_at" }
set -u

JOBS_DIR="${JOBS_DIR:-/jobs}"
CONF_DIR="${CONF_DIR:-/conf}"
SCAN_INTERVAL_SECONDS="${SCAN_INTERVAL_SECONDS:-2}"
SCHEDULE_HOURS="${SCHEDULE_HOURS:-0}"

CONF_FILE="${CONF_DIR}/dr-backup.conf"
SCRIPT="/opt/dr/dr-backup.sh"
TAIL_BYTES=65536
mkdir -p "$JOBS_DIR/pending" "$JOBS_DIR/done"

log() { printf '[%s] [sidecar] %s\n' "$(date '+%F %T')" "$*"; }

# Minimal JSON string escaping (no python in this image). Newlines and
# carriage returns MUST become \\n / \\r escapes: a raw control character
# inside a JSON string makes the whole job result unparseable for the API.
json_escape() {
  printf '%s' "$1" |
    sed -e 's/\\/\\\\/g' \
        -e 's/"/\\"/g' \
        -e 's/\r/\\r/g' \
        -e 's/\t/\\t/g' |
    awk 'BEGIN { ORS = "" } { if (NR > 1) printf "\\n"; print }' |
    tr -d '\000-\010\013\014\016-\037'
}

# execute <label> <args...> — runs dr-backup.sh with the shared conf and
# streams output to stdout (caller captures). Returns the script exit code.
execute() {
  local label="$1"
  shift
  log "$label: dr-backup.sh $*"
  "$SCRIPT" -c "$CONF_FILE" "$@"
}

# write_done <id> <action> <exit> <outfile>
write_done() {
  local id="$1" action="$2" exit="$3" outfile="$4"
  local tail escaped tmp
  tail="$(tail -c "$TAIL_BYTES" "$outfile" 2>/dev/null || true)"
  escaped="$(json_escape "$tail")"
  tmp="$JOBS_DIR/done/.${id}.tmp"
  printf '{"id":"%s","action":"%s","exit":%s,"output_tail":"%s","finished_at":"%s"}' \
    "$id" "$action" "$exit" "$escaped" "$(date -u '+%Y-%m-%dT%H:%M:%SZ')" >"$tmp"
  mv "$tmp" "$JOBS_DIR/done/${id}.json"
}

last_schedule_run=0

log "sidecar started; jobs=$JOBS_DIR conf=$CONF_FILE schedule=${SCHEDULE_HOURS}h"

while true; do
  # Scheduled backup (best-effort; skipped while a conf is absent).
  if [ "$SCHEDULE_HOURS" -gt 0 ] && [ -r "$CONF_FILE" ]; then
    now="$(date +%s)"
    due=$((last_schedule_run + SCHEDULE_HOURS * 3600))
    if [ "$now" -ge "$due" ]; then
      last_schedule_run="$now"
      out="$(mktemp "${TMPDIR:-/tmp}/dr-sidecar-sched.XXXXXX")"
      execute "scheduled-backup" >"$out" 2>&1
      code=$?
      log "scheduled backup exit=$code; tail: $(tail -n 3 "$out" | tr '\n' ' ')"
      rm -f "$out"
    fi
  fi

  for job in "$JOBS_DIR"/pending/*.json; do
    [ -e "$job" ] || break
    id="$(sed -n 's/.*"id":"\([^"]*\)".*/\1/p' "$job" | head -1)"
    action="$(sed -n 's/.*"action":"\([^"]*\)".*/\1/p' "$job" | head -1)"
    # flags are written by the backend from a fixed per-action allowlist.
    flags="$(sed -n 's/.*"flags":\[\(.*\)\].*/\1/p' "$job" | tr -d '[]"' | sed 's/,/ /g')"
    [ -n "$id" ] || id="unknown"
    [ -n "$action" ] || action="unknown"
    log "job $id ($action) picked up"

    out="$(mktemp "${TMPDIR:-/tmp}/dr-sidecar-job.XXXXXX")"
    if [ ! -r "$CONF_FILE" ]; then
      printf 'ERROR: conf not readable at %s\n' "$CONF_FILE" >"$out"
      exit=1
    else
      case "$action" in
        backup)  execute "job $id" >"$out" 2>&1; exit=$? ;;
        list)    execute "job $id" -l >"$out" 2>&1; exit=$? ;;
        restore) execute "job $id" $flags >"$out" 2>&1; exit=$? ;;
        *)       printf 'unknown action: %s\n' "$action" >"$out"; exit=2 ;;
      esac
    fi
    log "job $id ($action) exit=$exit"
    write_done "$id" "$action" "$exit" "$out"
    rm -f "$out" "$job"
  done

  sleep "$SCAN_INTERVAL_SECONDS"
done
