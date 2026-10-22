#!/usr/bin/env bash
set -euo pipefail

# Regeneration reference for the application icon.
# The committed PNG/ICO files make normal builds independent of these tools.
# Tools: rsvg-convert (librsvg), inkscape, or imagemagick as a fallback.

SOURCE="assets/icons/pdfmerge.svg"
OUTPUT_DIR="assets/icons"

mkdir -p "$OUTPUT_DIR"

if ! command -v rsvg-convert >/dev/null 2>&1; then
  echo "rsvg-convert est requis pour générer les PNG" >&2
  exit 1
fi

for SIZE in 16 32 48 64 128 256 512; do
  rsvg-convert -w "$SIZE" -h "$SIZE" "$SOURCE" -o "$OUTPUT_DIR/pdfmerge-${SIZE}.png"
done

if command -v magick >/dev/null 2>&1; then
  magick "$OUTPUT_DIR/pdfmerge-256.png" \
    \( "$OUTPUT_DIR/pdfmerge-48.png" \) \
    \( "$OUTPUT_DIR/pdfmerge-32.png" \) \
    \( "$OUTPUT_DIR/pdfmerge-16.png" \) \
    "$OUTPUT_DIR/pdfmerge.ico"
elif command -v python3 >/dev/null 2>&1 && python3 -c "import PIL" 2>/dev/null; then
  python3 - "$OUTPUT_DIR" <<'PY'
import os
import sys

from PIL import Image

output_dir = sys.argv[1]
base = Image.open(os.path.join(output_dir, "pdfmerge-256.png")).convert("RGBA")
sizes = (16, 32, 48, 64, 128, 256)
base.save(
    os.path.join(output_dir, "pdfmerge.ico"),
    format="ICO",
    sizes=[(size, size) for size in sizes],
)
PY
else
  echo "magick ou python3 avec Pillow est requis pour générer le fichier ICO" >&2
  exit 1
fi