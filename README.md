# Ginger

**Run and render musical systems from editable declarations.** Ginger by Lemon Audio.

Describe sources, connections, controls and behavior in a declaration, then play the result
through SuperCollider's `scsynth`. Inspect the running instrument, prepare a variation, record
its execution, and render the same declaration offline. Musical patterns live in libraries you
can read and change; the core knows only a few primitives: a typed graph, clocks and events,
named state, rules that wire events and predicates to guarded changes, and capture taps.

Status: **scaffold** (2026-09-05). Nothing plays yet. The technical design for the first wave
is in [`docs/design/ginger-v0-technical-design.md`](docs/design/ginger-v0-technical-design.md);
the plan it belongs to is *The Running Studio* (linked from that document). Work is tracked as
beads tasks in the vault under the Ginger epic.

## Layout

```
crates/ginger-core   declaration, validation, compiler to a logical plan, rules reducer, clock, seeded randomness, checkpoints (sync, no I/O)
crates/ginger-sc     lowering to scsynth, OSC with timetags, readback, NRT score writing
crates/lemon-ginger  the `ginger` binary: service, daemon, scheduler, capture, probe, journal, local API, MCP
assets/synthdefs     trusted SynthDef sources, compiled files and port manifests
examples/passage     the first passage (days 1 to 2, week 1)
tests/{fixtures,engine,nrt}   conformance fixtures, mock-scsynth engine tests, NRT measurement tests
```

## Install (target, not yet published)

```bash
cargo install lemon-ginger --locked     # binary: ginger
ginger --version
```

Requires SuperCollider (`scsynth`) on the machine that plays; the compiler, reducer and their
tests run without it.

## License

MIT.
