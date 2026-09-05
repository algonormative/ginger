---
title: "Ginger v0 — technical design for the first wave"
status: draft for review (2026-09-05); reviewed by a Fable critic, a Codex gpt-5.6-sol pass and a Codex gpt-6-astra self-review before tasks were cut
source: written by Codex gpt-6-astra (piece 8 of the design session) against the plan page "The Running Studio"; libraries verified by the orchestrating session on 2026-09-05 (table at the end)
scope: the two-day experiment and weeks 1 to 2 only; everything else is progressive (see "Deliberately undecided")
---

# Ginger v0 — technical design for the first wave

Plan page: https://claude.ai/code/artifact/9b6a1fd0-ccd8-48e0-866a-02c404cb0150 (sections "What we build next", "How it works together", appendix "Architecture", "Ginger", "Primitives").

## 1. Language and shape

Use **Rust for Ginger’s daemon, compiler, and CLI**. Keep the compiler and rules engine synchronous and independent of I/O so live playback and NRT rendering execute the same code. Run scheduling on a dedicated thread, with asynchronous services handling requests, assets, and persistence around it. Keep Python in music-hub for projections and the existing viewer, and retain sclang for preparing trusted SynthDefs. This gives Ginger one execution model while preserving the existing Python tools.

All library choices below are **to verify**, not confirmed dependencies.

| Responsibility | Choice |
|---|---|
| OSC packets and bundles with timetags | `rosc` (to verify) |
| Async services, local sockets, UDP | `tokio` (to verify); dedicated scheduling thread outside its executor |
| Future QUIC peer transport | `iroh` (to verify), excluded from v0 dependencies |
| JSON and typed structures | `serde` (to verify), `serde_json` (to verify) |
| YAML parsing | `yaml-rust2` (to verify), restricted to a JSON-compatible subset |
| Schema validation | Rust `jsonschema` (to verify); Python `jsonschema` (to verify) |
| Canonical JSON | `serde_json_canonicalizer` (to verify); Python `rfc8785` (to verify) for conformance |
| Local index | `rusqlite` (to verify) |
| Content hashing | `blake3` (to verify) |
| MCP server | `rmcp` (to verify), stdio transport |
| CLI | `clap` (to verify) |
| Probe WAV decoding | `hound` (to verify), float32 WAV only initially |
| Future live plugins | VSTPlugin (to verify), capability-gated and disabled in the first wave |

## 2. Ginger v0 internals

### Module boundaries

One workspace in Ginger; shared ledger code remains in music-hub.

```text
algonormative/ginger/
  Cargo.toml
  Cargo.lock
  crates/
    ginger-core/src/
      declaration.rs       # types, validation, operator manifests
      compiler.rs          # validated declaration -> logical execution plan
      expressions.rs       # bounded expression evaluator
      rules.rs             # deterministic event/state reducer
      clock.rs             # ticks, frames, virtual clock
      random.rs            # per-node deterministic draws
      checkpoint.rs
    ginger-sc/src/
      allocation.rs        # groups, nodes, audio/control buses
      lowering.rs          # logical plan -> scsynth commands
      osc.rs
      readback.rs
      nrt.rs
    lemon-ginger/src/
      main.rs
      service.rs           # shared application operations
      daemon.rs
      scheduler.rs
      capture.rs
      probe.rs
      journal.rs
      local_api.rs
      mcp.rs
  assets/synthdefs/         # trusted source, compiled files, port manifests
  examples/passage/
  tests/{fixtures,engine,nrt}/
  packaging/               # native releases and compatibility metadata

algonormative/music-hub/
  contracts/v0/            # normative schemas, prose rules, conformance vectors
  packages/ledger/
    core/                  # Rust writing, hashing, reading, indexing
    cli/                   # local lemon-ledger commands
  src/music_rig/
    history/
    projections/
    viewer/
```

### Declaration

All listed fields are required unless marked `?`. Maps use stable authored IDs. Unknown declaration fields, duplicate keys, non-finite numbers, YAML tags, anchors, and implicit timestamps are rejected.

