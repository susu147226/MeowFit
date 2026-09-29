#!/usr/bin/env bash
# ============================================================
# 喵尺 MeowFit — GPL 版 FFmpeg 源码构建脚本
#
# 用途：产出自带全部所需组件的 ffmpeg.exe / ffprobe.exe，满足规范 12.5 的自检：
#   - 编码器 libx264 / libx265 / libvpx-vp9 / libaom-av1
#   - 滤镜   palettegen / paletteuse / zscale / tonemap
#   - 解码器 hevc
#
# 运行环境：MSYS2 的 MINGW64 shell（不是 Git Bash，也不是 CMD/PowerShell）
#   1. 安装 MSYS2：https://www.msys2.org/
#   2. 打开「MSYS2 MINGW64」
#   3. pacman -S --needed base-devel mingw-w64-x86_64-toolchain \
#        mingw-w64-x86_64-nasm mingw-w64-x86_64-cmake \
#        mingw-w64-x86_64-meson mingw-w64-x86_64-ninja \
#        mingw-w64-x86_64-pkgconf mingw-w64-x86_64-yasm
#   4. bash scripts/build-ffmpeg-gpl.sh
#
# 产物：build/ffmpeg-gpl/win-x64/{ffmpeg.exe,ffprobe.exe}
#       复制到 src-tauri/resources/ffmpeg/win-x64/ 即可
#
# 注意：**本脚本尚未在作者机器上实际跑通**（该机器没有任何 C 构建工具链）。
# 首次运行大概率需要按报错微调依赖版本，请把失败信息发回。
# ============================================================

set -euo pipefail

# ---------- 版本（GPL 源码获取途径就在这些地址）----------
FFMPEG_VER="7.1"
X264_REV="b35605ace3ddf7c1a5d67a2eb553f034aef41d55"   # x264 无版本号，用提交
X265_VER="4.1"
VPX_VER="1.15.0"
AOM_VER="3.11.0"
ZIMG_VER="3.0.5"
WEBP_VER="1.5.0"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="${ROOT}/build/ffmpeg-gpl"
PREFIX="${WORK}/prefix"
SRC="${WORK}/src"
OUT="${WORK}/win-x64"
JOBS="$(nproc)"

mkdir -p "${PREFIX}" "${SRC}" "${OUT}"
cd "${SRC}"

fetch() { # fetch <url> <tarball>
  [ -f "$2" ] || curl -fL --retry 3 -o "$2" "$1"
}

echo "==> 工具链检查"
for tool in gcc make pkg-config nasm cmake meson ninja; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "缺少 $tool。请先按脚本头部说明安装 MSYS2 MINGW64 依赖。" >&2
    exit 1
  }
done

export PKG_CONFIG_PATH="${PREFIX}/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
CFG=(--prefix="${PREFIX}" --enable-static --disable-shared --disable-debug)

# ---------- 1. zimg（zscale 滤镜依赖）----------
if [ ! -f "${PREFIX}/lib/libzimg.a" ]; then
  echo "==> 构建 zimg ${ZIMG_VER}"
  fetch "https://github.com/sekrit-twc/zimg/archive/refs/tags/release-${ZIMG_VER}.tar.gz" "zimg.tar.gz"
  rm -rf "zimg-release-${ZIMG_VER}" && tar xf zimg.tar.gz
  ( cd "zimg-release-${ZIMG_VER}" && ./autogen.sh && ./configure "${CFG[@]}" && make -j"${JOBS}" && make install )
fi

# ---------- 2. x264 ----------
if [ ! -f "${PREFIX}/lib/libx264.a" ]; then
  echo "==> 构建 x264 ${X264_REV:0:8}"
  fetch "https://code.videolan.org/videolan/x264/-/archive/${X264_REV}/x264-${X264_REV}.tar.gz" "x264.tar.gz"
  rm -rf "x264-${X264_REV}" && tar xf x264.tar.gz
  ( cd "x264-${X264_REV}" && ./configure --prefix="${PREFIX}" --enable-static --disable-cli && make -j"${JOBS}" && make install )
fi

# ---------- 3. x265 ----------
if [ ! -f "${PREFIX}/lib/libx265.a" ]; then
  echo "==> 构建 x265 ${X265_VER}"
  fetch "https://bitbucket.org/multicoreware/x265_git/downloads/x265_${X265_VER}.tar.gz" "x265.tar.gz"
  rm -rf "x265_${X265_VER}" && tar xf x265.tar.gz
  ( cd "x265_${X265_VER}/source" && cmake -G Ninja -DCMAKE_INSTALL_PREFIX="${PREFIX}" \
      -DHIGH_BIT_DEPTH=OFF -DENABLE_CLI=OFF -DENABLE_SHARED=OFF . && ninja && ninja install )
