#!/bin/sh
# Run from the FlipCTL checkout. No network downloads or core/service changes.
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repo"
test "$(uname -m)" = aarch64 || { echo 'Build/package on ARM64' >&2; exit 1; }
for name in appimagetool-aarch64 runtime-aarch64; do
    checksum=$(awk -v name="$name" '$1 == name {print $3}' tools/appimage/tools.lock)
    test -n "$checksum"
    printf '%s  %s\n' "$checksum" "target/appimage/tools/$name" | sha256sum -c -
done
binary="$repo/target/release/wifi-analyzer"
test -x "$binary"
stage="$repo/target/appimage/wifi-analyzer/AppDir"
output="$repo/target/appimage/wifi-analyzer-aarch64.fap.AppImage"
test ! -e "$output" || { echo "Output exists: $output; move it aside before rebuilding" >&2; exit 1; }
python3 tools/appimage/bundle.py stage apps/wifi-analyzer "$stage" \
    --binary "$binary" --version 0.1.0-bookworm --repo "$repo"
SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct 2>/dev/null || printf '0')} ARCH=aarch64 \
  target/appimage/tools/appimagetool-aarch64 --appimage-extract-and-run \
    -n --comp zstd --mksquashfs-opt -Xcompression-level --mksquashfs-opt 19 \
    --runtime-file "$repo/target/appimage/tools/runtime-aarch64" "$stage" "$output"
sha256sum "$output"