```text
GingerDocument:
  schema: "ginger/document/0"
  clock:
    source: "internal"
    bpm: positive decimal string
    ticks_per_beat: 960
    beats_per_bar: integer 1..16
  audio:
    owner: "ginger"
    device_binding: local binding name
    sample_rate: 48000
    output_channels: 2
    input_channels: 0
    block_frames: 64
  seed: 64 lowercase hex characters
  nodes: map<NodeId, Node>
  edges: Edge[]
  taps: map<TapId, Tap>
  state: map<StateId, StateCell>
  rules: map<RuleId, Rule>
  provenance:
    library: Ref | null
    arguments: Ref | null

Node:
  operator: Ref                    # trusted implementation and port manifest
  kind: sample | synth | control | mix | plugin
  ports: map<PortId, Port>         # must match implementation manifest
  controls: map<ControlId, Scalar>
  assets: map<AssetId, Ref>
  voice_limit: integer 1..32
  seed?: 64 lowercase hex characters
  plugin?:                        # recognized, but unsupported in v0 execution
    identity: string
    format: string
    version: string
    state: Ref | null

Port:
  direction: in | out
  rate: audio | control | event
  value_type: number | boolean | string | event
  channels: positive integer
  unit: string | null

Edge:
  from: NodeId.PortId
  to: NodeId.PortId
  feedback: null | { delay_blocks: 1 }

Tap:
  from: NodeId.PortId
  meter: boolean
  record: boolean
  probe_windows: boolean

StateCell:
  type: boolean | integer | number | string
  initial: Scalar
  minimum?: number
  maximum?: number

Rule:
  on: PeriodicEvent | NamedEvent
  priority: integer
  when: Expr
  changes: Change[]

PeriodicEvent:
  clock: "internal"
  every_ticks: positive integer
  offset_ticks: nonnegative integer

NamedEvent:
  name: string

Expr:
  literal
  | read(state/control/event-field)
  | arithmetic/comparison/boolean operation
  | select(predicate, yes, no)
  | random(NodeId)

Change:
  set(state/control target, Expr)
  | trigger(NodeId, parameter expressions)
  | emit(event name, payload expressions, delay_ticks >= 1)
```

### Compiler

Resolve assets, validate types, and produce a logical plan before allocating engine resources. Reject implicit channel conversion, unbounded voice creation, and undeclared cycles.

Lower sources and processors into ordered scsynth groups with allocated buses. Select trusted SynthDefs from their manifests. Event rules create voices; control-rate modulators run as synths and connect through `/n_map`. Reading a mapped control also requires reading its control bus.

For feedback, remove the marked edges before topological sorting. Dedicated feedback readers use InFeedback before the corresponding writers run, enforcing the declared one-block delay. Other cycles are rejected.

Reserve plugin lowering behind a capability flag. Later it creates VSTPlugin-backed nodes with explicit preparation and state loading; v0 reports `unsupported_capability` instead of silently substituting another sound.

### Daemon and scheduling

Use a fixed internal tempo per execution epoch. Convert integer ticks to sample frames using rational arithmetic, then map those frames to OSC timetags through a measured monotonic-time/wall-time anchor. A clock discontinuity halts scheduling and requires a new epoch.

Start with **100 ms lookahead**, a **5 ms wake interval**, and **25 ms admission margin**. These are initial operating targets, not realtime guarantees. Compilation, loading, and journal flushes finish outside the scheduling thread. Recheck the horizon afterward; a missed boundary is rejected, never quietly fired immediately.

Keep **projected state**, **scheduled state**, and **observed engine state** separate. External cues enter only beyond the sealed scheduling horizon. Their resolved position is returned to the caller.

Process events in stable order: frame, priority, source ID, source sequence, rule ID. Each rule evaluates against one state snapshot and commits its changes together. Limit work to 256 events per 100 ms, 64 rule evaluations per event, and 4,096 expression steps per event; exceeding a limit stops further scheduling with an explicit error.

Configuration revision changes only when a candidate activates. Named-state changes have their own sequence. Generate random values from the root seed, stable node ID, and draw counter using a versioned BLAKE3-based algorithm. Checkpoints include counters, pending logical events, and named state.

### Replacement and restart

Allow one pending configuration activation. Choose boundary B beyond the sealed horizon, derive the projected checkpoint immediately before B, and initialize the replacement from it. Events before B belong to the old subtree; events at or after B belong to the replacement.

