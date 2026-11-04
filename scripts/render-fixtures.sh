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

# A fixture listed in fixtures/glyph-gaps.txt (F101) has a character no bundled font maps, so it is rendered without
# --strict-fonts (the PDF bytes are the same either way). Every other fixture is strict.
gap_listed() { grep -Eq "^$1([[:space:]]|\$)" fixtures/glyph-gaps.txt; }

for n in "${names[@]}"; do
  dir="fixtures/invoices/$n"; [ -f "$dir/index.html" ] || continue
  out="out/$n"; mkdir -p "$out"
  IMAGES=()
  for img in "$dir"/*.png "$dir"/*.jpg; do [ -f "$img" ] && IMAGES+=(--image "$(basename "$img")=$img"); done
  # A fixture gets the compiled Tailwind sheet only when its <link> names it (27 is plain CSS: no preflight).
  CSS=(); grep -q 'tailwind\.css' "$dir/index.html" && CSS=(--css fixtures/tailwind/tailwind.css)
  STRICT=(--strict-fonts); gap_listed "$n" && STRICT=()
  "$BIN" render "$dir/index.html" -o "$out/htmltopdf.pdf" \
      "${CSS[@]}" "${FONTS[@]}" "${IMAGES[@]}" \
      --title "$n" --lang cs "${STRICT[@]}" --strict-images
  rm -f "$out"/htmltopdf-p*.png
  pdftoppm -r 96 -png "$out/htmltopdf.pdf" "$out/htmltopdf-p"
  if [ $update = 1 ]; then
    mkdir -p "$dir/golden"; cp "$out/htmltopdf.pdf" "$dir/golden/htmltopdf.pdf"; cp "$out"/htmltopdf-p*.png "$dir/golden/"
    echo "  golden updated: $dir/golden"
  fi
done
