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
  # main.mjs only when the plugin ships JS (themes-only packs have none);
  # plus every theme CSS file (flat or nested, never node_modules).
  css_files="$(cd "$dir" && find . -name '*.css' -not -path './node_modules/*' | sed 's|^\./||')"
  # shellcheck disable=SC2086
  (cd "$dir" && zip -q "$out/$name.zip" manifest.json $([ -f main.mjs ] && echo main.mjs) $css_files)
  echo "packed $name.zip"
done
