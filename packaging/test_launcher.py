#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Run launcher tests and expose failures as GitHub annotations on either OS."""
import argparse
import os
from pathlib import Path
import sys
import unittest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pattern', default='test_launcher_*.py')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    os.chdir(root)
    suite = unittest.defaultTestLoader.discover(str(root / 'tests'), pattern=args.pattern)
    result = unittest.TextTestRunner(verbosity=1).run(suite)
    if os.environ.get('GITHUB_ACTIONS') == 'true':
        def escape(value):
            return str(value).replace('%', '%25').replace('\r', '%0D').replace('\n', '%0A')
        for test, trace in result.errors + result.failures:
            title = escape(test.id()).replace(':', '%3A').replace(',', '%2C')
            print(f'::error title={title}::{escape(trace)}', flush=True)
    return 0 if result.wasSuccessful() else 1


if __name__ == '__main__':
    sys.exit(main())
