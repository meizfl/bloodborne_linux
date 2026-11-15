#!/usr/bin/env python3
"""Run in the Debian 11 builder; stage Tk and non-glibc dependencies for embedding."""
import argparse
import hashlib
from pathlib import Path
import re
import shutil
import subprocess

GLIBC = {'libc.so.6', 'libm.so.6', 'libdl.so.2', 'librt.so.1', 'libpthread.so.0',
         'libresolv.so.2', 'libutil.so.1', 'ld-linux-x86-64.so.2'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--prefix', type=Path, default=Path('/opt/tk92'))
    args = parser.parse_args()
    dest = args.destination
    dest.mkdir(parents=True, exist_ok=True)
    libs = dest / 'lib'
    libs.mkdir(exist_ok=True)
    roots = [args.prefix / 'lib/libtcl9.1.so', args.prefix / 'lib/libtcl9tk9.2.so']
    libraries = set(roots)
    for root in roots:
        output = subprocess.check_output(['ldd', str(root)], text=True)
        if 'not found' in output:
            raise SystemExit(output)
        libraries.update(Path(p) for p in re.findall(r'=>\s+(/\S+)', output))
    manifest = []
    for path in sorted(libraries):
        if path.name in GLIBC:
            continue
        target = libs / path.name
        shutil.copy2(path.resolve(), target)
        digest = hashlib.sha256(target.read_bytes()).hexdigest()
        manifest.append((path.name, digest))
        owner = subprocess.run(['dpkg-query', '-S', str(path)], text=True, capture_output=True)
        for line in owner.stdout.splitlines():
            package = line.partition(': ')[0].partition(':')[0]
            copyright_file = Path('/usr/share/doc') / package / 'copyright'
            if copyright_file.is_file():
                license_dir = dest / 'licenses'
                license_dir.mkdir(exist_ok=True)
                shutil.copy2(copyright_file, license_dir / (package + '.copyright'))
    license_dir = dest / 'licenses'
    license_dir.mkdir(exist_ok=True)
    for base in (Path('/tmp/bb-deps'), Path('/tmp/tk92')):
        if base.is_dir():
            for project in base.iterdir():
                if project.is_dir():
                    for pattern in ('LICENSE*', 'COPYING*', 'license.terms'):
                        for notice in project.glob(pattern):
                            if notice.is_file():
                                shutil.copy2(notice, license_dir / (project.name + '-' + notice.name))
    notices = Path('/opt/bb-deps/share/licenses')
    if notices.is_dir():
        shutil.copytree(notices, license_dir / 'bb-deps', dirs_exist_ok=True)
    for name in ('tcl9.1', 'tk9.2'):
        shutil.copytree(args.prefix / 'lib' / name, dest / name, dirs_exist_ok=True)
    (dest / 'libraries.sha256').write_text(''.join(f'{digest}  {name}\n' for name, digest in manifest))
    print(f'Staged {len(manifest)} libraries (glibc stays on the host)')


if __name__ == '__main__':
    main()
