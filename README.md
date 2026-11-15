# Bloodborne launcher — Rust-only development branch

This branch contains the in-progress native Rust launcher and the existing native
runtime sources. The launcher uses Tcl/Tk directly through FFI. Its executable
contains no Python interpreter, scripts or shell launch wrappers.

The port is **not a finished replacement**: game launch integration, online,
several UI workflows and the Windows toolkit package still need work. The existing
working launcher is preserved on `codex/linux-shared-launcher`; the migration
reference is on `codex/rust-launcher`.

See [launcher-rs/MIGRATION.md](launcher-rs/MIGRATION.md) for status and Linux build
instructions. User data remains in `$HOME/.local/share/Bloodborne` on Linux and
`%LOCALAPPDATA%\Bloodborne` on Windows. The separate game runtime belongs in
`game_runtime/`, with its libraries in `game_runtime/libs/`.

There are no tracked Python source files in this branch's current tree. Standard
third-party native build tools can still require system Python. This branch does
not rewrite Git history or the upstream submodule histories.
