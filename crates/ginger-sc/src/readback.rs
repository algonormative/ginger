//! Correlating dispatched commands with the evidence that came back. Design amendment 18,
//! "Evidence never overstates certainty": applied-ness is established by correlating `/fail`,
//! `/n_go` and post-boundary tree queries, with `unknown` retained for races. A `/sync` barrier
//! is ordering, not proof — a node whose readback never arrived stays [`Outcome::Unknown`].

use crate::osc::{Message, Reply};
use std::collections::BTreeMap;

/// What the evidence supports for one node. `Unknown` is a real answer, not a default that
/// pretends: it means no readback for that node has been received.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Applied,
    Rejected,
    Unknown,
}

#[derive(Clone, Debug, Default)]
pub struct Correlator {
    outcomes: BTreeMap<i32, Outcome>,
    barriers: Vec<i32>,
}

impl Correlator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a command as dispatched. Node-addressing commands start at `Unknown`: having
    /// sent a packet is not evidence that the engine acted on it.
    pub fn observe_command(&mut self, msg: &Message) {
        let node_id = match msg.address.as_str() {
            "/s_new" => msg.int(1),
            "/n_set" => msg.int(0),
            _ => None,
        };
        if let Some(node_id) = node_id {
            self.outcomes.entry(node_id).or_insert(Outcome::Unknown);
        }
    }

    /// Fold one reply into the outcomes. A rejection is never overturned by a later tree
    /// listing: an explicit `/fail` outranks topology read after the fact.
    pub fn observe_reply(&mut self, reply: &Reply) {
        match reply {
            Reply::NodeStarted(node_id) => {
                self.outcomes.insert(*node_id, Outcome::Applied);
            }
            Reply::Failed {
                node_id: Some(node_id),
                ..
            } => {
                self.outcomes.insert(*node_id, Outcome::Rejected);
            }
            Reply::QueryTreeReply(node_ids) => {
                for node_id in node_ids {
                    if self.outcome(*node_id) != Outcome::Rejected {
                        self.outcomes.insert(*node_id, Outcome::Applied);
                    }
                }
            }
            // A barrier orders the stream; it says nothing about any node.
            Reply::Synced(id) => self.barriers.push(*id),
            Reply::Failed { node_id: None, .. } => {}
        }
    }

    /// The outcome for one node. A node nothing was observed about is `Unknown`.
    pub fn outcome(&self, node_id: i32) -> Outcome {
        let known = self.outcomes.get(&node_id);
        known.copied().unwrap_or(Outcome::Unknown)
    }

    /// The tracked nodes still without readback — the unknown coverage to report.
    pub fn unknown_nodes(&self) -> Vec<i32> {
        let unknown = |(n, o): (&i32, &Outcome)| (*o == Outcome::Unknown).then_some(*n);
        self.outcomes.iter().filter_map(unknown).collect()
    }

    pub fn barriers(&self) -> &[i32] {
        &self.barriers
    }
}
