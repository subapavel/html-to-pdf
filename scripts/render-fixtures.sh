#!/usr/bin/env bash
# Renders every fixture with htmltopdf into out/<fixture>/htmltopdf.pdf and rasterises pages
# to out/<fixture>/htmltopdf-p<N>.png (96 dpi, same as the Chrome reference).
#   scripts/render-fixtures.sh [--update-goldens] [fixture ...]
set -euo pipefail
cd "$(dirname "$0")/.."
BIN=${HTMLTOPDF_BIN:-target/release/htmltopdf}
[ -x "$BIN" ] || { echo "build first: cargo build --release -p htmltopdf-cli" >&2; exit 1; }
[ -f fixtures/fonts/Inter-Regular.ttf ] || scripts/fetch-fonts.sh

update=0
if [ "${1:-}" = "--update-goldens" ]; then update=1; shift; fi
names=("$@")
[ ${#names[@]} -eq 0 ] && mapfile -t names < <(ls fixtures/invoices)

FONTS=(); for f in fixtures/fonts/Inter-*.ttf; do FONTS+=(--font "$f"); done

for n in "${names[@]}"; do
  dir="fixtures/invoices/$n"; [ -f "$dir/index.html" ] || continue
  out="out/$n"; mkdir -p "$out"
  IMAGES=()
  for img in "$dir"/*.png "$dir"/*.jpg; do [ -f "$img" ] && IMAGES+=(--image "$(basename "$img")=$img"); done
  "$BIN" render "$dir/index.html" -o "$out/htmltopdf.pdf" \
      --css fixtures/tailwind/tailwind.css "${FONTS[@]}" "${IMAGES[@]}" \
      --title "$n" --lang cs --strict-fonts --strict-images
  rm -f "$out"/htmltopdf-p*.png
  pdftoppm -r 96 -png "$out/htmltopdf.pdf" "$out/htmltopdf-p"
  if [ $update = 1 ]; then
    mkdir -p "$dir/golden"; cp "$out/htmltopdf.pdf" "$dir/golden/htmltopdf.pdf"; cp "$out"/htmltopdf-p*.png "$dir/golden/"
    echo "  golden updated: $dir/golden"
  fi
done
