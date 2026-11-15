#!/bin/bash
set -euo pipefail
cd /tmp/tk92
curl -fsSL --retry 3 https://www.python.org/ftp/python/3.14.0/Python-3.14.0.tar.xz | tar xJ
cd Python-3.14.0
export PKG_CONFIG_PATH=/opt/tk92/lib/pkgconfig:$PKG_CONFIG_PATH
export LD_LIBRARY_PATH=/opt/tk92/lib:$LD_LIBRARY_PATH
export LDFLAGS="-fuse-ld=lld -Wl,-rpath,/opt/python-tk92/lib:/opt/tk92/lib:/opt/bb-deps/lib"
./configure --prefix=/opt/python-tk92 --enable-shared --disable-test-modules --without-static-libpython --with-openssl=/opt/bb-deps --with-openssl-rpath=auto
make -j8
make install
/opt/python-tk92/bin/python3.14 -c 'import _tkinter; print("Tcl:", _tkinter.TCL_VERSION, "Tk:", _tkinter.TK_VERSION)'
/opt/python-tk92/bin/python3.14 -m pip install Pillow==11.3.0
CC=clang CXX=clang++ PYINSTALLER_COMPILE_BOOTLOADER=1 /opt/python-tk92/bin/python3.14 -m pip install --no-binary=PyInstaller PyInstaller==6.16.0
/opt/python-tk92/bin/python3.14 /tmp/build_bootloader.py --verify-existing
