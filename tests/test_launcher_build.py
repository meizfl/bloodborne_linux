# SPDX-License-Identifier: GPL-2.0-or-later
"""A distributable launcher cannot silently use a stock non-Clang bootloader."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('launcher_packager', ROOT / 'packaging/build_launcher.py')
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)


class ClangBootloaderTests(unittest.TestCase):
    def verify(self, compiler=None, tamper=False):
        with tempfile.TemporaryDirectory() as folder:
            package = Path(folder) / 'PyInstaller'
            binary = package / 'bootloader/Linux-64bit-intel/run'
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b'compiled bootloader')
            if compiler is not None:
                record = {'compiler': compiler, 'files': {'run': hashlib.sha256(binary.read_bytes()).hexdigest()}}
                (binary.parent / 'bb-clang-build.json').write_text(json.dumps(record))
            if tamper:
                binary.write_bytes(b'replaced stock bootloader')
            with mock.patch.dict(sys.modules, {'PyInstaller': types.SimpleNamespace(__file__=str(package / '__init__.py'))}), \
                    mock.patch.object(packager.sys, 'platform', 'linux'):
                packager.require_clang_bootloader()

    def test_unrecorded_stock_bootloader_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, 'Clang bootloader first'):
            self.verify()

    def test_verified_clang_build_is_accepted(self):
        self.verify('Debian clang version 23.1.6')

    def test_replaced_or_non_clang_bootloader_is_rejected(self):
        for compiler, tamper in [('GCC 14.2.0', False), ('clang version 23.1.6', True)]:
            with self.subTest(compiler=compiler, tamper=tamper), self.assertRaises(RuntimeError):
                self.verify(compiler, tamper)
