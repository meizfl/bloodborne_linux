# Experimental Tk 9.2 native Wayland build

This opt-in toolchain uses Debian 11 and Clang/LLVM 23. It does not replace the
stable Tk 8.6 launcher image. Tk 9.2a0 is an upstream development snapshot.

Pinned sources:

- Tcl 9.1: `b6ad51455ca6e208ae9c1543ed555c6ead86fcc8`
- Tk `wayland` branch: `bdde6d9e656d3e292ac363bc4fe45e9c346b86c1`
- GLFW 3.4, libdecor 0.2.2, HarfBuzz 8.5.0
- Python 3.14.0 (the existing Python 3.12/Tk 8.6 extension cannot load Tk 9)

Build the stable base image first as described in [BUILD.md](../BUILD.md), then:

```sh
podman build --layers -t localhost/bbport-tk92 -f packaging/linux/Containerfile.tk92 .
podman build --layers -t localhost/bbport-tk92-python -f packaging/linux/Containerfile.tk92-python .
```

Tk is configured with `--enable-wayland`; this is a separate native backend,
not XWayland. Native rendering needs EGL/OpenGL ES. For a software rendering check, forward the session’s Wayland socket and unset
`DISPLAY`. Verify `tk windowingsystem` reports `wayland` before evaluating the
launcher. Do not substitute the stable installation until GUI checks pass.

The experimental container includes its own Mesa for software rendering checks;
its Mesa driver libraries must not be copied over the user's system GPU stack.

Debian 11 compatibility: `tk92-debian11-xkb.patch` matches the old XKB header's
non-const `XkbOpenDisplay` argument in Tk's unused Wayland shim and stub table.
`CPPFLAGS=-D_GNU_SOURCE` exposes `strcasestr` used by the new font backend.
Both adjustments are local to the experimental build.

Run the native Tk check from a Wayland session:

```sh
podman run --rm --init -v "$PWD:/src:ro" \
    -v "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY:/tmp/bb-wayland/wayland-0" \
    -e XDG_RUNTIME_DIR=/tmp/bb-wayland -e WAYLAND_DISPLAY=wayland-0 \
    -e LIBGL_ALWAYS_SOFTWARE=1 localhost/bbport-tk92 \
    bash -c 'chmod 700 /tmp/bb-wayland; bash /src/packaging/linux/check-tk92-wayland.sh' 
```

The check must report `WINDOW_SYSTEM=wayland` and `DISPLAY_EXISTS=0`.
The Debian 11 Weston 9 headless backend did not initialize this GLFW/Tk snapshot;
the native desktop compositor check succeeds instead. This is an experimental
backend, so test against the compositor actually used for the launcher.

## Launcher renderer fixes and single executable

The upstream snapshot initially drew hidden pages over the active page and let
scrolled-out widgets paint over the footer. `tk92-hidden-widgets.patch` checks
ancestor visibility, excludes hidden subtrees from occlusion masks, intersects
all ancestor bounds, and masks fully clipped widgets. The launcher also unmaps
inactive pages on Wayland and animates wheel input; Windows/X11 keep their
existing page stack and wheel behavior.

```sh
podman build --layers -t localhost/bbport-tk92-render -f packaging/linux/Containerfile.tk92-render .
podman build --layers -t localhost/bbport-tk92-preview -f packaging/linux/Containerfile.tk92-preview .
mkdir -p dist/tk92
podman run --rm --init -v "$PWD:/src" -v "$PWD/dist/tk92:/output" \
    -v "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY:/tmp/bb-wayland/wayland-0" \
    -e XDG_RUNTIME_DIR=/tmp/bb-wayland -e WAYLAND_DISPLAY=wayland-0 \
    -e LIBGL_ALWAYS_SOFTWARE=1 localhost/bbport-tk92-preview bash -c '
      chmod 700 /tmp/bb-wayland
      cd /src
      bash packaging/linux/check-tk92-wayland.sh /opt/python-tk92/bin/python3.14 \
        packaging/build_launcher.py --native-wayland --dist-dir /output --check --ui-check
      /opt/python-tk92/bin/python3.14 packaging/linux/check_abi.py /output/linux/BloodborneLauncher
    '
```

The resulting `dist/tk92/linux/BloodborneLauncher` needs no Python or shell wrapper.
Copy it next to `game_runtime/` under a separate name such as
`BloodborneLauncher-Wayland` while evaluating this development backend.
`--native-wayland` includes libdecor plugins and locale data. Its runtime hook
prefers a sufficiently new host libstdc++ before loading EGL, with the bundled
Debian library as fallback. This lets current Mesa plugins load without passing
LD_PRELOAD or private desktop library paths to the game or browser.

Validation: KDE Wayland with DISPLAY unset; launcher page creation, only one
page mapped at a time, startup and WM_DELETE_WINDOW; single-file ABI check:
135 ELF files, maximum required glibc 2.30. This proves the Debian 11 ABI
baseline, not full visual or input compatibility with every compositor.

Smooth scrolling uses time-based animation and accumulates fractional wheel
movement. The X11 Button-4/5-to-MouseWheel adapter is disabled on Wayland because
Tk already emits both native and compatibility events. Offscreen widgets are
culled before NanoVG/OpenGL work to reduce redraw cost on long settings pages.

A local scroll callback benchmark on RX 5600 XT improved from approximately
20.0 ms to 13.2 ms mean interval after culling (50 to 76 updates/s). These are
application callback measurements, not compositor presentation FPS; no fixed
frame rate is guaranteed by this development renderer.

The final wheel animation uses a critically damped spring: new input retargets
motion without resetting velocity, fractional pixels accumulate, and reaching
a scroll boundary cancels inertia. Regression tests cover acceleration from
rest, distance preservation across multiple notches, page changes and edges.

`tk92-canvas-damage.patch` restores photo items across the same six-pixel
antialiasing margin cleared by Canvas. Without it, partial redraws leave black
rectangular borders over the banner. `check_canvas_damage.py` reads actual FBO
pixels after deleting several shapes; the original snapshot fails with black
pixels and the patched renderer restores the original background color.
Run it through `check-tk92-wayland.sh` using the preview image and Python 3.14.