fi

# ---------- 4. libvpx（VP9）----------
if [ ! -f "${PREFIX}/lib/libvpx.a" ]; then
  echo "==> 构建 libvpx ${VPX_VER}"
  fetch "https://github.com/webmproject/libvpx/archive/refs/tags/v${VPX_VER}.tar.gz" "libvpx.tar.gz"
  rm -rf "libvpx-${VPX_VER}" && tar xf libvpx.tar.gz
  ( cd "libvpx-${VPX_VER}" && ./configure --prefix="${PREFIX}" --target=x86_64-win64-gcc \
      --enable-static --disable-shared --disable-examples --disable-unit-tests \
      --disable-tools --disable-docs && make -j"${JOBS}" && make install )
fi

# ---------- 5. libaom（AV1）----------
if [ ! -f "${PREFIX}/lib/libaom.a" ]; then
  echo "==> 构建 libaom ${AOM_VER}（这一步很慢）"
  fetch "https://aomedia.googlesource.com/aom/+archive/v${AOM_VER}.tar.gz" "aom.tar.gz"
  rm -rf aom && mkdir aom && tar xf aom.tar.gz -C aom
  ( cd aom && mkdir -p build && cd build && cmake -G Ninja \
      -DCMAKE_INSTALL_PREFIX="${PREFIX}" -DBUILD_SHARED_LIBS=0 \
      -DENABLE_EXAMPLES=0 -DENABLE_TESTS=0 -DENABLE_DOCS=0 \
      -DCONFIG_AV1_ENCODER=1 -DCONFIG_AV1_HIGHBITDEPTH=1 .. && ninja && ninja install )
fi

# ---------- 6. libwebp（动图 WebP，P4 需要）----------
if [ ! -f "${PREFIX}/lib/libwebp.a" ]; then
  echo "==> 构建 libwebp ${WEBP_VER}"
  fetch "https://storage.googleapis.com/downloads.webmproject.org/releases/webp/libwebp-${WEBP_VER}.tar.gz" "libwebp.tar.gz"
  rm -rf "libwebp-${WEBP_VER}" && tar xf libwebp.tar.gz
  ( cd "libwebp-${WEBP_VER}" && ./configure "${CFG[@]}" --enable-libwebpmux --disable-libwebpextras \
      && make -j"${JOBS}" && make install )
fi

# ---------- 7. FFmpeg ----------
echo "==> 构建 FFmpeg ${FFMPEG_VER}"
fetch "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VER}.tar.xz" "ffmpeg.tar.xz"
rm -rf "ffmpeg-${FFMPEG_VER}" && tar xf ffmpeg.tar.xz
( cd "ffmpeg-${FFMPEG_VER}" && ./configure \
    --prefix="${PREFIX}" \
    --pkg-config-flags=--static \
    --extra-cflags="-I${PREFIX}/include" \
    --extra-ldflags="-L${PREFIX}/lib" \
    --enable-gpl \
    --enable-libx264 --enable-libx265 --enable-libvpx --enable-libaom \
    --enable-libzimg --enable-libwebp \
    --disable-doc --disable-debug --disable-network \
    --enable-static --disable-shared \
  && make -j"${JOBS}" \
  && make install )

cp "${PREFIX}/bin/ffmpeg.exe" "${PREFIX}/bin/ffprobe.exe" "${OUT}/"

echo
echo "==> 完成：${OUT}"
echo "请复制到 src-tauri/resources/ffmpeg/win-x64/，然后运行自检："
"${OUT}/ffmpeg.exe" -hide_banner -encoders | grep -E "libx264|libx265|libvpx-vp9|libaom-av1" || true
"${OUT}/ffmpeg.exe" -hide_banner -filters  | grep -E "palettegen|paletteuse|zscale|tonemap"        || true
"${OUT}/ffmpeg.exe" -hide_banner -decoders | grep -E "hevc"                                        || true

cat <<'NOTE'

GPL 义务提醒（规范 3.2）：随包分发这份 FFmpeg 时，必须一并提供其完整源码获取途径。
本项目所用各组件的源码地址即上面 fetch 用到的那些固定版本地址，请把它们填进
README 的「许可与致谢」第二段，替换占位符：
  【构建时填入所采用的上游发行地址】
NOTE
