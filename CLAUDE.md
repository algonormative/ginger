# Ginger — conventions for agents

Read `docs/design/ginger-v0-technical-design.md` before changing anything. It is the design of record for the
first wave; the plan page it belongs to is linked at its top. Tasks live in the vault's beads store under the
Ginger epic; one repo per task, one PR per task, branches from `origin/main`.

## Rules that do not bend

- **Primitives, not patterns.** The core declaration knows a typed graph, clocks and events, named state, rules
  (event + predicate + guarded change), seeds, taps and an audio owner. Section functions, energy curves, roles,
  budgets and every other compositional idea live in authored libraries that compile *into* the core. If a change
  adds a musical noun to `ginger-core`, stop and file a follow-up instead.
- **Synchronous core, no I/O.** `ginger-core` has no sockets, files or clocks of its own; live playback and NRT
  rendering call the same compiler and reducer. Scheduling runs on a dedicated thread; async services stay
  outside it.
- **Candidate before execution.** A change is a candidate first; applying it is a separate step that produces an
  outcome. Nothing writes declared state retroactively.
- **Evidence over assertion.** A `/sync` barrier is not proof. Readback and captures are; unknowns are reported
  as unknown. The record contract (music-hub `contracts/v0`) is the only wire format for journal records.
- **Deterministic by construction.** Randomness comes from the root seed, the node id and a draw counter. Golden
  traces in `tests/engine` compare logical events and state, never transport timestamps.

## What runs where

- CI (no SuperCollider, no audio device): build, unit tests, golden traces, mock-scsynth engine tests, schema and
  fixture round-trips.
- A machine with `scsynth`: NRT measurement tests (`tests/nrt`) and the lag-gate probe. A task that needs them
  says so in its description with the line `needs: local audio`; autonomous workers skip those parts and leave a
  `needs:human` note in the PR.

## Working here

- `cargo build --workspace --locked && cargo test --workspace --locked && cargo fmt --all -- --check && cargo clippy --workspace -- -D warnings`
- Keep PRs under 1000 changed lines. Multi-file refactors are fine when the task's `## Acceptance` names the surface.
- Every task closes with a `Follow-ups:` line naming the next one to three tasks it revealed.
