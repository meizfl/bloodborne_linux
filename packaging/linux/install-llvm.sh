#!/bin/bash
set -euo pipefail
curl -fsSL https://apt.llvm.org/llvm-snapshot.gpg.key -o /etc/apt/trusted.gpg.d/apt.llvm.org.asc
printf '%s\n' 'deb https://apt.llvm.org/bullseye/ llvm-toolchain-bullseye-23 main' > /etc/apt/sources.list.d/llvm.list
apt-get update
apt-get install -y --no-install-recommends clang-23 lld-23 llvm-23 libc++-23-dev libc++abi-23-dev libunwind-23-dev
for tool in clang clang++ lld ld.lld llvm-ar llvm-ranlib llvm-nm llvm-strip; do
    ln -sf "/usr/bin/$tool-23" "/usr/local/bin/$tool"
done
rm -rf /var/lib/apt/lists/*
clang --version
