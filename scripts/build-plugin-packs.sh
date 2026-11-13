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
  # Plugins with a src/ tree are bundled here (main.mjs is a build artifact).
  if [ -d "$dir/src" ]; then
    (cd "$dir" && bun build src/main.ts --target browser --format esm --outfile main.mjs >/dev/null)
  fi
  (cd "$dir" && zip -q "$out/$name.zip" manifest.json main.mjs)
  echo "packed $name.zip"
done
