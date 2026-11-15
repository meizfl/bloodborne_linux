#!/bin/bash
set -euo pipefail
export LD_LIBRARY_PATH=/opt/bb-deps/lib
cd /tmp/bb-deps/ffmpeg
# FFmpeg 7.1's shared H.264/HEVC SEI code references the AOM film-grain helper.
# Enable HEVC too so that helper is included in libavcodec's link closure.
./configure --cc=clang --cxx=clang++ --ld=clang --ar=llvm-ar --ranlib=llvm-ranlib --prefix=/opt/bb-deps --enable-shared --disable-static --disable-programs --disable-doc \
    --disable-debug --disable-autodetect --disable-everything --enable-decoder=h264,aac,hevc \
    --enable-demuxer=mov --enable-parser=h264,aac --enable-protocol=file --enable-pic
make -j8
make install
