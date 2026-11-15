#!/bin/bash
set -euo pipefail
mkdir -p /tmp/bb-deps/openssl
cd /tmp/bb-deps
curl -fsSL --retry 3 https://github.com/openssl/openssl/releases/download/openssl-3.5.9/openssl-3.5.9.tar.gz -o openssl.tar.gz
tar -xf openssl.tar.gz -C openssl --strip-components=1
cd openssl
export CC=clang AR=llvm-ar RANLIB=llvm-ranlib
./Configure linux-x86_64 shared no-tests --prefix=/opt/bb-deps --libdir=lib --openssldir=/etc/ssl
make -j8
make install_sw
