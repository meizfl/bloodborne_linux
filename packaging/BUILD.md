# Rust launcher build

See [the Rust migration build instructions](../launcher-rs/MIGRATION.md).
This branch does not include the legacy launcher or its Python packaging tools.

Linux release builds use Debian 11, Podman and Clang/LLD. The native game runtime
and game files remain separate. Windows core tests are configured in
`.github/workflows/rust-launcher.yml`; Windows GUI packaging remains unfinished.
