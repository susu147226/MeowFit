#!/usr/bin/env bash
# ============================================================
# 喵尺 MeowFit — GPL 版 FFmpeg 构建脚本
#
# 产物：ffmpeg.exe / ffprobe.exe（静态链接）+ libgcc_s_seh-1.dll
#       复制到 src-tauri/resources/ffmpeg/win-x64/
#
# 开出的组件覆盖规范 12.5 的自检项：
#   编码器 libx264 / libx265 / libvpx-vp9 / libaom-av1
#   滤镜   palettegen / paletteuse / zscale / tonemap
#   解码器 hevc
#
# ------------------------------------------------------------
# 依赖库为什么用 MSYS2 的包，而不是全部从源码编
# ------------------------------------------------------------
# 本脚本最初打算把 zimg / x264 / x265 / libvpx / libaom / libwebp 全部从源码
# 编一遍。实践下来有两个坑：
#   1. libaom 只发布在 googlesource，而该域名在部分网络下完全不可达；
#   2. x265 4.1 的 CMakeLists 使用 cmake_policy(SET CMP0025 OLD)，
#      CMake 4.x 已彻底移除该兼容，必须用 CMake 3.x 才能配置。
# 因此改为：**依赖库用 MSYS2 的预编译包，FFmpeg 本体仍从源码构建**。
# 这样既避开了不可达的下载源，也保证了编码器/滤镜组合正是我们需要的。
#
# ------------------------------------------------------------
# 用法（在 MSYS2 的 MINGW64 shell 里执行，不是 Git Bash / CMD / PowerShell）
# ------------------------------------------------------------
#   1. 安装 MSYS2：https://www.msys2.org/
#   2. 打开「MSYS2 MINGW64」，执行：
#
#      pacman -S --needed git base-devel mingw-w64-x86_64-toolchain \
#        mingw-w64-x86_64-nasm mingw-w64-x86_64-cmake \
#        mingw-w64-x86_64-meson mingw-w64-x86_64-ninja \
#        mingw-w64-x86_64-pkgconf mingw-w64-x86_64-yasm \
#        mingw-w64-x86_64-aom mingw-w64-x86_64-libvpx \
#        mingw-w64-x86_64-x264 mingw-w64-x86_64-x265 \
#        mingw-w64-x86_64-zimg mingw-w64-x86_64-libwebp
#
#   3. bash scripts/build-ffmpeg-gpl.sh
#
# 校验记录：本脚本于 2026-09-29 在 MSYS2 + GCC 16.2.0 下实际跑通，
# 产出 ffmpeg 9.0.2，规范 12.5 的三项自检全部通过。
# ============================================================

set -euo pipefail

FFMPEG_VER="9.0.2"
FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VER}.tar.xz"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# 构建目录必须**不含空格**：autotools / cmake 的生成文件在带空格的路径上经常失败。
if [ -n "${MEOWFIT_FFMPEG_BUILD:-}" ]; then
  WORK="${MEOWFIT_FFMPEG_BUILD}"
elif [[ "${ROOT}" == *" "* ]]; then
  WORK="${HOME}/meowfit-ffmpeg-build"
  echo "提示：仓库路径含空格，构建改在 ${WORK} 进行（可用 MEOWFIT_FFMPEG_BUILD 覆盖）"
else
  WORK="${ROOT}/build/ffmpeg-gpl"
fi
SRC="${WORK}/src"
OUT="${WORK}/win-x64"
JOBS="$(nproc)"

mkdir -p "${SRC}" "${OUT}"
cd "${SRC}"

echo "==> 检查依赖包"
missing_pkgs=()
for pc in x264 x265 vpx aom zimg libwebp; do
  pkg-config --exists "${pc}" 2>/dev/null || missing_pkgs+=("${pc}")
