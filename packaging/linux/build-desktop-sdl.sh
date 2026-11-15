#!/bin/bash
set -euo pipefail
# WindowSDL supports X11/Wayland, not SDL's direct-KMS console backend.
# Let the host GPU driver own libdrm/GBM instead of shadowing it with old libraries.
cmake -S /tmp/bb-deps/SDL -B /tmp/bb-deps/SDL/build -DSDL_KMSDRM=OFF
cmake --build /tmp/bb-deps/SDL/build -j8
cmake --install /tmp/bb-deps/SDL/build
