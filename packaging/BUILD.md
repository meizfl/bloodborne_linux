# Build and install (Windows and Linux)

The launcher is one executable, with Python, Tk, Pillow, fonts, artwork and game
preparation code embedded. Players do not need Python or shell launch wrappers.
PyInstaller extracts its private payload to an OS temporary directory while it runs.
The native game runtime is a separate directory; decrypted game files are not distributed.

```text
Bloodborne/
  BloodborneLauncher          # Linux; Bloodborne.exe on Windows
  game_runtime/
    bb-probe                  # .exe on Windows
    bb-gpu-capabilities       # .exe on Windows
    lib/                      # Linux shared libraries; Windows DLLs beside its EXEs
  Game/
    eboot.bin
    sce_sys/
    ...
```

Start the executable directly, or use `--play` to start the game with saved settings.
Select the game folder in the launcher if it is elsewhere. Saves, engine settings,
launcher settings and generated files are stored outside the installation:

* Linux: `$HOME/.local/share/Bloodborne` (`user/`, `bbport.ini`, `settings.json`, `out/`).
* Windows: `%LOCALAPPDATA%\Bloodborne` (inside AppData).

`BB_DATA_DIR` overrides the data root. `BB_PROBE` overrides the native loader.
The separate runtime and game directories must remain available during play.

## Linux: Debian 11 / glibc 2.31, using Podman

Host tools: Git, Python 3 (only the build coordinator) and rootless Podman. The host
compiler and glibc do not determine the resulting ABI. The container uses Debian 11,
Python 3.12/Tk 8.6, Clang/LLVM 23, libc++, pinned dependency releases and a dated
Debian snapshot. All project C/C++ code and the PyInstaller bootloader are built
with Clang; the container builds its C++ dependencies with libc++ too.

```sh
git clone --recursive git@github.com:meizfl/bloodborne_linux.git
cd bloodborne_linux
git switch codex/linux-shared-launcher
python3 packaging/linux/build.py --jobs 4
```

Outputs: `dist/debian11/BloodborneLauncher` and `dist/debian11/game_runtime/`.
Copy them beside `Game/`. No `.py`, `.sh` or `_internal/` directory is needed there.
The first build downloads and compiles dependencies; Podman caches the build image
and the `bbport-debian11-build` volume retains incremental native build files.
Use `--output /absolute/path` to choose another output directory.

The build checks imports, every UI page and normal startup/close under Xvfb. It also
inspects **all embedded ELF files** inside the launcher and the runtime's shared-library
closure, rejecting GLIBC requirements above 2.31. A source build on a newer host is
possible, but it does not establish this compatibility baseline.

The host still supplies glibc, GPU drivers and the desktop session. This is x86-64
Linux/Vulkan, not a static binary or an emulation layer. FFmpeg is an LGPL build
with the H.264/HEVC/AAC and MP4/MOV support used by the game's movies.

To build just the launcher inside the prepared image:

```sh
podman run --rm --init -v "$PWD:/src:ro" -v "$PWD/dist/debian11:/output" \
  localhost/bbport-debian11 bash -c \
  'mkdir /tmp/build && cd /src && tar --exclude=./out --exclude=./dist --exclude=.git -cf - . | tar -xf - -C /tmp/build; cd /tmp/build && xvfb-run -a python packaging/build_launcher.py --dist-dir /output/launcher --check --ui-check'
```

## Windows: build the single executable

Install 64-bit Python 3.12+ from python.org with Tcl/Tk enabled, Git, LLVM/Clang
and Visual Studio Build Tools with the Windows SDK. Open the x64 Developer
PowerShell so Clang can locate the Windows SDK and linker:

```powershell
git clone --recursive https://github.com/meizfl/bloodborne_linux.git
cd bloodborne_linux
git switch codex/linux-shared-launcher
py -3.12 -m venv .venv
.\.venv\Scripts\python -m pip install -r packaging/launcher-requirements.txt
$env:CC = "clang"
$env:CXX = "clang++"
$env:LDFLAGS = "-fuse-ld=lld"
$env:PYINSTALLER_COMPILE_BOOTLOADER = "1"
$env:PYINSTALLER_BOOTLOADER_WAF_ARGS = "--clang"
.\.venv\Scripts\python -m pip install --force-reinstall --no-cache-dir --no-binary=PyInstaller PyInstaller==6.16.0
.\.venv\Scripts\python packaging/test_launcher.py
.\.venv\Scripts\python packaging/build_launcher.py --check --ui-check
```

Output: `dist/launcher/windows/Bloodborne.exe`. This contains the launcher and its
preparation scripts, not the native runtime or proprietary game files.
`--onedir` is available for the legacy folder package.

For the native Windows runtime, install MSYS2 and the CLANG64 dependencies listed in
[windows/README.md](windows/README.md), then run `bash build.sh` in CLANG64.
Copy `out/bb-probe.exe`, `out/bb-gpu-capabilities.exe` and their runtime DLLs to
`game_runtime/`. The existing `packaging/windows/package.sh` automates DLL collection
for the original full package; its `bin/` contents can supply `game_runtime/`.
Linux binaries cannot serve as the Windows runtime. Windows packages must be built
and checked on Windows; the CI workflow does this for the launcher.

## Source and licensing

The full sources and build recipes are in the `codex/linux-shared-launcher` branch
of https://github.com/meizfl/bloodborne_linux. Clone with `--recursive` to obtain
third-party submodules. The project is GPL-2.0-or-later; bundled dependencies retain
their own licenses. Distribute the corresponding source, including the pinned
submodules and `gpu/patches/fsr-vulkan`, with binary distributions. Game data is excluded.