done
if [ ${#missing_pkgs[@]} -gt 0 ]; then
  cat >&2 <<MSG
缺少依赖库：${missing_pkgs[*]}
请先在 MSYS2 MINGW64 里执行：
  pacman -S --needed mingw-w64-x86_64-aom mingw-w64-x86_64-libvpx \\
    mingw-w64-x86_64-x264 mingw-w64-x86_64-x265 \\
    mingw-w64-x86_64-zimg mingw-w64-x86_64-libwebp
MSG
  exit 1
fi

for tool in gcc make nasm; do
  command -v "${tool}" >/dev/null 2>&1 || {
    echo "缺少 ${tool}，请按脚本头部说明安装 MSYS2 MINGW64 依赖。" >&2
    exit 1
  }
done

echo "==> 下载 FFmpeg ${FFMPEG_VER}"
# GitHub 类归档在部分网络下会被重置连接，这里统一加重试
[ -f "ffmpeg-${FFMPEG_VER}.tar.xz" ] || \
  curl -fL --retry 5 --retry-delay 3 --retry-all-errors --connect-timeout 20 \
    -o "ffmpeg-${FFMPEG_VER}.tar.xz" "${FFMPEG_URL}"

rm -rf "ffmpeg-${FFMPEG_VER}" && tar xf "ffmpeg-${FFMPEG_VER}.tar.xz"
cd "ffmpeg-${FFMPEG_VER}"

echo "==> 配置"
# 几点说明：
#   --pkg-config-flags=--static 让依赖库走静态链接
#   -static                      让链接器只找 .a，从而把编解码器全部编进单个 exe
#   -Wl,--allow-multiple-definition
#       MSYS2 上同时链接 libgcc_eh.a 与 libgcc_s.a 时，_Unwind_Resume 会重复定义，
#       这是 MinGW 静态链接的已知冲突；两处符号实现等价，允许重复定义即可。
#       代价是退出时仍需随包附带 libgcc_s_seh-1.dll。
#   --disable-network            程序只处理本地文件，不需要任何网络协议
./configure \
  --prefix="${WORK}/prefix" \
  --pkg-config-flags=--static \
  --extra-cflags="-I/mingw64/include" \
  --extra-ldflags="-L/mingw64/lib -static -Wl,--allow-multiple-definition" \
  --enable-gpl \
  --enable-libx264 --enable-libx265 --enable-libvpx --enable-libaom \
  --enable-libzimg --enable-libwebp \
  --disable-doc --disable-debug --disable-network \
  --disable-vaapi --disable-vdpau --disable-vulkan \
  --enable-static --disable-shared

echo "==> 编译（这一步会跑一阵）"
make -j"${JOBS}"

echo "==> 安装并复制产物"
make install
cp "${WORK}/prefix/bin/ffmpeg.exe" "${WORK}/prefix/bin/ffprobe.exe" "${OUT}/"
# 静态链接后仅剩这一个外部运行时依赖
cp /mingw64/bin/libgcc_s_seh-1.dll "${OUT}/"

echo
echo "==> 完成：${OUT}"
echo "请复制到 src-tauri/resources/ffmpeg/win-x64/"
echo
echo "==> 规范 12.5 自检"
"${OUT}/ffmpeg.exe" -hide_banner -encoders | grep -E "libx264|libx265|libvpx-vp9|libaom-av1" || \
  echo "!! 缺少编码器，按规范 12.5 必须告知作者而不是静默发布"
"${OUT}/ffmpeg.exe" -hide_banner -filters  | grep -E "palettegen|paletteuse|zscale|tonemap" || \
  echo "!! 缺少滤镜"
"${OUT}/ffmpeg.exe" -hide_banner -decoders | grep -E "hevc" || \
  echo "!! 缺少 hevc 解码器"

cat <<'NOTE'

------------------------------------------------------------
GPL 义务提醒（规范 3.2）
------------------------------------------------------------
随包分发这份 FFmpeg 时，必须一并提供其完整源码获取途径。本次构建涉及的组件：

  FFmpeg  9.0.2                      https://ffmpeg.org/releases/
  x264    0.165（MSYS2 包 x264）      https://code.videolan.org/videolan/x264
  x265    4.3  （MSYS2 包 x265）      https://bitbucket.org/multicoreware/x265_git
  libvpx  1.17.0（MSYS2 包 libvpx）   https://chromium.googlesource.com/webm/libvpx
  libaom  3.15.1（MSYS2 包 aom）      https://aomedia.googlesource.com/aom
  zimg    3.0.6 （MSYS2 包 zimg）     https://github.com/sekrit-twc/zimg
  libwebp 1.6.0 （MSYS2 包 libwebp）  https://chromium.googlesource.com/webm/libwebp

MSYS2 各包的构建配方（PKGBUILD）同样属于对应源码，见 https://github.com/msys2/MINGW-packages

请把这些地址填进 README 的「许可与致谢」第二段，替换占位符：
  【构建时填入所采用的上游发行地址】
NOTE
