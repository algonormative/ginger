//! ginger-core: declaration types, validation, compiler to a logical plan, expressions,
//! the deterministic rules reducer, clocks, seeded randomness and checkpoints.
//! Synchronous and free of I/O so live playback and NRT rendering share one code path.
//! Module plan (docs/design/ginger-v0-technical-design.md §2): declaration, compiler,
//! expressions, rules, clock, random, checkpoint.

/// Ticks per beat fixed for v0 (design §2, Declaration.clock).
pub const TICKS_PER_BEAT: u32 = 960;

#[cfg(test)]
mod tests {
    #[test]
    fn scaffold_builds() {
        assert_eq!(super::TICKS_PER_BEAT, 960);
    }
}
