Engine tests run against a mock scsynth endpoint (records outgoing OSC, replies with scripted responses,
failures, dropped packets and delayed readbacks). No real scsynth is used here; see `tests/nrt` for renders.

`fixtures/` holds golden traces as JSON: `{ name, events, state }`, where each event carries a logical `frame`,
an authored `node`, a `kind` and string `params`, plus optional `wall_time` and `engine_node_id` — transport,
recorded for reading and never compared.
