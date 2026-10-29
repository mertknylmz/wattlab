# Rust toolchain smoke example

This learning example verifies Cargo build/run/test and demonstrates borrowed slices,
Option, and the difference between missing and zero measurements. It does not access
Bluetooth, HTTP, or PostgreSQL and is not the Rust backend implementation.

Open this directory or its Cargo.toml in RustRover. Let Cargo finish syncing, then use
the Run icon next to main or the tests. Rust tools are installed in ~/.cargo/bin;
restart the IDE after toolchain installation if they are not detected.

From this directory:

```bash
cargo run --offline
cargo test --offline
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
```

There are no external dependencies. Cargo.lock is tracked for the executable example.
The default stable toolchain was verified locally as Rust/Cargo 1.99.1. No IDE debugger
session has been verified yet; IDE installation is being handled by the user.
