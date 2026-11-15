#!/bin/bash
set -euo pipefail
export CC=clang CXX=clang++ AR=llvm-ar RANLIB=llvm-ranlib
apt-get update
apt-get install -y --no-install-recommends libatk1.0-dev libatk-bridge2.0-dev libspeechd-dev
mkdir -p /tmp/tk92
cd /tmp/tk92
curl -fsSL --retry 3 https://gitlab.freedesktop.org/libdecor/libdecor/-/archive/0.2.2/libdecor-0.2.2.tar.gz | tar xz
meson setup libdecor-build libdecor-0.2.2 --prefix=/opt/bb-deps --libdir=lib -Ddemo=false -Dgtk=disabled
meson compile -C libdecor-build -j8
meson install -C libdecor-build
curl -fsSL --retry 3 https://codeload.github.com/glfw/glfw/tar.gz/refs/tags/3.4 | tar xz
cmake -S glfw-3.4 -B glfw-build -DCMAKE_INSTALL_PREFIX=/opt/bb-deps -DCMAKE_INSTALL_LIBDIR=lib -DBUILD_SHARED_LIBS=ON -DGLFW_BUILD_WAYLAND=ON -DGLFW_BUILD_X11=OFF -DGLFW_BUILD_EXAMPLES=OFF -DGLFW_BUILD_TESTS=OFF -DGLFW_BUILD_DOCS=OFF
cmake --build glfw-build -j8
cmake --install glfw-build
curl -fsSL --retry 3 https://github.com/harfbuzz/harfbuzz/releases/download/8.5.0/harfbuzz-8.5.0.tar.xz | tar xJ
meson setup harfbuzz-build harfbuzz-8.5.0 --prefix=/opt/bb-deps --libdir=lib -Dtests=disabled -Ddocs=disabled -Dutilities=disabled
meson compile -C harfbuzz-build -j8
meson install -C harfbuzz-build
