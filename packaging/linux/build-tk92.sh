#!/bin/bash
set -euo pipefail
cd /tmp/tk92
curl -fsSL --retry 3 https://codeload.github.com/tcltk/tcl/tar.gz/b6ad51455ca6e208ae9c1543ed555c6ead86fcc8 | tar xz
curl -fsSL --retry 3 https://codeload.github.com/tcltk/tk/tar.gz/bdde6d9e656d3e292ac363bc4fe45e9c346b86c1 | tar xz
cd tcl-*/unix
./configure --disable-zipfs --prefix=/opt/tk92
make -j8
make install-binaries install-libraries install-headers
cd /tmp/tk92/tk-*
patch -p1 < /tmp/tk92-debian11-xkb.patch
cd unix
CPPFLAGS=-D_GNU_SOURCE ./configure --disable-zipfs --prefix=/opt/tk92 --with-tcl=/opt/tk92/lib --enable-wayland
make -j8
make install-binaries install-libraries install-headers