Keep output routing, meters, crossfaders, and recorders outside replaceable subtrees. Old voices finish their tails without duplicate triggers. Start with a declared 100 ms linear crossfade; reject incompatible state changes or edits to shared stateful effects.

Restart creates a new execution epoch. Ginger stops any surviving engine process it owns, reconciles unfinished journal attempts as unknown, and restores only declared state plus an explicitly selected checkpoint.

### Capture, probe, and readback

Record the master through a persistent DiskOut tap. Separate bounded scratch buffers supply completed two-second probe windows; never decode an open recording as though it were finalized.

`probe` reports sample peak, RMS, channel count, window bounds, and evidence age. Live meter synths provide short-window estimates; completed WAV windows provide reproducible measurements. LUFS, true peak, and pitch remain unavailable unless an explicit external measurement supplies them.

`tree` uses `/g_queryTree` plus lifecycle notifications to report topology, voices, and exposed controls. It does not recover envelope phases, feedback contents, or plugin internals.

A `/sync` barrier is not operation-specific proof. After activation, query affected nodes and controls, recording matches, mismatches, and unknown coverage. Audible timing is measured separately from captures.

### Journal, NRT, and interfaces

Persist an intent before dispatch. Emit queued, applied, rejected, cancelled, partial, or unknown outcomes as evidence arrives. Emit observations for readbacks, decisions, meters, capture progress, and gaps. Batch routine decision traces into referenced artifacts; do not fsync every generated note.

The NRT twin uses the same reducer and logical plan under a virtual clock. It replays recorded cues, writes the OSC score, and renders through scsynth. No live input is silently invented.

One application service backs both CLI and MCP. V0 commands are `validate`, `run`, `stop`, `status`, `prepare`, `apply`, `cancel`, `cue`, `checkpoint`, `capture start/stop`, `probe`, `tree`, `trace`, `render`, and `mcp`. The server is `lemon-ginger`; exposed operations use `ginger_` prefixes.

## 3. Record contract v0

The core knows artifacts, actors, targets, revisions, and positions. Ginger owns musical meanings through namespaced payloads.

These field lists define the frozen wire vocabulary. Optional extensions are namespaced and preserved; they cannot change execution semantics without a supported capability.

```text
Hash: "blake3:" + 64 lowercase hexadecimal characters
Counter: canonical unsigned decimal string, no leading zeros
Timestamp: UTC RFC3339 string with exactly six fractional digits

Ref:
  hash: Hash
  media: string
  scheme: namespaced string          # ledger/jcs-0, bytes/blake3-0, smpl/pcm-v1
  schema: string | null

Target:
  adapter: string
  schema: namespaced string
  data: object                      # interpreted only by that adapter/profile

Position:
  adapter: string
  schema: namespaced string
  epoch: UUID
  data: object

Revision:
  adapter: string
  generation: UUID
  revision: Counter

Context:
  session: UUID
  commit: Hash | null
  execution: UUID | null

SessionDocument:
  schema: "ledger/session/0"
  session: UUID
  title: string
  adapters: map<string, {
    declaration: Ref,
    requirements: Ref | null,
    managed_targets: Target[]
  }>
  resources: map<string, Ref>
  policies: Ref[]

Commit:
  schema: "ledger/commit/0"
  session: UUID
  parents: Hash[]                    # v0 creates zero or one parent
  document: Ref
  author: string
  created_at: Timestamp
  message: string
  reverts: Hash | null

BranchUpdate:
  schema: "ledger/branch/0"
  session: UUID
  branch: string                     # actor/name
  actor: string
  predecessors: Hash[]              # branch-update IDs
  head: Hash
  created_at: Timestamp

CommonRecord:
  schema: "ledger/record/0"
  kind: intent | outcome | observation
  context: Context
  actor: string
  recorded_at: Timestamp

Intent = CommonRecord +:
  kind: intent
  nonce: UUID
  action: namespaced string
  target: Target
  base_commit: Hash | null
  plan: Ref
  expected: Revision[]
  requested_position: Position | null
  related_intent: Hash | null

Outcome = CommonRecord +:
  kind: outcome
  source_event: UUID
  intent: Hash
  previous_outcome: Hash | null
  status: queued | applied | rejected | cancelled | partial | unknown
  before: Revision[]
  after: Revision[]
  positions: Position[]
  evidence: Ref[]
  coverage: Target                  # checked/matched/unknown fields
  reason: { code: string, message: string } | null

Observation = CommonRecord +:
  kind: observation
  source_event: UUID
  type: namespaced string
  about: Hash[]
  revisions: Revision[]
  positions: Position[]
  value: Target
  supersedes: Hash[]

JournalLine:
  schema: "ledger/line/0"
  entry_id: Hash
  writer: UUID
  incarnation: UUID
  seq: Counter
  previous_entry: Hash | null
  record: { id: Hash, body: Intent | Outcome | Observation }

CaptureManifest:
  schema: "ledger/capture/0"
  capture: UUID
  context: Context
  status: complete | interrupted
  streams: [{
    name: string,
    source: Target,
    artifacts: Ref[],
    ranges: [{ start: Position, end: Position }]
  }]
  bindings: [{
    start: Position,
    end: Position,
    commits: Hash[],
    revisions: Revision[],
    evidence: Hash[]
  }]
  gaps: [{ start: Position, end: Position | null, reason: string }]
  provenance: Ref[]

ProjectionResult:
  schema: "ledger/projection/0"
  projection: "rig/session/0" | "rig/compare/0"
  generated_at: Timestamp
  fixture: boolean
  scope: {
    actor: string,
    readable_targets: Target[],
    writable_targets: Target[]
  }
  sources: {
    commits: Hash[],
    feed_watermarks: [{ writer: UUID, incarnation: UUID, seq: Counter }],
    manifests: Hash[]
  }
  completeness: complete | partial
  unknowns: [{ subject: Target, reason: string }]
  payload: Target
```

