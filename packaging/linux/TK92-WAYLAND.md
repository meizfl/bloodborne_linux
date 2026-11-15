# Native Tk 9.2 / Wayland for the Rust launcher

Build from the repository root using rootless Podman:

```sh
podman build --layers -t localhost/bbport-debian11 -f packaging/linux/Containerfile .
podman build --layers -t localhost/bbport-tk92 -f packaging/linux/Containerfile.tk92 .
podman build --layers -t localhost/bbport-tk92-render -f packaging/linux/Containerfile.tk92-render .
podman build --layers -t localhost/bbport-rust-launcher -f packaging/linux/Containerfile.rust-launcher .
```

These builders use Debian 11 and Clang/LLD. The pinned development snapshots are
Tcl `b6ad51455ca6e208ae9c1543ed555c6ead86fcc8` and Tk
`bdde6d9e656d3e292ac363bc4fe45e9c346b86c1` from the Wayland branch.
Tk is configured with `--enable-wayland`; it uses EGL/OpenGL ES directly.

The local patches fix Debian 11 XKB header compatibility, hidden-widget rendering,
ancestor clipping and Canvas damage margins. The Rust frontend uses this toolkit
in-process through FFI. No Python GUI binding is involved.

Follow [the Rust build instructions](../../launcher-rs/MIGRATION.md) to embed the
resulting toolkit and dependency closure into the launcher binary. GPU drivers
and glibc remain supplied by the host. Test in a native Wayland session with
`--check-tk` and confirm that the backend is `wayland`.

This is an experimental Tk snapshot. The Rust migration remains unfinished;
Windows GUI packaging and X11 fallback still need validation.
