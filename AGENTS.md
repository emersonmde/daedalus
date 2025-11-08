# Repository Guidelines

## Project Structure & Module Organization
- Root `Cargo.toml` builds the Raspberry Pi 4 (`aarch64`) kernel under `src/`. Treat Pi as the only supported architecture; any talk of reintroducing others needs a new plan in `ARCHITECTURE.md` first.
- Documentation lives beside the code (`README.md`, `ARCHITECTURE.md`, `AGENTS.md`). Add new design notes to this folder root.
- Build artifacts land in `target/`. Do not commit anything from that directory.

## Build, Test, and Development Commands
- `cargo build --target aarch64-daedalus-os.json` – produces both the ELF and `kernel8.img` for Pi firmware/QEMU.
- `qemu-system-aarch64 -M raspi4b -cpu cortex-a72 -serial stdio -display none -kernel target/aarch64-daedalus-os/debug/kernel8.img` – launches the Pi kernel in QEMU; capture the serial output for logs.
- Install/update the nightly toolchain, `rust-src`, `llvm-tools-preview`, and `bootimage` per `README.md`.
- Keep commands scripted (upcoming `xtask`) so contributors can mirror exact invocations.

## Coding Style & Naming Conventions
- Rust 2024 edition; nightly toolchain pinned via `rust-toolchain`.
- Prefer module-level documentation comments, `snake_case` for files, `CamelCase` for types, per Rust defaults.
- Use `rustfmt` (nightly) before committing; no tabs—4 spaces indentation.
- Inline comments should explain intent, especially around hardware registers or boot code.

## Architecture Decision Protocol
- Treat any boot-flow, linker, or hardware-interface change as a one-way door. Review `ARCHITECTURE.md`, socialize the plan, and record the rationale + rollback strategy there before landing code.
- When in doubt (e.g., touching memory maps, experimenting with new SoCs), pause development and document the proposal in `ARCHITECTURE.md` for approval.
- We borrow ideas from Phil Opp’s tutorial, but we are not following it chapter-by-chapter. Note which concepts you ported and why instead of referencing chapter numbers.

## Testing Guidelines
- Reuse Phil Opp’s testing ideas when they help, but build only the infrastructure that benefits the Pi target today.
- Name tests after the behavior they cover, e.g., `test_writer_scrolls_on_newline`.
- After every iteration or milestone, build and run the Pi target: `cargo build --target aarch64-daedalus-os.json` then `qemu-system-aarch64` as above. Capture the serial output and have the operator confirm the expected string (e.g., “Please verify the UART prints `Hello from Daedalus`”). Do not close the iteration until that feedback is recorded.
- When new host-side tests or QEMU smoke tests are added, document how to run them and keep them opt-in so they do not block the basic Pi build.
- After milestone testing completes, update `README.md`, `ARCHITECTURE.md`, and this file with any new commands, architectural notes, or behavior changes uncovered during the work.

## Tooling & CLI Usage
- You MUST use the Codex built-in tools/commands (direct `ls`, `cat`, `rg`, file editors, etc.) before reaching for custom shell wrappers. The workspace already permits read operations without escalation.
- If a shell wrapper (`bash -lc`, custom script) is genuinely required, clearly document the reason in your notes/PR so reviewers know why the builtin path was insufficient.

## Commit & Pull Request Guidelines
- Commits should be scoped and descriptive (`Added PL011 console`, `Defined Pi linker script`). Use present tense and mention the subsystem touched.
- Reference design decisions in commit messages when changing boot flows or hardware magic numbers (e.g., “Documented PL011 base 0xFE201000”).
- Pull requests must summarize changes, list verification steps (Pi build + QEMU run), and note any outstanding TODOs. Include screenshots or QEMU logs only when they clarify serial/VGA output.