A Ginger beat position uses `schema: ginger/beat-position/0`; its data contains tick, ticks-per-beat, and meter. Sample positions use `ginger/sample-position/0`. Core code carries these values without interpreting bars or samples. Feedback uses a ledger-owned payload; audio comparisons place their musical details inside Ginger-owned targets and positions.

### Identity and durability

Hash canonical JCS bodies. Store immutable object bodies without self-IDs; their filenames and references supply identity. Record IDs hash only record bodies. Entry IDs hash the complete journal line except `entry_id`, binding delivery order separately.

An intent’s nonce is inside its hashed body. Maintain durable idempotency by actor, execution scope, and nonce: identical retries return the existing attempt; changed requests under that key are rejected. Preserve timestamps and resolved boundaries. Rescheduling requires a new nonce and a relationship to the earlier attempt.

Each producer writes `.local/journal/<writer>/<incarnation>.open`, with sequence starting at zero per incarnation. Seal at 1 MiB or 30 seconds: flush, fsync, publish by hash under `history/journal/<writer>/<incarnation>/`, and fsync the directory. Never resume a crashed incarnation. Recover complete lines; quarantine the suffix and expose the gap.

Commits and branch updates use separate immutable files under `history/`. Concurrent branch heads remain unresolved. Missing referenced objects remain visibly incomplete.

The conformance bundle supplies canonical bytes and expected hashes for every object kind, repeated records, Unicode, decimals, a maximum-u64 revision, and a Ginger position. Negative cases include duplicate keys, bad hashes, non-finite numbers, conflicting nonce reuse, and truncated lines.

## 4. Local Ledger in music-hub

Implement writing, validation, canonicalization, and indexing in the Rust ledger library under `packages/ledger`. Ginger links it; Python calls its CLI for writes. This avoids a second writer implementation and requires no ledger daemon in v0.

SQLite contains objects, commits, branch updates, records, deliveries, revisions, and artifact references. Index batches transactionally. Duplicate IDs must match their existing content; unsigned counters are stored as decimal text and ordered numerically. Declared diff resolves the two documents and compares stable paths. It never blends drift into authored changes.

The feedback POST requires a valid session token, an exact allowed Origin, JSON content type, bounded input, and known artifact references. Derive the actor server-side. Append through the shared writer; do not grant approval from feedback. CLI feedback uses the same application operation without HTTP.

Session projects the selected declaration, execution evidence, capture progress, and scoped observations. Compare projects two candidates, their captures, declared differences, alignment conditions, and feedback. Python produces these results; HTML and CLI format them. Peer replication, transports, blame, and search are deferred.

## 5. Testing without hardware

