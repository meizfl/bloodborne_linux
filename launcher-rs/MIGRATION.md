# Rust migration — work in progress

The production launcher on the original branches remains the reference. This branch
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
- Guarded native plans for Quit to Desktop, PC Settings, Rebirth, cloth, foliage,
  jumps and effect timing. Their eight code-write outputs match the reference.
- Render/output/HUD sizes, Sharp UI and ultrawide patch rewriting; four native,
  4K and ultrawide output cases match the reference byte for byte.
- Mod discovery, ordering, case-insensitive paths and temporary read-only views.
- Keyboard/mouse/controller input.ini model, combinations, conflict detection,
  Dark Souls III layout and one-shot capture state. Eight read/rewrite/layout/mouse
  cases match the Python reference; capture GUI wiring is still in progress.
- Save title/slot/name inspection, compatible ZIP backups and CRC/SHA-256
  verification. Restore/import UI is not connected yet.
- Linux single-file toolkit/dependency embedding and native Wayland startup
  with the build-time toolkit hidden. Embedded ELF requirements are <= glibc 2.30.

Still required before switching the production entry point:

- Complete behavior parity for every callback, dialog and dynamic view.
- Complete game preparation, patching, mods and process lifecycle.
- Account/online, saves, trophies, input mapping, hardware and update workflows.
- Remaining source languages (Arabic, Spanish, Portuguese, French, German,
  Italian, Polish, Turkish, Chinese, Japanese and Korean), translated dynamic
  dialogs, DPI/window resizing and Windows native behavior.
- Windows toolkit packaging, full license audit and Windows/Linux release CI checks.
  A native-core CI workflow is present; Windows cross-target checking succeeds,
  but Windows GUI/runtime execution has not yet been validated.

Development commands use `packaging/linux/Containerfile.rust-launcher` after
building the existing patched Tk preview image. `--check-tk`, `--check-ui`,
`--snapshot-ui DIRECTORY`, `--inspect-game DIRECTORY`, and
`--prepare-game GAME_DIRECTORY OUTPUT_DIRECTORY` exercise the port.
`--check-interactions` drives real Tk help/hover/combobox callbacks.
`--inspect-backup ARCHIVE` verifies compatible ZIP backups.
`--preview-ui` is a development preview. Unported callbacks are explicitly
reported, not treated as implemented functionality.

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
  'bash /src/packaging/linux/stage-rust-toolkit.sh /src/out/rust-toolkit && cargo test --locked --offline && cargo build --locked --offline --release'
```

The development executable is `out/rust-build/release/bloodborne-launcher`.
The shell command stages native build resources; it is not a player launch wrapper.
`--preview-ui` remains required until behavior parity is complete.
Do not distribute this preview as the finished launcher.

This branch intentionally omits the Python reference and all Python migration/QA
scripts. Their last version remains on `codex/rust-launcher`. Checked-in UI and
patch resources are data; native action identifiers replace source-code snippets.
The native Rust unit tests and `--check-interactions` remain available.

The build-image chain is `Containerfile` → `Containerfile.tk92` →
`Containerfile.tk92-render` → `Containerfile.rust-launcher`, tagged respectively
`localhost/bbport-debian11`, `localhost/bbport-tk92`,
`localhost/bbport-tk92-render`, `localhost/bbport-rust-launcher`.
CMake/Meson and other external native build tools may use system Python; no
project Python code is included in this branch. To run Rust core tests without
building Tk, use `cargo test --locked --manifest-path launcher-rs/Cargo.toml`.
