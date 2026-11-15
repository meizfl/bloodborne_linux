#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Build the shared launcher on Windows or Linux. Does not build or bundle the game."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def copy_resources(target):
    ignore = shutil.ignore_patterns('__pycache__', '*.pyc')
    for name in ('launcher', 'scripts', 'patches', 'data'):
        shutil.copytree(ROOT / name, target / name, dirs_exist_ok=True, ignore=ignore)
    for name in ('run.py', 'run.sh', 'LICENSE'):
        shutil.copy2(ROOT / name, target / name)
    (target / 'launch.py').write_text(
        'from pathlib import Path\nimport sys\n'
        'sys.path.insert(0, str(Path(__file__).resolve().parent / "launcher"))\n'
        'from bloodborne_launcher import main\nraise SystemExit(main())\n', encoding='utf-8')


def require_clang_bootloader():
    import PyInstaller
    platform = 'Windows-64bit-intel' if sys.platform == 'win32' else 'Linux-64bit-intel'
    root = Path(PyInstaller.__file__).parent / 'bootloader' / platform
    name = 'runw.exe' if sys.platform == 'win32' else 'run'
    marker = root / 'bb-clang-build.json'
    if not marker.is_file():
        raise RuntimeError('Build the Clang bootloader first: python packaging/build_bootloader.py')
    info = json.loads(marker.read_text(encoding='utf-8'))
    if 'clang' not in info.get('compiler', '').lower() or info.get('files', {}).get(name) != hashlib.sha256((root / name).read_bytes()).hexdigest():
        raise RuntimeError('Clang bootloader verification failed; rerun packaging/build_bootloader.py')


def linux_tk_libraries():
    """Include Tk's ELF closure even for libraries PyInstaller considers system X11.

    A desktop normally has them, but a single executable must also import cleanly
    on the minimal Debian 11 verification image. Host glibc stays external.
    """
    import _tkinter
    output = subprocess.check_output(['ldd', _tkinter.__file__], text=True)
    system = re.compile(r'^(ld-linux.*|lib(c|m|pthread|dl|rt|resolv|util|anl)\.so[.\d]*)$')
    return [path for name, path in re.findall(r'^\s*(\S+) => (/\S+)', output, re.M)
            if not system.match(name)]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--onedir', action='store_true', help='legacy folder package instead of a single executable')
    parser.add_argument('--source-only', action='store_true', help='create a source zip; requires Python, Tk and Pillow')
    parser.add_argument('--dist-dir', type=Path, default=ROOT / 'dist/launcher')
    parser.add_argument('--check', action='store_true', help='check imports/Tcl/assets in the frozen executable')
    parser.add_argument('--ui-check', action='store_true', help='exercise all frozen UI pages; requires a display')
    args = parser.parse_args(argv)
    if sys.platform != 'win32' and not sys.platform.startswith('linux'):
        parser.error('Native launcher packages are supported on Windows and Linux; build on the target OS.')
    destination = args.dist_dir.resolve()
    destination.mkdir(parents=True, exist_ok=True)
    if args.source_only:
        source = destination / 'BloodborneLauncher-source'
        source.mkdir(exist_ok=True)
        copy_resources(source)
        print(shutil.make_archive(str(source), 'zip', destination, source.name))
        return 0
    require_clang_bootloader()
    host = 'windows' if sys.platform == 'win32' else 'linux'
    name = 'Bloodborne' if host == 'windows' else 'BloodborneLauncher'
    work = ROOT / 'out/launcher-build' / host
    work.mkdir(parents=True, exist_ok=True)
    command = [sys.executable, '-m', 'PyInstaller', '--noconfirm', '--clean', '--onedir' if args.onedir else '--onefile', '--windowed',
               '--name', name, '--distpath', str(destination / host), '--workpath', str(work / 'work'),
               '--specpath', str(work), '--paths', str(ROOT), '--paths', str(ROOT / 'launcher'), '--paths', str(ROOT / 'scripts'),
               '--add-data', str(ROOT / 'launcher/assets') + os.pathsep + 'assets']
    for item in ('scripts', 'patches', 'data', 'run.py', 'LICENSE'):
        command += ['--add-data', str(ROOT / item) + os.pathsep + (item if (ROOT / item).is_dir() else '.')]
    command += ['--add-data', str(ROOT / 'launcher/bloodborne.png') + os.pathsep + 'launcher']
    # The preparation scripts run dynamically in the Windows executable's --script role.
    for module in ('argparse', 'base64', 'bisect', 'collections', 'contextlib', 'hashlib', 'json',
                   're', 'shutil', 'struct', 'tempfile', 'xml.etree.ElementTree', 'urllib.request', 'ctypes.wintypes',
                   'PIL._tkinter_finder', 'PIL._imagingtk', 'run', 'linux_start', 'bbport_linux_runner'):
        command += ['--hidden-import', module]
    if host == 'linux':
        for library in linux_tk_libraries():
            command += ['--add-binary', library + os.pathsep + '.']
    if host == 'windows':
        from PIL import Image
        icon = work / 'bloodborne.ico'
        with Image.open(ROOT / 'launcher/bloodborne.png') as image:
            image.save(icon, format='ICO', sizes=[(16, 16), (32, 32), (48, 48), (256, 256)])
        command += ['--icon', str(icon)]
    command.append(str(ROOT / 'launcher/bloodborne_launcher.py'))
    subprocess.run(command, cwd=ROOT, check=True)
    package = destination / host / name
    executable = package / (name + ('.exe' if host == 'windows' else '')) if args.onedir else package.with_suffix('.exe' if host == 'windows' else '')
    if args.onedir:
        copy_resources(package)
    check_dir = package if args.onedir else executable.parent
    if args.check:
        checked = subprocess.run([str(executable), '--check-runtime'], cwd=check_dir,
                                 capture_output=True, text=True, timeout=30)
        if checked.returncode:
            raise RuntimeError(checked.stdout + checked.stderr)
        print(checked.stdout)
        script_check = subprocess.run([str(executable), '--script', 'patches.py', '--help'],
                                      cwd=check_dir, capture_output=True, text=True, timeout=30)
        if script_check.returncode or 'usage:' not in script_check.stdout:
            raise RuntimeError(script_check.stdout + script_check.stderr)
        print('PASS: embedded preparation script without external Python')
    if args.ui_check:
        checked = subprocess.run([str(executable), '--smoke-test'], cwd=check_dir,
                                 capture_output=True, text=True, timeout=120)
        if checked.returncode:
            raise RuntimeError(checked.stdout + checked.stderr)
        print(checked.stdout)
    if args.onedir:
        archive = shutil.make_archive(str(destination / ('BloodborneLauncher-' + host)), 'zip', package.parent, package.name)
    else:
        import zipfile
        archive = destination / ('BloodborneLauncher-' + host + '.zip')
        with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as zipped:
            zipped.write(executable, executable.name)
    print(archive)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
