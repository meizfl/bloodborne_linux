#!/bin/bash
# Invoked by Podman, never needed by the installed launcher.
set -euo pipefail
mkdir -p /build
cd /src
tar --exclude=./out --exclude=./dist --exclude=./.git --exclude='*/__pycache__' -cf - . | tar -xf - -C /build
cd /build
# Source archive builds also work without git metadata. Applied vendor patches are idempotent.
for change in gpu/patches/fsr-vulkan/*.patch; do
    if ! patch --dry-run -R -p1 -d gpu/third_party/fsr-vulkan < "$change" >/dev/null 2>&1; then
        patch -p1 -d gpu/third_party/fsr-vulkan < "$change"
    fi
done
# build.sh's patch loop uses git apply; a tiny local repository records this source snapshot.
git init -q
git -C gpu/third_party/fsr-vulkan init -q
export CC=gcc CXX=g++ BB_LTO=OFF CMAKE_BUILD_PARALLEL_LEVEL=${JOBS:-4}
bash build.sh
python /src/packaging/linux/package_runtime.py /build/out /output/game_runtime
xvfb-run -a python packaging/build_launcher.py --dist-dir /output/launcher --check --ui-check
cp /output/launcher/linux/BloodborneLauncher /output/BloodborneLauncher
python /src/packaging/linux/check_abi.py /output/launcher/linux/BloodborneLauncher /output/game_runtime
