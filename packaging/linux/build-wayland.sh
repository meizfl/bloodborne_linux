#!/bin/bash
set -euo pipefail
python -m pip install --no-cache-dir meson==1.7.0
cd /tmp/bb-deps
curl -fsSL --retry 3 https://gitlab.freedesktop.org/wayland/wayland/-/releases/1.24.0/downloads/wayland-1.24.0.tar.xz -o wayland.tar.xz
mkdir -p wayland
tar -xf wayland.tar.xz -C wayland --strip-components=1
meson setup wayland/build wayland --prefix=/opt/bb-deps --libdir=lib \
    -Ddocumentation=false -Dtests=false -Ddtd_validation=false
meson compile -C wayland/build -j8
meson install -C wayland/build
