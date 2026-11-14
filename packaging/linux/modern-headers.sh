#!/bin/bash
set -euo pipefail
cd /tmp
curl -fL --retry 3 https://github.com/KhronosGroup/Vulkan-Headers/archive/refs/tags/v1.4.363.tar.gz -o vk-headers.tar.gz
mkdir -p vk-headers
tar -xf vk-headers.tar.gz -C vk-headers --strip-components=1
cmake -S vk-headers -B vk-headers/build -G Ninja -DCMAKE_INSTALL_PREFIX=/opt/bb-deps
cmake --install vk-headers/build
curl -fL --retry 3 https://archives.boost.io/release/1.87.0/source/boost_1_87_0.tar.bz2 -o boost.tar.bz2
tar -xf boost.tar.bz2
cp -a boost_1_87_0/boost /opt/bb-deps/include/
# Only Boost's header libraries are used. Supply their standard CMake interface.
mkdir -p /opt/bb-deps/lib/cmake/Boost-1.87.0
cat > /opt/bb-deps/lib/cmake/Boost-1.87.0/BoostConfig.cmake <<'CMAKE'
set(Boost_VERSION_STRING 1.87.0)
set(Boost_FOUND TRUE)
if(NOT TARGET Boost::headers)
    add_library(Boost::headers INTERFACE IMPORTED)
    set_target_properties(Boost::headers PROPERTIES INTERFACE_INCLUDE_DIRECTORIES /opt/bb-deps/include)
endif()
CMAKE
mkdir -p /opt/bb-deps/share/licenses/boost
cp boost_1_87_0/LICENSE_1_0.txt /opt/bb-deps/share/licenses/boost/
rm -rf /tmp/boost_1_87_0 /tmp/boost.tar.bz2 /tmp/vk-headers /tmp/vk-headers.tar.gz
