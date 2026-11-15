"""PyInstaller runtime hook for the experimental EGL/Wayland frontend."""
import ctypes
import os
from pathlib import Path
import re
import sys

root = Path(sys._MEIPASS)
# A current Mesa plugin may require a newer libstdc++ than the Debian 11 bundle.
# Prefer a compatible host copy before any GUI library loads; keep the bundled
# library as the fallback on machines without one. No LD_PRELOAD leaks to games.
def versions(path):
    return {tuple(map(int, version.split(b'.'))) for version in
            re.findall(rb'GLIBCXX_([0-9]+\.[0-9]+(?:\.[0-9]+)?)', path.read_bytes())}

bundled = root / 'libstdc++.so.6'
if bundled.exists():
    required = versions(bundled)
    for directory in ('/usr/lib/x86_64-linux-gnu', '/lib/x86_64-linux-gnu', '/usr/lib64', '/usr/lib', '/lib64'):
        candidate = Path(directory) / 'libstdc++.so.6'
        try:
            if candidate.exists() and required <= versions(candidate):
                ctypes.CDLL(str(candidate), mode=ctypes.RTLD_GLOBAL)
                break
        except OSError:
            pass

if not os.environ.get('FONTCONFIG_FILE'):
    config = str(root / 'assets/fonts/fonts.conf')
    os.environ['FONTCONFIG_FILE'] = config
    os.environ['BB_BUNDLED_FONTCONFIG'] = config
for variable, marker, relative in (
        ('LIBDECOR_PLUGIN_DIR', 'BB_BUNDLED_LIBDECOR', 'wayland-data/libdecor'),
        ('XLOCALEDIR', 'BB_BUNDLED_XLOCALE', 'wayland-data/locale')):
    if not os.environ.get(variable):
        os.environ[variable] = os.environ[marker] = str(root / relative)
