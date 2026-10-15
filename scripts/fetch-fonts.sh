#!/usr/bin/env bash
# Fetches the fonts the fixtures use into fixtures/fonts/ (git-ignored).
# Inter (SIL OFL 1.1) from the official GitHub release; static TTFs, not the variable font,
# because the PDF subsetter handles static instances most predictably.
set -euo pipefail
cd "$(dirname "$0")/../fixtures/fonts"
VER="4.1"
ZIP="Inter-${VER}.zip"
if [ ! -f Inter-Regular.ttf ]; then
  echo "downloading Inter ${VER}…"
  curl -fsSL -o "$ZIP" "https://github.com/rsms/inter/releases/download/v${VER}/${ZIP}"
  unzip -q -o -j "$ZIP" "extras/ttf/Inter-Regular.ttf" "extras/ttf/Inter-Medium.ttf" "extras/ttf/Inter-SemiBold.ttf" "extras/ttf/Inter-Bold.ttf" "extras/ttf/Inter-ExtraBold.ttf"
  rm -f "$ZIP"
fi
ls -l *.ttf
