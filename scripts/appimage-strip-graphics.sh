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
# libs from the AppImage excludelist (glvnd, Mesa, and the DRI xcb protocol
# libs) that are unsafe to ship pinned.
strip_globs=(
  'libwayland-client.so*'
  'libwayland-server.so*'
  'libwayland-egl.so*'
  'libwayland-cursor.so*'
  'libGL.so*'
  'libEGL.so*'
  'libGLdispatch.so*'
  'libGLX.so*'
  'libOpenGL.so*'
  'libdrm.so*'
  'libglapi.so*'
  'libgbm.so*'
  'libxcb-dri2.so*'
  'libxcb-dri3.so*'
)

# One find(1) clause per glob, joined with -o, so the strip loop and the
# verification below can never drift apart.
name_clauses=()
for glob in "${strip_globs[@]}"; do
  name_clauses+=(-name "$glob" -o)
done
unset 'name_clauses[-1]'

cd "$work_dir"
rm -rf squashfs-root
# --appimage-extract works without FUSE, so this is CI/container-safe.
"$appimage_path" --appimage-extract >/dev/null

lib_dir="squashfs-root/usr/lib"
removed=0
while IFS= read -r -d '' lib; do
  echo "stripping bundled $(basename "$lib")"
  rm -f "$lib"
  removed=$((removed + 1))
done < <(find "$lib_dir" -type f \( "${name_clauses[@]}" \) -print0 2>/dev/null)
echo "stripped $removed host-graphics lib(s) from AppDir"

# Guard the workaround itself: any surviving match anywhere in the AppDir —
# not just usr/lib; a bundler layout change would otherwise silently reship
# the EGL_BAD_ALLOC crash — must fail the build loudly.
leftovers=$(find squashfs-root -type f \( "${name_clauses[@]}" \) -print)
if [ -n "$leftovers" ]; then
  echo "ERROR: host-graphics libs still present after stripping:" >&2
  echo "$leftovers" >&2
  exit 1
fi

# extract-and-run avoids the FUSE requirement.
tool_dir="$(mktemp -d)"
appimagetool="$tool_dir/appimagetool-x86_64.AppImage"
wget -qO "$appimagetool" \
  "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
chmod +x "$appimagetool"

# The appimagetool source dir must be ABSOLUTE: the current continuous build
# resolves relative paths against its own --appimage-extract-and-run temp dir,
# not the caller's CWD, and fails with "no such file or directory: squashfs-root".
ARCH=x86_64 "$appimagetool" --appimage-extract-and-run \
  "$work_dir/squashfs-root" "$appimage_path"

rm -rf squashfs-root "$tool_dir"
echo "repackaged $appimage_path without host-graphics libs"
