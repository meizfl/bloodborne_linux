#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Build and record a Clang PyInstaller bootloader for native Windows/Linux."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verify-existing', action='store_true', help='Linux: verify the ELF compiler ID instead of rebuilding')
    args = parser.parse_args()
    compiler = shutil.which('clang')
    if not compiler:
        parser.error('Clang is required. Install LLVM and use a Windows SDK developer shell on Windows.')
    version = subprocess.check_output([compiler, '--version'], text=True).splitlines()[0]
    if 'clang' not in version.lower():
        parser.error('The selected compiler is not Clang.')
    if not args.verify_existing:
        env = dict(os.environ, CC='clang', CXX='clang++',
                   PYINSTALLER_COMPILE_BOOTLOADER='1', PYINSTALLER_BOOTLOADER_WAF_ARGS='--clang',
                   LDFLAGS='-fuse-ld=lld')
        result = subprocess.run([sys.executable, '-m', 'pip', 'install', '--force-reinstall', '--no-cache-dir',
                                 '--no-binary=PyInstaller', 'PyInstaller==6.16.0'], env=env,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        print(result.stdout, flush=True)
        if result.returncode:
            if os.environ.get('GITHUB_ACTIONS') == 'true':
                details = result.stdout[-16000:].replace('%', '%25').replace('\r', '%0D').replace('\n', '%0A')
                print('::error title=Clang bootloader build failed::' + details, flush=True)
            raise RuntimeError('Clang bootloader compilation failed; see compiler output above.')
    import PyInstaller
    root = Path(PyInstaller.__file__).parent / 'bootloader'
    platform = 'Windows-64bit-intel' if sys.platform == 'win32' else 'Linux-64bit-intel'
    binaries = list((root / platform).glob('run*'))
    binaries = [path for path in binaries if path.is_file()]
    if not binaries:
        raise RuntimeError('Built bootloader files were not found: ' + str(root / platform))
    if args.verify_existing:
        if not sys.platform.startswith('linux'):
            parser.error('--verify-existing requires Linux ELF compiler metadata')
        for path in binaries:
            comment = subprocess.check_output(['readelf', '-p', '.comment', str(path)], text=True)
            if 'clang' not in comment.lower():
                raise RuntimeError('Bootloader was not built with Clang: ' + str(path))
    marker = {'compiler': version, 'files': {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                                           for path in binaries}}
    (root / platform / 'bb-clang-build.json').write_text(json.dumps(marker, indent=2), encoding='utf-8')
    print('PASS: Clang bootloader:', version)


if __name__ == '__main__':
    main()
