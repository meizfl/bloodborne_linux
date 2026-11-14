#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Collect Debian 11 native binaries and their ELF closure, excluding host glibc."""
import re
from pathlib import Path
import shutil
import subprocess
import sys

SYSTEM = re.compile(r'^(ld-linux.*|lib(c|m|pthread|dl|rt|resolv|util|anl)\.so[.\d]*)$')


def package(source, target):
    target.mkdir(parents=True, exist_ok=True)
    library = target / 'lib'
    library.mkdir(exist_ok=True)
    pending = []
    for name in ('bb-probe', 'bb-gpu-capabilities'):
        shutil.copy2(source / name, target / name)
        pending.append(target / name)
    # The runtime intentionally loads bbgpu through its normal DT_NEEDED entry.
    seen = set()
    while pending:
        binary = pending.pop()
        if binary.name in seen:
            continue
        seen.add(binary.name)
        output = subprocess.check_output(['ldd', str(binary)], text=True)
        if 'not found' in output:
            raise RuntimeError(output)
        for name, path in re.findall(r'^\s*(\S+) => (/\S+)', output, re.M):
            if SYSTEM.match(name):
                continue
            destination = library / name
            if not destination.exists():
                shutil.copy2(path, destination)
                pending.append(destination)
    for binary in library.iterdir():
        subprocess.run(['patchelf', '--set-rpath', '$ORIGIN', str(binary)], check=True)
        subprocess.run(['strip', '--strip-unneeded', str(binary)], check=True)
    # Do not patchelf the non-PIE loader: GNU symbol-version tables must retain their layout.
    for name in ('bb-probe', 'bb-gpu-capabilities'):
        subprocess.run(['strip', '--strip-debug', str(target / name)], check=True)
    (target / 'BUILD.txt').write_text(
        'Debian 11 / glibc 2.31, x86-64. Built with GCC 14.2.0.\n'
        'GPU drivers and glibc are provided by the host.\n'
        'FFmpeg 7.1.1: H.264/AAC decoding, MOV/MP4 demuxing; LGPL build.\n')
    licenses = target / 'licenses'
    licenses.mkdir(exist_ok=True)
    for doc in Path('/usr/share/doc').glob('*/copyright'):
        shutil.copy2(doc, licenses / (doc.parent.name + '.txt'))
    for base in (Path('/tmp/bb-deps'), Path('/build/gpu/third_party')):
        if base.is_dir():
            for project in base.iterdir():
                if not project.is_dir():
                    continue
                for pattern in ('LICENSE*', 'COPYING*'):
                    for notice in project.glob(pattern):
                        if notice.is_file():
                            shutil.copy2(notice, licenses / (project.name + '-' + notice.name))
    for prefix in ('/opt/bb-deps/share/licenses', '/opt/gcc/distro-info/licenses'):
        if Path(prefix).is_dir():
            shutil.copytree(prefix, licenses / Path(prefix).parent.name, dirs_exist_ok=True)


if __name__ == '__main__':
    package(Path(sys.argv[1]), Path(sys.argv[2]))
