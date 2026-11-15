# Rust migration — work in progress

The production Python launcher is still the reference implementation. This branch
is not a completed replacement and must not overwrite a working installation.
The development Rust frontend uses the same patched Tk 9.2 in process, without
Python or an external wish process. Rust uses the LLVM compiler toolchain;
C/Tk dependencies and the final native link use Clang/LLD in Debian 11/Podman.

Implemented foundations:

- Tcl/Tk 9 ABI and Rust callbacks, with thread confinement and panic boundaries.
- Existing widget tree, images, fonts, styles and geometry, covering 17 pages
  in Russian and English, with saved/system language selection.
- Framebuffer snapshots for comparison with the reference UI. The 17 English
  pages match their reference captures pixel for pixel.
- Navigation, core option variables, compatible JSON/INI files and atomic writes.
- Native smooth-scroll spring, folder selection and desktop link opening.
- Bounds-checked plaintext SELF/ELF and SFO parsers and loaded-image hashes.
- Native Boot2/Boot5 linking, TLS, FS-to-GS rewriting, guarded CPU translations
  and Windows red-zone protection. Prepared images are compared byte for byte
  against the reference, including the combined legacy-CPU/Windows profile.
- Literal XML patch compilation and original-code fingerprinting: all 2,983
  valid built-in writes match the reference; out-of-image patches are rejected.
- Mod discovery, ordering, case-insensitive paths and temporary read-only views.
- Save title/slot/name inspection, compatible ZIP backups and CRC/SHA-256
  verification. Restore/import UI is not connected yet.
- Linux single-file toolkit/dependency embedding and native Wayland startup
  with the build-time toolkit hidden. Embedded ELF requirements are <= glibc 2.30.

Still required before switching the production entry point:

- Complete behavior parity for every callback, dialog and dynamic view.
- Complete game preparation, patching, mods and process lifecycle.
- Account/online, saves, trophies, input mapping, hardware and update workflows.
- Translated dynamic dialogs, DPI/window resizing and Windows native behavior.
- Windows toolkit packaging, full license audit and Windows/Linux release CI checks.

Development commands use `packaging/linux/Containerfile.rust-launcher` after
building the existing patched Tk preview image. `--check-tk`, `--check-ui`,
`--snapshot-ui DIRECTORY`, `--inspect-game DIRECTORY`, and
`--prepare-game GAME_DIRECTORY OUTPUT_DIRECTORY` exercise the port.
`--preview-ui` is a development preview. Unported callbacks are explicitly
reported, not treated as implemented functionality.

`tools/rust-migration/export_ui.py` is a one-time reference extraction/QA tool,
not code run by the Rust launcher. The checked-in interface resources contain
no user account/profile data; exports use an isolated test profile.


Linux development build (after the patched Tk images described in
`packaging/linux/TK92-WAYLAND.md` have been built):

```sh
podman build -t localhost/bbport-rust-launcher -f packaging/linux/Containerfile.rust-launcher .
mkdir -p out/rust-cargo/registry
CARGO_HOME="$PWD/out/rust-cargo" cargo fetch --locked --manifest-path launcher-rs/Cargo.toml
podman run --rm --init -v "$PWD:/src" \
  -v "$PWD/out/rust-cargo/registry:/opt/cargo/registry" \
  -e CARGO_TARGET_DIR=/src/out/rust-build \
  -e BB_TK_BUNDLE=/src/out/rust-toolkit localhost/bbport-rust-launcher bash -c \
  'python3 /src/packaging/linux/stage-rust-toolkit.py /src/out/rust-toolkit && cargo test --locked --offline && cargo build --locked --offline --release'
```

The development executable is `out/rust-build/release/bloodborne-launcher`.
Python above stages build resources; it is not included in or called by that
executable. `--preview-ui` remains required until behavior parity is complete.
Do not distribute this preview as the finished launcher.

`tools/rust-migration/check_native.py` compares native patch binaries,
fingerprints, prepared images and CPU/red-zone reports with the reference.
It accepts a binary, a prepared output directory, `--reference-prepared` and
`--output`; it requires local game preparation outputs but never distributes them.
