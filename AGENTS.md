# Repository Guidelines

## Project Structure & Module Organization
- Root `Cargo.toml` currently hosts the tutorial-aligned x86_64 kernel under `src/`. Architecture unification work will move code into `kernels/` (see `ARCHITECTURE.md` for the roadmap), so keep new modules arch-aware via `cfg`.
- Documentation lives beside the code (`README.md`, `ARCHITECTURE.md`, `AGENTS.md`). Add new design notes to this folder root.
- Build artifacts land in `target/`. Do not commit anything from that directory.

## Build, Test, and Development Commands
- `cargo bootimage` – builds the x86_64 kernel with Philipp Oppermann’s bootloader.
- `qemu-system-x86_64 -drive format=raw,file=target/x86_64-daedalus-os/debug/bootimage-daedalus-os.bin` – launches the tutorial kernel in QEMU.
- Future Pi target: follow §4.2 of `ARCHITECTURE.md` to build `kernel8.img` via `cargo build --target aarch64-daedalus-os.json` and run `qemu-system-aarch64 -M raspi4b -kernel <elf> -serial stdio`.
- Keep commands scripted (upcoming `xtask`) so contributors can mirror exact invocations.

## Coding Style & Naming Conventions
- Rust 2021 edition; nightly toolchain pinned via `rust-toolchain`.
- Prefer module-level documentation comments, `snake_case` for files, `CamelCase` for types, per Rust defaults.
- Use `rustfmt` (nightly) before committing; no tabs—4 spaces indentation.
- Inline comments should explain intent, especially around hardware registers or boot code.

## Testing Guidelines
- Follow Phil Opp’s chapter on custom test frameworks once the tutorial reaches it; until then, rely on unit tests in shared modules (`#[cfg(test)]`).
- Name tests after the behavior they cover, e.g., `test_writer_scrolls_on_newline`.
- After every iteration or milestone, rebuild and re-run the appropriate QEMU target(s):  
  - x86_64: `cargo bootimage` then `qemu-system-x86_64 -drive format=raw,file=target/x86_64-daedalus-os/debug/bootimage-daedalus-os.bin`.  
  - Pi (once enabled): `cargo build --target aarch64-daedalus-os.json` then `qemu-system-aarch64 -M raspi4b -cpu cortex-a72 -serial stdio -kernel <elf>`.  
  Capture the serial/VGA output; if you cannot view QEMU yourself, give the operator these commands plus the exact string to confirm (“Please verify the UART prints `Hello from Daedalus`”). Do not close the iteration until that feedback is recorded.
- When the Pi target lands, document any host-side tests or QEMU smoke tests added; keep them opt-in so they do not block x86 builds.

## Commit & Pull Request Guidelines
- Commits should be scoped and descriptive (`Added VGA driver`, `Ported UART console`). Use present tense and mention the subsystem touched.
- Reference design decisions in commit messages when changing boot flows or hardware magic numbers (e.g., “Documented PL011 base 0xFE201000”).
- Pull requests must summarize changes, list verification steps (`cargo bootimage`, QEMU runs), and note any outstanding TODOs. Include screenshots or QEMU logs only when they clarify visual output (e.g., VGA text).
