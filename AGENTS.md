# Repository Guidelines

## Project Structure & Module Organization
- Root `Cargo.toml` builds the Raspberry Pi 4 (`aarch64`) kernel under `src/`. Treat Pi as the only supported architecture; any talk of reintroducing others needs a new plan in `PROJECT.md` first.
- Documentation lives beside the code (`README.md`, `PROJECT.md`, `AGENTS.md`). Add new design notes to this folder root.
- Build artifacts land in `target/`. Do not commit anything from that directory.

## Build, Test, and Development Commands
- `cargo build` – builds the kernel ELF binary (target configured in `.cargo/config.toml`)
- `cargo run` – builds and runs the kernel in QEMU (uses `scripts/qemu-runner.sh`)
- `cargo test` – runs tests in QEMU; exits with status 0 on success
- `cargo objcopy --release -- -O binary kernel8.img` – generates `kernel8.img` for real Pi hardware (only needed for SD card deployment, not QEMU)
- Expected `cargo run` output: Interactive shell with `daedalus>` prompt and welcome message
- Expected `cargo test` behavior: Exits with status 0 when all tests pass
- Install/update the nightly toolchain, `rust-src`, `llvm-tools`, and `cargo-binutils` per `README.md`
- Keep commands scripted (upcoming `xtask`) so contributors can mirror exact invocations

## Coding Style & Naming Conventions
- Rust 2024 edition; nightly toolchain pinned via `rust-toolchain`.
- Prefer module-level documentation comments, `snake_case` for files, `CamelCase` for types, per Rust defaults.
- Use `rustfmt` (nightly) before committing; no tabs—4 spaces indentation.
- Inline comments should explain intent, especially around hardware registers or boot code.

## Architecture Decision Protocol
- Treat any boot-flow, linker, or hardware-interface change as a one-way door. Review `PROJECT.md`, socialize the plan, and record the rationale + rollback strategy there before landing code.
- When in doubt (e.g., touching memory maps, experimenting with new SoCs), pause development and document the proposal in `PROJECT.md` for approval.
- We borrow ideas from Phil Opp's tutorial, but we are not following it chapter-by-chapter. Note which concepts you ported and why instead of referencing chapter numbers.

## Testing Guidelines
- Use Phil Opp's custom test framework pattern with ARM-specific adjustments (semihosting instead of x86 debug ports).
- Name tests after the behavior they cover, e.g., `test_println`, `test_memory_allocation`.
- Mark test functions with `#[test_case]` attribute.
- After every iteration or milestone, run `cargo test --bin daedalus` and verify all tests show `[ok]`.
- After every iteration or milestone, run `cargo run` and confirm the shell launches with interactive prompt.
- When testing shell functionality, verify: prompt displays, commands execute, line editing works (backspace, Ctrl-U, Ctrl-C).
- When adding new tests, ensure they work in the bare-metal environment (no_std, no heap unless allocator is implemented).
- Tests run in QEMU using ARM semihosting for exit; note that QEMU exits with status 1 even on success due to semihosting limitations.
- After milestone testing completes, update `README.md`, `PROJECT.md`, and this file with any new commands, architectural notes, or behavior changes uncovered during the work.

## Tooling & CLI Usage
- Favor built-in tools over custom bash commands even for mundane tasks (listing files, showing snippets, searching) because each ad-hoc `bash` invocation needs approval; keep the workflow approval-light by leaning on the provided tools first.
- If a shell wrapper (`bash -lc`, custom script) is genuinely required, clearly document the reason in your notes/PR so reviewers know why the builtin path was insufficient.
- **NEVER run `cargo run` in the background** - Claude Code cannot interact with the shell. Instead, instruct the user to run `cargo run` and provide specific commands to type.
- Set timeouts (10-30 seconds) for all bash commands, especially `cargo test` which should complete in under 10 seconds.

## Commit & Pull Request Guidelines
- Commits should be scoped and descriptive (`Added PL011 console`, `Defined Pi linker script`). Use present tense and mention the subsystem touched.
- Reference design decisions in commit messages when changing boot flows or hardware magic numbers (e.g., “Documented PL011 base 0xFE201000”).
- Pull requests must summarize changes, list verification steps (Pi build + QEMU run), and note any outstanding TODOs. Include screenshots or QEMU logs only when they clarify serial/VGA output.
