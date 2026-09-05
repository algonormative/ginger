# Testing Ginger

Three tiers, each with its own evidence label. A PR states which tier produced its evidence: **mock**, **SC render**, or **physical capture**. Mock success is never reported as playback proof.

## Cloud (CI and the autonomous executor)
No SuperCollider, no audio device. Runs `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -D warnings`.
- Unit tests in each crate.
- Engine tests against the mock scsynth endpoint (`crates/ginger-sc/tests/`): it records outgoing OSC, replies with scripted responses, failures, dropped packets and delayed readbacks. A `/sync` barrier alone must never produce verified success.
- Golden traces: logical events and named state compared exactly; transport timestamps and allocated engine ids excluded.
- Schema and conformance round-trips for records Ginger writes (bundle from music-hub `contracts/v0`).

## SuperCollider runner (a machine with `scsynth`; later a dedicated runner)
- NRT renders of fixtures; audio compared by measurement (onset, RMS, peak tolerances), logical traces exactly. `RandSeed` per node makes stochastic UGens repeatable.
- Interactive readback: `/g_queryTree` after activation matches the compiled graph; mismatches and unknown coverage are recorded, not hidden.

## Local audio (the owner's machine; `needs: local audio` in the task)
- Lag gate with recorded marker impulses: p99 onset error within one 64-frame block at 48 kHz after measured alignment; zero scsynth "late" lines; no accumulating drift over an hour.
- Sustained playback, capture finalization, restart into a new execution epoch, listening.
