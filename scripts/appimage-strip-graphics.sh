#!/usr/bin/env bash
# Tauri's AppImage bundler (linuxdeploy-plugin-gtk) copies Ubuntu-era graphics
# libs into the AppDir and force-prepends them via LD_LIBRARY_PATH. On non-Debian
# hosts the bundled libwayland-client shadows the host's newer copy and breaks
# Mesa driver loading, so the WebView aborts on EGL init (EGL_BAD_ALLOC). The
# graphics stack must come from the host, so this removes the offending libs,
# following the standard AppImage excludelist.
set -euo pipefail

appimage_path="${1:?usage: appimage-strip-graphics.sh <path-to.AppImage>}"
appimage_path="$(readlink -f "$appimage_path")"
work_dir="$(dirname "$appimage_path")"

# libwayland-* is the confirmed culprit; the rest are graphics/driver-adjacent
# libs from the AppImage excludelist that are unsafe to ship pinned.
strip_globs=(
  'libwayland-client.so*'
  'libwayland-server.so*'
  'libwayland-egl.so*'
  'libwayland-cursor.so*'
  'libGL.so*'
  'libEGL.so*'
  'libGLdispatch.so*'
  'libGLX.so*'
  'libdrm.so*'
  'libgbm.so*'
)

cd "$work_dir"
rm -rf squashfs-root
# --appimage-extract works without FUSE, so this is CI/container-safe.
"$appimage_path" --appimage-extract >/dev/null

lib_dir="squashfs-root/usr/lib"
removed=0
for glob in "${strip_globs[@]}"; do
  while IFS= read -r -d '' lib; do
    echo "stripping bundled $(basename "$lib")"
    rm -f "$lib"
    removed=$((removed + 1))
  done < <(find "$lib_dir" -type f -name "$glob" -print0 2>/dev/null)
done
echo "stripped $removed host-graphics lib(s) from AppDir"

# extract-and-run avoids the FUSE requirement.
tool_dir="$(mktemp -d)"
appimagetool="$tool_dir/appimagetool-x86_64.AppImage"
wget -qO "$appimagetool" \
  "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
chmod +x "$appimagetool"

ARCH=x86_64 "$appimagetool" --appimage-extract-and-run \
  squashfs-root "$appimage_path"

rm -rf squashfs-root "$tool_dir"
echo "repackaged $appimage_path without host-graphics libs"
