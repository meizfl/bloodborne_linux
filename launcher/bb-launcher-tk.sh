#!/usr/bin/env bash
# Native Windows/Linux Tk interface, using the system Python + Tk + Pillow.
set -euo pipefail
port=$(cd -- "$(dirname -- "$0")/.." && pwd)
exec "${PYTHON:-python3}" "$port/launcher/bloodborne_launcher.py" "$@"