**Normal CI:** schema validation; canonical fixture round-trips in Rust and Python; deterministic rule traces; seeded variation; state preservation; boundary ordering; limits; nonce retries; partial-file recovery; index rebuilds; and projection fixtures.

A mock scsynth endpoint records outgoing OSC and supplies scripted replies, failures, dropped packets, and delayed readbacks. Test that a barrier alone cannot produce verified success. Golden traces compare logical events and state, excluding transport timestamps and allocated engine IDs.

**SuperCollider CI job:** use a pinned engine image for NRT renders. Test known tones, sample triggers, control mappings, and feedback with measured onset, RMS, and peak tolerances. Compare logical traces exactly; audio is measured rather than universally required to be byte-identical.

**Local audio machine:** run the lag gate using recorded marker impulses. Target a p99 onset error within one 64-frame block after measured alignment, with no accumulating drift over a minute. Stall preparation and scheduling separately; missed admission deadlines must reject, while unexpected engine lateness must appear in evidence.

Projection tests derive valid full hashes from the mockup fixture, retain its short display labels, and verify unknowns, scope, feedback/approval separation, and fixture markings. No test calls a generation service or requires purchased plugins.

## 6. Decisions resolved

V0 uses one Ginger-owned scsynth process, one selected stereo output, 48 kHz, and no physical inputs. Capture happens inside that engine. Do not attach to arbitrary existing servers or coordinate another recorder.

Publish `lemon-ginger` with binary `ginger`; internal crates stay private initially. Pin the ledger dependency and release assets. Native releases include trusted SynthDefs, schemas, and their hashes.

Projection envelopes and existing field meanings remain stable within version zero. Additions are optional; removals or changed meanings require a new version. Display wording is not an API.

**The two-day experiment:** build the core event reducer, OSC lowering, three fixed sources, hold/advance cues, stable master capture, and two alternative continuations. Use a narrow JSON declaration and two static HTML audio players. The human performs the cue; replay its recorded position for comparison. The prototype obeys replacement boundaries but uses an in-memory request queue and simple experiment files.

Do not build SQLite, MCP, generalized schemas, or restart recovery during those two days. Label the page “Experiment; recording real, session history incomplete.” Week one replaces the narrow loader with the v0 declaration; week two adds durable attempts, commits, and generated projections.

## 7. Deliberately undecided

1. External clock following and Link integration.
2. Physical inputs and multitrack device routing.
3. Broader subtree replacement and state migration.
4. Plugin discovery, state portability, and supported plugin matrix.
5. Continuous per-agent audio windows beyond the master tap.
6. Richer measurements and calibrated check libraries.
7. Collections, queues, and richer named-state types.
8. General pattern-library compilation and distribution.
9. Signed identities and remote authorization.
10. QUIC, WebRTC, and browser peers.
11. CAS replication across PCM identities and container-byte hashes.
12. Rytm and Pd participation beyond fixture data.
13. Structural merges, blame, search, and long-term retention policy.

## Library verification (2026-09-05, crates.io and PyPI)

| Crate | Latest | Last release | Note |
|---|---|---|---|
| rosc | 0.11.4 | 2025-03 | OSC packets and bundles with timetags; stable, slow-moving |
| tokio | 1.53.1 | 2026-07 | async services; scheduling stays on a dedicated thread |
| iroh | 1.1.0 | 2026-08 | not a v0 dependency |
| serde / serde_json | 1.0.229 / 1.0.151 | 2026-07 | |
| yaml-rust2 | 0.12.0 | 2026-08 | restrict to a JSON-compatible subset |
| jsonschema | 0.53.0 | 2026-09 | Rust schema validation |
| serde_json_canonicalizer | 0.3.2 | 2026-02 | JCS (RFC 8785) |
| rusqlite | 0.40.2 | 2026-08 | local index |
| blake3 | 1.8.7 | 2026-08 | hashing, same family as smpl's store |
| rmcp | 3.2.0 | 2026-08 | MCP server over stdio |
| clap | 4.6.6 | 2026-08 | CLI |
| hound | 3.5.1 | 2023-09 | float32 WAV reading; maintenance-light, adequate for probes |
| Python jsonschema | 4.26.0 | | conformance tests |
| Python rfc8785 | 0.1.4 | | JCS for the Python side of conformance |
