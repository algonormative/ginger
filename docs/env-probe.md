# Environment probe — executor sandbox

Probed 2026-09-16 in the Claude Routine executor sandbox (`routine-default` environment),
the machine that runs the cloud tier of [testing.md](testing.md). Every number below came
from a command run in that session; nothing is estimated.

## Host and toolchain

| | |
| --- | --- |
| `uname -srm` | `Linux 6.18.44-fc-v33 x86_64` |
| `nproc` | 4 |
| `cargo --version` | `cargo 1.94.1 (29ea6fb6a 2026-03-24)` |
| `rustc --version` | `rustc 1.94.1 (e408947bf 2026-03-25)` |
| active toolchain | `stable-x86_64-unknown-linux-gnu`, active because overridden by `rust-toolchain.toml` |
| rustup home | `/root/.rustup`; cargo/rustc/rustup at `/root/.cargo/bin` |

The pinned `rustfmt` and `clippy` components are both present — `cargo fmt` and
`cargo clippy` ran without a component install.

## Verification commands

All five exited 0. Timings are wall clock (`date +%s.%N` deltas), `CARGO_TERM_COLOR=never`.
Cold = immediately after `cargo clean`; warm = against a populated `target/`.

| Command | Exit | Cold | Warm |
| --- | --- | --- | --- |
| `cargo --version` | 0 | — | — |
| `cargo build --workspace --locked` | 0 | 0.22 s | 0.07 s |
| `cargo test --workspace --locked` | 0 | 0.21 s | 0.08 s |
| `cargo fmt --all -- --check` | 0 | 0.07 s | 0.07 s |
| `cargo clippy --workspace -- -D warnings` | 0 | 0.14 s | 0.10 s |

Cold and warm are close because the workspace has **no external dependencies** —
`Cargo.lock` lists only `ginger-core`, `ginger-sc` and `lemon-ginger`, so a cold build
compiles three local crates and nothing else. `--locked` did not need to touch the network.

## Test inventory

`cargo test --workspace` runs five targets. Only one test exists today:

- `ginger-core` unittests — `running 1 test`, `tests::scaffold_builds ... ok`
- `ginger-sc` unittests — `running 0 tests`
- `lemon-ginger` (`src/main.rs`) unittests — `running 0 tests`
- doc-tests `ginger_core`, doc-tests `ginger_sc` — `running 0 tests` each

`tests/engine/` and `tests/nrt/` currently contain only a `README.md`, so neither
contributes a test target yet. A green `cargo test` here means the workspace compiles and
one scaffold assertion passes — it is not coverage.

## Disk

`cargo clean` removed 161 files / 27.8 MiB. `du -sh target` then read:

| Point | `target/` |
| --- | --- |
| after `cargo clean` | absent |
| after cold `cargo build` | 5.4 M |
| after `cargo test` + `cargo clippy` | 25 M |

`df -h /` read `252G` size / `8.3G` used / `29G` avail / 23% both before and after the
whole probe — the ~25 M of build output is below the resolution `df -h` reports.

## What this proves

- The cloud tier's four-command verification string runs end to end in this sandbox, green,
  in well under a second warm and about 0.6 s cold.
- The `rust-toolchain.toml` pin resolves here; rustfmt and clippy are installed.
- A full rebuild costs ~25 M of disk, which this sandbox has room for.

## What this does not prove

- **Nothing about audio.** There is no SuperCollider here: `scsynth` and `sclang` are not on
  `PATH`, and this probe did not look for one elsewhere or install one. NRT renders
  (`tests/nrt`) and the lag gate cannot run in this environment and were not attempted.
- Nothing about test coverage — see the inventory above.
- Nothing about build cost once real dependencies land; today's timings are a floor for a
  dependency-free workspace, not a prediction.

Evidence label: **mock**.
