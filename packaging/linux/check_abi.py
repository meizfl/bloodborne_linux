#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Reject ELF files requiring a glibc newer than the Debian 11 baseline."""
import re
from pathlib import Path
import subprocess
import sys


def check(paths, limit=(2, 31)):
    count = 0
    maximum = (0, 0)
    for root in paths:
        for path in root.rglob('*') if root.is_dir() else [root]:
            if not path.is_file() or path.open('rb').read(4) != b'\x7fELF':
                continue
            count += 1
            data = subprocess.check_output(['readelf', '--version-info', str(path)], text=True)
            versions = [tuple(map(int, value.split('.'))) for value in re.findall(r'GLIBC_([\d.]+)', data)]
            needed = max(versions, default=(0, 0))
            maximum = max(maximum, needed)
            if needed > limit:
                raise RuntimeError(f'{path}: requires GLIBC_{".".join(map(str, needed))}, baseline is 2.31')
    if not count:
        raise RuntimeError('No ELF files found')
    print(f'PASS: {count} ELF files; maximum required glibc {".".join(map(str, maximum))}')


if __name__ == '__main__':
    import tempfile
    from PyInstaller.archive.readers import CArchiveReader
    paths = [Path(value) for value in sys.argv[1:]]
    with tempfile.TemporaryDirectory(prefix='bb-abi-') as folder:
        for path in list(paths):
            if not path.is_file():
                continue
            try:
                archive = CArchiveReader(str(path))
            except Exception:
                continue
            for index, (name, entry) in enumerate(archive.toc.items()):
                if entry[-1] != 'b':
                    continue
                blob = archive.extract(name)
                if blob[:4] == b'\x7fELF':
                    target = Path(folder) / str(index) / Path(name).name
                    target.parent.mkdir(parents=True)
                    target.write_bytes(blob)
        paths.append(Path(folder))
        check(paths)
