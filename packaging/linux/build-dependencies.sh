#!/bin/bash
set -euo pipefail
mkdir -p /opt/gcc /opt/bb-deps /tmp/bb-deps
cd /tmp/bb-deps
if [[ ${1:-all} == gcc || ${1:-all} == all ]]; then
curl -fL --retry 3 https://github.com/xpack-dev-tools/gcc-xpack/releases/download/v14.2.0-2/xpack-gcc-14.2.0-2-linux-x64.tar.gz -o gcc.tar.gz
echo 'd27b09c677cefde24bdaacdde045d35ea1a391fbb5b4525d057e1974af912c4d  gcc.tar.gz' | sha256sum -c -
tar -xf gcc.tar.gz -C /opt/gcc --strip-components=1
fi
export PATH=/opt/gcc/bin:$PATH
export LD_LIBRARY_PATH=/opt/gcc/lib64
export CC=gcc CXX=g++ CMAKE_PREFIX_PATH=/opt/bb-deps PKG_CONFIG_PATH=/opt/bb-deps/lib/pkgconfig
build() {
    local name=$1 url=$2
    shift 2
    curl -fL --retry 3 "$url" -o "$name.tar.gz"
    mkdir -p "$name"
    tar -xf "$name.tar.gz" -C "$name" --strip-components=1
    local source=$name
    [[ $name != miniupnpc ]] || source=$name/miniupnpc
    cmake -S "$source" -B "$name/build" -G Ninja -DCMAKE_BUILD_TYPE=Release \
        -DCMAKE_INSTALL_PREFIX=/opt/bb-deps -DCMAKE_INSTALL_LIBDIR=lib \
        -DCMAKE_POSITION_INDEPENDENT_CODE=ON "$@"
    cmake --build "$name/build" -j4
    cmake --install "$name/build"
}
if [[ ${1:-all} == fmt || ${1:-all} == all ]]; then
build fmt https://github.com/fmtlib/fmt/archive/refs/tags/11.1.4.tar.gz -DFMT_TEST=OFF
fi
if [[ ${1:-all} == robin || ${1:-all} == all ]]; then
build robin https://github.com/Tessil/robin-map/archive/refs/tags/v1.3.0.tar.gz
fi
if [[ ${1:-all} == vulkan-headers || ${1:-all} == all ]]; then
build vulkan-headers https://github.com/KhronosGroup/Vulkan-Headers/archive/refs/tags/v1.4.304.tar.gz
fi
if [[ ${1:-all} == vma || ${1:-all} == all ]]; then
build vma https://github.com/GPUOpen-LibrariesAndSDKs/VulkanMemoryAllocator/archive/refs/tags/v3.2.1.tar.gz
fi
if [[ ${1:-all} == zycore || ${1:-all} == all ]]; then
build zycore https://github.com/zyantific/zycore-c/archive/refs/tags/v1.5.2.tar.gz -DZYCORE_BUILD_TESTS=OFF
fi
if [[ ${1:-all} == zydis || ${1:-all} == all ]]; then
build zydis https://github.com/zyantific/zydis/archive/refs/tags/v4.1.1.tar.gz -DZYAN_SYSTEM_ZYCORE=ON -DZYCORE_INCLUDE_DIR=/opt/bb-deps/include -DZYDIS_BUILD_TOOLS=OFF -DZYDIS_BUILD_EXAMPLES=OFF
fi
if [[ ${1:-all} == SDL || ${1:-all} == all ]]; then
build SDL https://github.com/libsdl-org/SDL/archive/refs/tags/release-3.2.10.tar.gz -DSDL_TESTS=OFF -DSDL_STATIC=OFF
fi
if [[ ${1:-all} == glslang || ${1:-all} == all ]]; then
build glslang https://github.com/KhronosGroup/glslang/archive/refs/tags/15.1.0.tar.gz -DENABLE_OPT=OFF -DBUILD_TESTING=OFF
fi
if [[ ${1:-all} == ffmpeg || ${1:-all} == all ]]; then
curl -fL --retry 3 https://ffmpeg.org/releases/ffmpeg-7.1.1.tar.xz -o ffmpeg.tar.xz
mkdir ffmpeg
tar -xf ffmpeg.tar.xz -C ffmpeg --strip-components=1
cd ffmpeg
./configure --prefix=/opt/bb-deps --enable-shared --disable-static --disable-programs --disable-doc \
    --disable-debug --disable-autodetect --disable-everything --enable-decoder=h264,aac \
    --enable-demuxer=mov --enable-parser=h264,aac --enable-protocol=file --enable-pic
make -j4
make install
cd /
fi

if [[ ${1:-all} == protobuf || ${1:-all} == all ]]; then
build protobuf https://github.com/protocolbuffers/protobuf/archive/refs/tags/v21.12.tar.gz -Dprotobuf_BUILD_TESTS=OFF -Dprotobuf_BUILD_SHARED_LIBS=ON
fi
if [[ ${1:-all} == miniupnpc || ${1:-all} == all ]]; then
build miniupnpc https://github.com/miniupnp/miniupnp/archive/refs/tags/miniupnpc_2_3_3.tar.gz -DUPNPC_BUILD_TESTS=OFF
fi
if [[ ${1:-all} == vulkan-loader || ${1:-all} == all ]]; then
build vulkan-loader https://github.com/KhronosGroup/Vulkan-Loader/archive/refs/tags/v1.4.304.tar.gz -DBUILD_TESTS=OFF -DBUILD_WSI_XCB_SUPPORT=OFF
fi
