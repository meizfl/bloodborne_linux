#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Build the launcher and separate glibc 2.31 runtime with rootless Podman."""
import argparse
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jobs', type=int, default=4)
    parser.add_argument('--output', type=Path, default=ROOT / 'dist/debian11')
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    subprocess.run(['podman', 'build', '--layers', '-t', 'localhost/bbport-debian11',
                    '-f', 'packaging/linux/Containerfile', '.'], cwd=ROOT, check=True)
    image = subprocess.check_output(['podman', 'image', 'inspect', '--format', '{{.Id}}',
                                     'localhost/bbport-debian11'], text=True).strip()
    subprocess.run(['podman', 'run', '--rm', '--init', '-e', f'JOBS={args.jobs}', '-e', f'BB_BUILD_IMAGE={image}',
                    '-v', f'{ROOT}:/src:ro', '-v', f'{output}:/output',
                    '-v', 'bbport-debian11-build:/build',
                    'localhost/bbport-debian11', 'bash', '/src/packaging/linux/build-in-container.sh'], check=True)
    print(f'Built: {output}')


if __name__ == '__main__':
    main()
