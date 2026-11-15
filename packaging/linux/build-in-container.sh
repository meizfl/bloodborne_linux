#!/bin/bash
# Invoked by Podman, never needed by the installed launcher.
set -euo pipefail
mkdir -p /build
python - <<'COPY'
from pathlib import Path
import shutil
source = Path('/src')
def ignored(directory, names):
    excluded = {'.git', '__pycache__', '.venv'}
    if Path(directory) == source:
        excluded |= {'out', 'dist', 'Game', 'user', 'game_runtime'}
    return set(names) & excluded
shutil.copytree(source, '/build', dirs_exist_ok=True, ignore=ignored)
COPY
cd /build
# Source archive builds also work without git metadata. Applied vendor patches are idempotent.
for change in gpu/patches/fsr-vulkan/*.patch; do
    if ! patch --dry-run -R -p1 -d gpu/third_party/fsr-vulkan < "$change" >/dev/null 2>&1; then
        patch -p1 -d gpu/third_party/fsr-vulkan < "$change"
    fi
done
# build.sh's patch loop uses git apply; a tiny local repository records this source snapshot.
git init -q
if [[ -f gpu/third_party/fsr-vulkan/.git ]]; then rm gpu/third_party/fsr-vulkan/.git; fi
git -C gpu/third_party/fsr-vulkan init -q
export CC=clang CXX=clang++ BB_CXX_RUNTIME=c++ BB_PGO=off BB_LTO=OFF CMAKE_BUILD_PARALLEL_LEVEL=${JOBS:-4}
# Header timestamps in release archives can predate cached .o files. A new ABI
# dependency set must invalidate the whole GPU build, not just CMake's cache.
environment=$({
    if [[ -n ${BB_BUILD_IMAGE:-} ]]; then printf '%s\n' "$BB_BUILD_IMAGE"; fi
    clang++ --version
    sha256sum /opt/bb-deps/include/vulkan/vulkan_core.h \
        /opt/bb-deps/include/boost/version.hpp /opt/bb-deps/include/openssl/opensslv.h \
        /opt/bb-deps/include/SDL3/SDL_version.h
} | sha256sum | cut -d' ' -f1)
if [[ ! -f out/.build-environment || $(cat out/.build-environment) != "$environment" ]]; then
    rm -rf out/gpu out/atrac9 out/libatrac9.a
    mkdir -p out
    printf '%s\n' "$environment" > out/.build-environment
fi
cmake -S gpu -B out/gpu -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo -DBB_LTO=OFF -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ \
    -DBoost_DIR=/opt/bb-deps/lib/cmake/Boost-1.87.0 \
    -DOPENSSL_INCLUDE_DIR=/opt/bb-deps/include \
    -DOPENSSL_SSL_LIBRARY=/opt/bb-deps/lib/libssl.so \
    -DOPENSSL_CRYPTO_LIBRARY=/opt/bb-deps/lib/libcrypto.so \
    -DVulkan_INCLUDE_DIR=/opt/bb-deps/include -DVulkan_LIBRARY=/opt/bb-deps/lib/libvulkan.so
bash build.sh
python /src/packaging/linux/package_runtime.py /build/out /output/game_runtime
xvfb-run -a python packaging/build_launcher.py --dist-dir /output/launcher --check --ui-check
cp /output/launcher/linux/BloodborneLauncher /output/BloodborneLauncher
python /src/packaging/linux/check_abi.py /output/launcher/linux/BloodborneLauncher /output/game_runtime | tee /output/ABI.txt
