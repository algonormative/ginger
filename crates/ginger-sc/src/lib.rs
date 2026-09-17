//! ginger-sc: lowering of a logical plan to scsynth (groups, buses, SynthDefs, /n_map,
//! InFeedback for declared one-block feedback), OSC encoding with timetags, readback via
//! /g_queryTree and notifications, and NRT score writing. Nothing here runs without a
//! scsynth or a mock endpoint; tests use the mock (tests/engine).
//! Present so far: `osc` (command encoding, reply decoding) and `readback` (the evidence
//! correlator). Allocation, lowering and NRT follow.

pub mod osc;
pub mod readback;
