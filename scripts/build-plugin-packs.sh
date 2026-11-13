#!/usr/bin/env bash
# Package the official plugins into installable archives served by the app
# (Settings → Plugins accepts these zips). Run from the repo root.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
src="$root/plugins/official"
out="$root/frontend/public/plugin-packs"
mkdir -p "$out"
for dir in "$src"/*/; do
  name="$(basename "$dir")"
  rm -f "$out/$name.zip"
  (cd "$dir" && zip -q "$out/$name.zip" manifest.json main.mjs)
  echo "packed $name.zip"
done
