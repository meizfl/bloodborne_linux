# Shared Windows/Linux launcher

The Tkinter interface now runs on Windows and Linux. `launcher/bloodborne_launcher.py`
is the neutral entry point; `launcher/bbport_launcher_win.py` stays compatible with
the existing Windows package and its command-line roles. The older GTK launcher
remains available separately.

## Run from source

Install Python 3.12+, Tkinter and Pillow, then:

```sh
python3 launcher/bloodborne_launcher.py
```

On Debian/Ubuntu, Tkinter and Pillow are available as `python3-tk` and `python3-pil`;
on Arch, install `tk` and `python-pillow`. Windows' python.org installer includes Tk.
The source launcher also works with the existing Windows packaging workflow.

Linux game binaries must be built separately (`bash build.sh --test`). Select the
decrypted game's folder in the launcher. It finds `bin/bb-probe` or `out/bb-probe`;
`BB_PROBE` can point to an existing native loader (with its libraries and the
capabilities helper). The game still needs x86-64 and a compatible Vulkan driver.

Windows settings keep their existing `%APPDATA%/bbport-launcher` location.
Linux settings use `$XDG_CONFIG_HOME/bbport-launcher-tk`, defaulting to
`~/.config/bbport-launcher-tk`; this does not overwrite the GTK launcher's profile.
Linux generated files, saves and `bbport.ini` use `$XDG_DATA_HOME/bbport`, defaulting
to `~/.local/share/bbport`. `BB_DATA_DIR` overrides that location on either OS.

## Build a distributable launcher

Build on the target OS; PyInstaller does not cross-compile between Windows and
Linux. The existing `packaging/windows/package.sh` still builds the full Windows
game package. To build only the shared launcher:

```sh
python -m venv out/launcher-venv
# Windows: out\launcher-venv\Scripts\activate
# Linux:   source out/launcher-venv/bin/activate
python -m pip install -r packaging/launcher-requirements.txt
python packaging/build_launcher.py --check --ui-check
```

`--ui-check` needs a desktop session (or `xvfb-run -a` on Linux). The archive is
`dist/launcher/BloodborneLauncher-windows.zip` or
`dist/launcher/BloodborneLauncher-linux.zip`. Keep its whole directory together:
the executable, `_internal`, scripts and assets are all required. No game files
or native game runtime are included. Linux's game preparation scripts use the
system Python 3; Pillow there is optional for trophy pictures.

For an existing source checkout, a standalone launcher can use its engine and
scripts with `BB_PORT_DIR=/path/to/checkout`. Without an engine, the launcher opens
normally and explains what is missing when Play is pressed.

`python packaging/build_launcher.py --source-only` makes a source zip instead.
Unpack it and run `python launch.py` with Python, Tk and Pillow installed.

## Platform boundaries

Windows keeps its native title bar integration, tray, process job objects,
updater, shortcuts and Steam integration. Linux uses the desktop's window frame,
XDG paths, `xdg-open`, desktop entries, POSIX process groups and file locks.
The launch lock includes preparation and survives the `exec` into the game.
Stop targets the current launch's process group, including its child processes.

Windows DLL injection (BBSC), DirectX/Windows FSR providers, the Windows updater
and automated Windows route benchmarks are unavailable on Linux with an explicit
message. Linux Steam entries can be added manually with `--play`. Ordinary online
settings remain available; the startup path writes the same server routes as
`run.py`. No live online session was used for validation.

Nix packages use the shared Tk launcher by default; `gtkLauncher = true` retains
the previous GTK UI. The GTK source entry point is unchanged.

## Validation

```sh
python -m unittest discover -s tests -p test_launcher_platform.py
python launcher/bloodborne_launcher.py --smoke-test
python packaging/build_launcher.py --check --ui-check
```

The smoke test builds every page on a temporary profile, without a game or an
account. CI runs the tests, source UI, native package checks and frozen UI on both
Windows and Linux. On Linux, the tests also exercise process-group termination
and the preparation lock across a real `exec`.

Local validation: Arch Linux, Python 3.14, Tcl/Tk 8.6, Pillow 12.3, PyInstaller
6.22.3. The native game reached the title screen with an RX 5600 XT and RADV;
source and frozen launcher UI checks passed. Windows needs its native CI run;
the Nix expression and full AppImage were not built in this environment.
